use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::JoinHandle;

use evil_render::{
    ExportPlan, ExportSpec, RenderCancellation, RenderOutput, RenderScope, RenderSnapshot,
};

const CANCELLATION_GRACE: std::time::Duration = std::time::Duration::from_millis(100);
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static TEMPORARY_LIFECYCLE: LazyLock<Mutex<HashSet<OsString>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static SWEPT_DIRECTORIES: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationTestPoint {
    TemporaryCreate,
    BeforeWrite,
    BeforeLock,
    BeforeRename,
}

#[cfg(test)]
type PublicationHook = Arc<dyn Fn(u64, PublicationTestPoint) + Send + Sync>;
#[cfg(test)]
type WorkerHook = Arc<dyn Fn() + Send + Sync>;

fn destination_identity(output: &Path) -> PathBuf {
    fn normalize(path: &Path) -> PathBuf {
        let Some(file_name) = path.file_name() else {
            return path.to_path_buf();
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::canonicalize(parent)
            .map_or_else(|_| path.to_path_buf(), |parent| parent.join(file_name))
    }

    let mut current = output.to_path_buf();
    let mut seen = HashSet::new();
    for _ in 0..64 {
        let normalized = normalize(&current);
        if !seen.insert(normalized.clone()) {
            return normalized;
        }
        if let Ok(target) = std::fs::canonicalize(&current) {
            return target;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return normalized;
        };
        if !metadata.file_type().is_symlink() {
            return normalized;
        }
        let Ok(target) = std::fs::read_link(&current) else {
            return normalized;
        };
        let parent = current
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        current = if target.is_absolute() {
            target
        } else {
            parent.join(target)
        };
    }
    current
}

#[derive(Default)]
struct PublicationLock {
    held: AtomicBool,
}

struct PublicationGuard {
    lock: Arc<PublicationLock>,
    owner: RenderOwner,
    generation: u64,
}

impl PublicationGuard {
    fn acquire(
        lock: Arc<PublicationLock>,
        cancellation: &RenderCancellation,
        owner: &RenderOwner,
        generation: u64,
    ) -> Result<Self, RenderError> {
        while lock
            .held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            cancellation
                .check()
                .map_err(|_| RenderError::cancelled(RenderStage::Publication))?;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        owner
            .publication_jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(generation);
        Ok(Self {
            lock,
            owner: owner.clone(),
            generation,
        })
    }
}

impl Drop for PublicationGuard {
    fn drop(&mut self) {
        self.owner
            .publication_jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.generation);
        self.lock.held.store(false, Ordering::Release);
    }
}

#[derive(Clone, Default)]
pub struct RenderOwner {
    generation: Arc<AtomicU64>,
    publication: Arc<Mutex<HashMap<PathBuf, Arc<PublicationLock>>>>,
    publication_jobs: Arc<Mutex<HashSet<u64>>>,
    workers: Arc<AtomicUsize>,
    #[cfg(test)]
    worker_test_hook: Arc<Mutex<Option<WorkerHook>>>,
    #[cfg(test)]
    result_test_hook: Arc<Mutex<Option<WorkerHook>>>,
    #[cfg(test)]
    publication_test_hooks: Arc<Mutex<HashMap<PathBuf, PublicationHook>>>,
    #[cfg(test)]
    identity_test_hook: Arc<Mutex<Option<WorkerHook>>>,
}

impl RenderOwner {
    fn begin(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn invalidate(&self, generation: u64) {
        if self.generation.load(Ordering::Acquire) == generation {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    fn publication_lock(
        &self,
        _generation: u64,
        output: &Path,
        cancellation: &RenderCancellation,
    ) -> Result<PublicationGuard, RenderError> {
        #[cfg(test)]
        self.call_identity_test_hook();
        let identity = destination_identity(output);
        let lock = self
            .publication
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(identity)
            .or_default()
            .clone();
        #[cfg(test)]
        self.call_publication_test_hook(_generation, output, PublicationTestPoint::BeforeLock);
        PublicationGuard::acquire(lock, cancellation, self, _generation)
    }

    #[cfg(test)]
    fn set_identity_test_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self
            .identity_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(hook));
    }

    #[cfg(test)]
    fn call_identity_test_hook(&self) {
        let hook = self
            .identity_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(test)]
    pub(crate) fn set_publication_test_hook(
        &self,
        output: &Path,
        hook: impl Fn(u64, PublicationTestPoint) + Send + Sync + 'static,
    ) {
        let identity = destination_identity(output);
        self.publication_test_hooks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(identity, Arc::new(hook));
    }

    #[cfg(test)]
    fn call_publication_test_hook(
        &self,
        generation: u64,
        output: &Path,
        point: PublicationTestPoint,
    ) {
        let identity = destination_identity(output);
        let hook = self
            .publication_test_hooks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&identity)
            .cloned();
        if let Some(hook) = hook {
            hook(generation, point);
        }
    }

    #[cfg(test)]
    pub(crate) fn worker_count(&self) -> usize {
        self.workers.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn set_worker_test_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self
            .worker_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(hook));
    }

    #[cfg(test)]
    fn call_worker_test_hook(&self) {
        let hook = self
            .worker_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(test)]
    pub(crate) fn set_result_test_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self
            .result_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(hook));
    }

    #[cfg(test)]
    fn call_result_test_hook(&self) {
        let hook = self
            .result_test_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }
}

pub struct RenderSuccess {
    pub output: RenderOutput,
    pub published: bool,
}

impl std::ops::Deref for RenderSuccess {
    type Target = RenderOutput;

    fn deref(&self) -> &Self::Target {
        &self.output
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderStage {
    Resolution,
    Render,
    Publication,
}

#[derive(Debug)]
pub struct RenderError {
    pub stage: RenderStage,
    pub message: String,
    pub cancelled: bool,
}

impl RenderError {
    fn new(stage: RenderStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            cancelled: false,
        }
    }

    fn cancelled(stage: RenderStage) -> Self {
        Self {
            stage,
            message: evil_synth::Error::RenderCancelled.to_string(),
            cancelled: true,
        }
    }

    fn from_render(stage: RenderStage, error: evil_render::Error) -> Self {
        fn is_cancellation(error: &evil_render::Error) -> bool {
            match error {
                evil_render::Error::Synth(evil_synth::Error::RenderCancelled)
                | evil_render::Error::Beats(evil_beats::Error::Synth(
                    evil_synth::Error::RenderCancelled,
                )) => true,
                evil_render::Error::InstrumentResolution { error, .. } => is_cancellation(error),
                _ => false,
            }
        }

        let cancelled = is_cancellation(&error);
        Self {
            stage,
            message: if cancelled {
                evil_synth::Error::RenderCancelled.to_string()
            } else {
                error.to_string()
            },
            cancelled,
        }
    }

    fn from_synth(stage: RenderStage, error: evil_synth::Error) -> Self {
        let cancelled = matches!(error, evil_synth::Error::RenderCancelled);
        Self {
            stage,
            message: error.to_string(),
            cancelled,
        }
    }
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RenderError {}

pub struct RenderJob {
    receiver: Receiver<Result<RenderSuccess, RenderError>>,
    snapshot: Arc<RenderSnapshot>,
    cancellation: RenderCancellation,
    owner: RenderOwner,
    generation: u64,
    resolving: Arc<AtomicBool>,
    cancelled_at: std::cell::Cell<Option<web_time::Instant>>,
    worker: Option<JoinHandle<()>>,
}

impl RenderJob {
    pub fn start_range(
        snapshot: Arc<RenderSnapshot>,
        start_tick: u64,
        end_tick: Option<u64>,
        owner: &RenderOwner,
    ) -> Self {
        Self::spawn(snapshot, owner, None, move |snapshot, cancellation| {
            evil_render::render_snapshot_range(snapshot, start_tick, end_tick, cancellation)
        })
    }

    pub fn start_wav(snapshot: Arc<RenderSnapshot>, output: PathBuf, owner: &RenderOwner) -> Self {
        Self::start_scoped_wav(snapshot, RenderScope::FullMix, output, owner)
    }

    pub fn start_range_wav(
        snapshot: Arc<RenderSnapshot>,
        start_tick: u64,
        end_tick: Option<u64>,
        output: PathBuf,
        owner: &RenderOwner,
    ) -> Self {
        Self::spawn(
            snapshot,
            owner,
            Some(output),
            move |snapshot, cancellation| {
                evil_render::render_snapshot_range(snapshot, start_tick, end_tick, cancellation)
            },
        )
    }

    pub fn start_scoped_wav(
        snapshot: Arc<RenderSnapshot>,
        scope: RenderScope,
        output: PathBuf,
        owner: &RenderOwner,
    ) -> Self {
        Self::spawn(
            snapshot,
            owner,
            Some(output),
            move |snapshot, cancellation| {
                evil_render::render_snapshot_scoped(snapshot, scope, cancellation)
            },
        )
    }

    fn spawn(
        snapshot: Arc<RenderSnapshot>,
        owner: &RenderOwner,
        output: Option<PathBuf>,
        render: impl FnOnce(
            &RenderSnapshot,
            &RenderCancellation,
        ) -> Result<RenderOutput, evil_render::Error>
        + Send
        + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancellation = RenderCancellation::default();
        let worker_cancellation = cancellation.clone();
        let worker_owner = owner.clone();
        let generation = owner.begin();
        let resolving = Arc::new(AtomicBool::new(!snapshot.is_resolved()));
        let worker_resolving = resolving.clone();
        let job_snapshot = snapshot.clone();
        owner.workers.fetch_add(1, Ordering::AcqRel);
        let worker = std::thread::spawn(move || {
            let _worker = WorkerGuard(worker_owner.clone());
            #[cfg(test)]
            worker_owner.call_worker_test_hook();
            let resolution = ResolutionGuard(worker_resolving);
            let result = snapshot
                .resolve(&worker_cancellation)
                .map_err(|error| RenderError::from_render(RenderStage::Resolution, error));
            resolution.finish();
            let result = result
                .and_then(|()| {
                    render(&snapshot, &worker_cancellation)
                        .map_err(|error| RenderError::from_render(RenderStage::Render, error))
                })
                .and_then(|rendered| {
                    let published = if let Some(output) = output {
                        publish_wav(
                            &rendered,
                            &output,
                            &worker_owner,
                            generation,
                            &worker_cancellation,
                        )?;
                        true
                    } else {
                        false
                    };
                    Ok(RenderSuccess {
                        output: rendered,
                        published,
                    })
                });
            #[cfg(test)]
            worker_owner.call_result_test_hook();
            let _ = sender.send(result);
        });
        Self {
            receiver,
            snapshot: job_snapshot,
            cancellation,
            owner: owner.clone(),
            generation,
            resolving,
            cancelled_at: std::cell::Cell::new(None),
            worker: Some(worker),
        }
    }

    pub fn cancel(&self) {
        if self.cancelled_at.get().is_none() {
            self.cancelled_at.set(Some(web_time::Instant::now()));
        }
        self.cancellation.cancel();
        self.owner.invalidate(self.generation);
    }

    pub fn is_resolving(&self) -> bool {
        self.resolving.load(Ordering::Acquire) || self.snapshot.is_resolving()
    }

    pub fn is_cancelling(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    #[cfg(test)]
    pub(crate) fn expire_cancellation_grace(&self) {
        self.cancelled_at
            .set(Some(web_time::Instant::now() - CANCELLATION_GRACE));
    }

    fn owns_publication(&self) -> bool {
        self.owner
            .publication_jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&self.generation)
    }

    pub fn detach_if_stalled(&mut self) -> bool {
        if self.worker.is_none()
            || self
                .cancelled_at
                .get()
                .is_none_or(|cancelled_at| cancelled_at.elapsed() < CANCELLATION_GRACE)
        {
            return false;
        }
        let Some(worker) = self.worker.take() else {
            return false;
        };
        Self::retire(worker);
        true
    }

    fn retire(worker: JoinHandle<()>) {
        let _ = std::thread::Builder::new()
            .name("midi-evil-render-retirement".to_owned())
            .spawn(move || {
                let _ = worker.join();
            });
    }

    pub fn try_finish(&mut self) -> Option<Result<RenderSuccess, RenderError>> {
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(RenderError::new(
                RenderStage::Render,
                "render worker disconnected",
            )),
        };
        if let Err(error) = self.join_worker() {
            return Some(Err(error));
        }
        Some(result)
    }

    fn finish_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Option<Result<RenderSuccess, RenderError>> {
        let result = match self.receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => return None,
            Err(RecvTimeoutError::Disconnected) => Err(RenderError::new(
                RenderStage::Render,
                "render worker disconnected",
            )),
        };
        if let Err(error) = self.join_worker() {
            return Some(Err(error));
        }
        Some(result)
    }

    fn join_worker(&mut self) -> Result<(), RenderError> {
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            return Err(RenderError::new(
                RenderStage::Render,
                "render worker panicked",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn wait(mut self) -> Result<RenderSuccess, RenderError> {
        let result = self
            .receiver
            .recv()
            .map_err(|error| RenderError::new(RenderStage::Render, error.to_string()))?;
        self.join_worker()?;
        result
    }

    #[cfg(test)]
    pub(crate) fn resolving_for_test() -> Self {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        Self {
            receiver,
            snapshot: Arc::new(RenderSnapshot::new(
                evil_project::Project::new("Resolution fixture"),
                PathBuf::from("."),
            )),
            cancellation: RenderCancellation::default(),
            owner: RenderOwner::default(),
            generation: 0,
            resolving: Arc::new(AtomicBool::new(true)),
            cancelled_at: std::cell::Cell::new(None),
            worker: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn cancelled_for_test() -> Self {
        let job = Self::resolving_for_test();
        job.resolving.store(false, Ordering::Release);
        job.cancel();
        job
    }
}

impl RenderJob {
    pub fn shutdown(mut jobs: Vec<Self>) {
        for job in &jobs {
            job.cancel();
        }
        let deadline = web_time::Instant::now() + SHUTDOWN_TIMEOUT;
        while !jobs.is_empty() && web_time::Instant::now() < deadline {
            jobs.retain_mut(|job| {
                if job
                    .finish_timeout(std::time::Duration::from_millis(10))
                    .is_some()
                {
                    return false;
                }
                if !job.owns_publication() {
                    job.detach_if_stalled();
                }
                true
            });
        }
    }
}

impl Drop for RenderJob {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            Self::retire(worker);
        }
    }
}

struct ResolutionGuard(Arc<AtomicBool>);

impl ResolutionGuard {
    fn finish(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Drop for ResolutionGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

struct WorkerGuard(RenderOwner);

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.workers.fetch_sub(1, Ordering::AcqRel);
    }
}

fn temporary_lifecycle() -> std::sync::MutexGuard<'static, HashSet<OsString>> {
    TEMPORARY_LIFECYCLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn is_job_temporary_name(name: &str) -> bool {
    let Some(identifier) = name
        .strip_prefix(".midi-evil-")
        .and_then(|suffix| suffix.strip_suffix(".tmp"))
    else {
        return false;
    };
    let mut parts = identifier.rsplitn(2, '-');
    parts.next().is_some_and(|sequence| {
        !sequence.is_empty() && sequence.chars().all(|character| character.is_ascii_digit())
    }) && parts.next().is_some_and(|process| {
        !process.is_empty() && process.chars().all(|character| character.is_ascii_digit())
    })
}

fn remove_stale_temporaries(output: &Path) {
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    {
        let mut swept = SWEPT_DIRECTORIES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !swept.insert(directory.to_path_buf()) {
            return;
        }
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let live = temporary_lifecycle().clone();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        if !name.to_str().is_some_and(is_job_temporary_name) || live.contains(&name) {
            continue;
        }
        let older_than_one_day = entry.metadata().is_ok_and(|metadata| {
            metadata.modified().is_ok_and(|modified| {
                modified
                    .elapsed()
                    .is_ok_and(|age| age > std::time::Duration::from_secs(24 * 60 * 60))
            })
        });
        if older_than_one_day {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

struct TemporaryOutput {
    path: PathBuf,
    retained: bool,
}

impl TemporaryOutput {
    fn create(output: &Path, _owner: &RenderOwner) -> Result<Self, String> {
        remove_stale_temporaries(output);
        #[cfg(test)]
        _owner.call_publication_test_hook(0, output, PublicationTestPoint::TemporaryCreate);
        output
            .file_name()
            .ok_or_else(|| format!("WAV output {} has no file name", output.display()))?;
        for _ in 0..128 {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path =
                output.with_file_name(format!(".midi-evil-{}-{sequence}.tmp", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    drop(file);
                    if let Some(name) = path.file_name() {
                        temporary_lifecycle().insert(name.to_os_string());
                    }
                    return Ok(Self {
                        path,
                        retained: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(format!(
                        "could not create temporary WAV beside {}: {error}",
                        output.display()
                    ));
                }
            }
        }
        Err(format!(
            "could not allocate a temporary WAV beside {}",
            output.display()
        ))
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.retained {
            let _ = std::fs::remove_file(&self.path);
        }
        if let Some(name) = self.path.file_name() {
            temporary_lifecycle().remove(name);
        }
    }
}

fn validate_destination_parent(output: &Path) -> Result<(), RenderError> {
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = std::fs::metadata(parent).map_err(|error| {
        RenderError::new(
            RenderStage::Publication,
            format!(
                "could not use WAV output directory {}: {error}",
                parent.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(RenderError::new(
            RenderStage::Publication,
            format!("WAV output parent {} is not a directory", parent.display()),
        ));
    }
    Ok(())
}

fn publish_wav(
    rendered: &RenderOutput,
    output: &Path,
    owner: &RenderOwner,
    generation: u64,
    cancellation: &RenderCancellation,
) -> Result<(), RenderError> {
    cancellation
        .check()
        .map_err(|error| RenderError::from_synth(RenderStage::Publication, error))?;
    if !owner.is_current(generation) {
        return Err(RenderError::cancelled(RenderStage::Publication));
    }
    let existing = match std::fs::symlink_metadata(output) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(RenderError::new(
                RenderStage::Publication,
                format!("could not inspect WAV output {}: {error}", output.display()),
            ));
        }
    };
    if existing {
        let _publication = owner.publication_lock(generation, output, cancellation)?;
        #[cfg(test)]
        owner.call_publication_test_hook(generation, output, PublicationTestPoint::BeforeWrite);
        cancellation
            .check()
            .map_err(|error| RenderError::from_synth(RenderStage::Publication, error))?;
        if !owner.is_current(generation) {
            return Err(RenderError::cancelled(RenderStage::Publication));
        }
        return rendered.sound.write_wav(output).map_err(|error| {
            RenderError::new(
                RenderStage::Publication,
                format!(
                    "could not write WAV {}; destination may contain a partial replacement: {error}",
                    output.display()
                ),
            )
        });
    }

    let output = destination_identity(output);
    validate_destination_parent(&output)?;
    let mut temporary = TemporaryOutput::create(&output, owner)
        .map_err(|error| RenderError::new(RenderStage::Publication, error))?;
    #[cfg(test)]
    owner.call_publication_test_hook(generation, &output, PublicationTestPoint::BeforeWrite);
    rendered
        .sound
        .write_wav_with_cancellation(&temporary.path, cancellation)
        .map_err(|error| RenderError::from_synth(RenderStage::Publication, error))?;
    cancellation
        .check()
        .map_err(|error| RenderError::from_synth(RenderStage::Publication, error))?;
    let _publication = owner.publication_lock(generation, &output, cancellation)?;
    cancellation
        .check()
        .map_err(|error| RenderError::from_synth(RenderStage::Publication, error))?;
    if !owner.is_current(generation) {
        return Err(RenderError::cancelled(RenderStage::Publication));
    }
    #[cfg(test)]
    owner.call_publication_test_hook(generation, &output, PublicationTestPoint::BeforeRename);
    std::fs::hard_link(&temporary.path, &output).map_err(|error| {
        RenderError::new(
            RenderStage::Publication,
            format!(
                "could not publish new WAV {} without replacing an existing destination: {error}",
                output.display()
            ),
        )
    })?;
    std::fs::remove_file(&temporary.path).map_err(|error| {
        RenderError::new(
            RenderStage::Publication,
            format!(
                "could not remove published temporary WAV {}: {error}",
                temporary.path.display()
            ),
        )
    })?;
    temporary.retained = true;
    Ok(())
}

/// Remaining scoped exports and the immutable project/instrument snapshot shared by every job.
pub struct WavExportQueue {
    pub output_directory: PathBuf,
    snapshot: Arc<RenderSnapshot>,
    remaining: VecDeque<ExportSpec>,
    pub completed: usize,
    pub total: usize,
    pub skipped: usize,
}

impl WavExportQueue {
    pub fn new(output_directory: PathBuf, plan: ExportPlan, snapshot: Arc<RenderSnapshot>) -> Self {
        Self {
            output_directory,
            snapshot,
            total: plan.exports.len(),
            remaining: plan.exports.into(),
            completed: 0,
            skipped: plan.skipped,
        }
    }

    pub fn next_spec(&mut self) -> Option<ExportSpec> {
        self.remaining.pop_front()
    }

    pub fn complete_current(&mut self) {
        self.completed += 1;
    }

    pub fn output_for(&self, spec: &ExportSpec) -> PathBuf {
        self.output_directory.join(&spec.file_name)
    }

    pub fn snapshot(&self) -> Arc<RenderSnapshot> {
        self.snapshot.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use evil_project::{Note, Part, Pattern, Project, Track};

    use super::*;

    fn project() -> Project {
        let mut project = Project::new("Background");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project
            .add_track(Track {
                name: "Muted harmony".to_owned(),
                notes: vec![Note {
                    pitch: 67,
                    ..Note::default()
                }],
                muted: true,
                ..Track::default()
            })
            .unwrap();
        project
    }

    #[test]
    fn worker_renders_a_project_without_blocking_the_caller() {
        let job = RenderJob::start_range(
            Arc::new(RenderSnapshot::new(project(), PathBuf::from("."))),
            0,
            None,
            &RenderOwner::default(),
        );
        assert_eq!(job.wait().unwrap().note_count, 1);
    }

    #[test]
    fn resolved_snapshot_does_not_report_spurious_resolution() {
        let snapshot = Arc::new(RenderSnapshot::new(project(), PathBuf::from(".")));
        snapshot.resolve(&RenderCancellation::default()).unwrap();
        let job = RenderJob::start_range(snapshot, 0, None, &RenderOwner::default());
        assert!(!job.is_resolving());
        assert_eq!(job.wait().unwrap().note_count, 1);
    }

    #[test]
    fn scoped_wav_worker_unmutes_only_the_selection_and_writes_pcm16_stereo() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("02-muted-harmony.wav");
        let job = RenderJob::start_scoped_wav(
            Arc::new(RenderSnapshot::with_scopes(
                project(),
                PathBuf::from("."),
                vec![RenderScope::Track { track_index: 1 }],
            )),
            RenderScope::Track { track_index: 1 },
            output.clone(),
            &RenderOwner::default(),
        );
        let rendered = job.wait().unwrap();
        assert_eq!(rendered.note_count, 1);
        assert_eq!(rendered.track_count, 1);
        assert_eq!(rendered.channel_meters[0].0, 1);
        let wav = std::fs::read(&output).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        let data = wav.windows(4).position(|chunk| chunk == b"data").unwrap();
        let data_bytes = u32::from_le_bytes(wav[data + 4..data + 8].try_into().unwrap());
        assert_eq!(data_bytes as usize, rendered.sound.left.len() * 4);
    }

    #[test]
    fn native_parity_full_and_section_wav_exports_match_the_full_mix_crop() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project();
        project.tracks[0].notes[0].duration_ticks = 1_920;
        let full_path = directory.path().join("full.wav");
        let section_path = directory.path().join("section.wav");
        let full = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(project.clone(), PathBuf::from("."))),
            full_path.clone(),
            &RenderOwner::default(),
        )
        .wait()
        .unwrap();
        let section = RenderJob::start_range_wav(
            Arc::new(RenderSnapshot::new(project, PathBuf::from("."))),
            480,
            Some(960),
            section_path.clone(),
            &RenderOwner::default(),
        )
        .wait()
        .unwrap();
        assert_eq!(section.sound.left.len(), 24_000);
        assert_eq!(section.sound.right.len(), 24_000);
        assert_eq!(section.sound.left, full.sound.left[24_000..48_000]);
        assert_eq!(section.sound.right, full.sound.right[24_000..48_000]);

        for (path, expected) in [(&full_path, full.sound.left.len()), (&section_path, 24_000)] {
            let mut reader = hound::WavReader::open(path).unwrap();
            assert_eq!(reader.spec().channels, 2);
            assert_eq!(reader.spec().sample_rate, 48_000);
            assert_eq!(reader.spec().bits_per_sample, 16);
            assert_eq!(reader.samples::<i16>().count(), expected * 2);
        }
        let section_bytes = std::fs::read(&section_path).unwrap();
        RenderJob::start_range_wav(
            Arc::new(RenderSnapshot::new(
                {
                    let mut rerun = self::project();
                    rerun.tracks[0].notes[0].duration_ticks = 1_920;
                    rerun
                },
                PathBuf::from("."),
            )),
            480,
            Some(960),
            section_path.clone(),
            &RenderOwner::default(),
        )
        .wait()
        .unwrap();
        assert_eq!(std::fs::read(&section_path).unwrap(), section_bytes);
    }

    #[test]
    fn native_parity_desktop_save_midi_stem_and_part_export_workflow() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = Project::new("Desktop release workflow");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let pattern = project
            .add_pattern(Pattern {
                name: "Verse".to_owned(),
                length_ticks: 480,
                notes: vec![Note {
                    pitch: 67,
                    ..Note::default()
                }],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 960,
                    length_ticks: 480,
                },
            )
            .unwrap();

        let project_path = directory.path().join("workflow.evil");
        project.save(&project_path).unwrap();
        let loaded = Project::load(&project_path).unwrap();
        assert_eq!(loaded, project);

        let midi_path = directory.path().join("workflow.mid");
        let midi_report =
            evil_exports::export_native_project_to_path(&loaded, directory.path(), &midi_path)
                .unwrap();
        assert_eq!(midi_report.note_count, 2);
        let imported = evil_imports::import_midi(&std::fs::read(&midi_path).unwrap()).unwrap();
        assert_eq!(imported.note_count(), 2);
        assert_eq!(imported.tracks[0].name, "Lead");

        let plans = [
            (
                "stems",
                evil_render::stem_export_plan(&loaded, None).unwrap(),
                "01-lead.wav",
                2,
            ),
            (
                "parts",
                evil_render::part_export_plan(&loaded, None, None).unwrap(),
                "01-lead__part-0001-verse.wav",
                1,
            ),
        ];
        for (name, plan, expected_file, expected_notes) in plans {
            let output_directory = directory.path().join(name);
            std::fs::create_dir(&output_directory).unwrap();
            let mut queue = WavExportQueue::new(
                output_directory,
                plan,
                Arc::new(RenderSnapshot::new(loaded.clone(), PathBuf::from("."))),
            );
            assert_eq!(queue.total, 1);
            let spec = queue.next_spec().unwrap();
            assert_eq!(spec.file_name, expected_file);
            let output = queue.output_for(&spec);
            let rendered = RenderJob::start_scoped_wav(
                queue.snapshot(),
                spec.scope,
                output.clone(),
                &RenderOwner::default(),
            )
            .wait()
            .unwrap();
            assert_eq!(rendered.note_count, expected_notes);
            let mut wav = hound::WavReader::open(output).unwrap();
            assert_eq!(wav.spec().channels, 2);
            assert_eq!(wav.spec().bits_per_sample, 16);
            assert_eq!(wav.samples::<i16>().count(), rendered.sound.left.len() * 2);
            queue.complete_current();
            assert!(queue.next_spec().is_none());
            assert_eq!(queue.completed, 1);
        }
    }

    #[test]
    fn export_queue_shares_one_snapshot_and_advances_sequentially() {
        let specs = vec![
            ExportSpec {
                scope: RenderScope::Track { track_index: 0 },
                file_name: "01-lead.wav".to_owned(),
            },
            ExportSpec {
                scope: RenderScope::TrackPart {
                    track_index: 0,
                    part_index: 2,
                },
                file_name: "01-lead__part-0003-verse.wav".to_owned(),
            },
        ];
        let mut queue = WavExportQueue::new(
            PathBuf::from("exports"),
            ExportPlan {
                exports: specs,
                skipped: 1,
            },
            Arc::new(RenderSnapshot::new(project(), PathBuf::from("."))),
        );
        assert!(!queue.is_empty());
        assert_eq!(queue.total, 2);
        assert_eq!(queue.skipped, 1);
        let first = queue.next_spec().unwrap();
        assert_eq!(queue.completed, 0);
        assert_eq!(
            queue.output_for(&first),
            Path::new("exports").join("01-lead.wav")
        );
        queue.complete_current();
        let second = queue.next_spec().unwrap();
        assert_eq!(queue.completed, 1);
        assert!(matches!(
            second.scope,
            RenderScope::TrackPart { part_index: 2, .. }
        ));
        queue.complete_current();
        assert!(queue.next_spec().is_none());
        assert_eq!(queue.completed, 2);
    }

    fn long_project() -> Project {
        let mut project = Project::new("Cancellation");
        for index in 0..16 {
            project
                .add_track(Track {
                    name: format!("Long {index}"),
                    notes: vec![Note {
                        duration_ticks: 28_800,
                        ..Note::default()
                    }],
                    ..Track::default()
                })
                .unwrap();
        }
        project
    }

    fn expensive_project() -> Project {
        let mut project = Project::new("One expensive track");
        project
            .add_track(Track {
                notes: vec![Note {
                    duration_ticks: 50_000,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        project
    }

    fn is_test_job_temporary(name: &str) -> bool {
        let Some(suffix) = name.strip_prefix(".midi-evil-") else {
            return false;
        };
        let Some(suffix) = suffix.strip_suffix(".tmp") else {
            return false;
        };
        let mut parts = suffix.rsplitn(2, '-');
        let valid_number = |part: &str| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        };
        parts.next().is_some_and(&valid_number) && parts.next().is_some_and(valid_number)
    }

    fn assert_no_job_temporaries(directory: &Path) {
        let names: Vec<_> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !names.iter().any(|name| is_test_job_temporary(name)),
            "orphan job temporaries: {names:?}"
        );
    }

    fn assert_stereo_wav(path: &Path) {
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        let samples: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>().unwrap();
        assert!(
            samples.chunks_exact(2).any(|frame| frame[0] != frame[1]),
            "{} was downmixed",
            path.display()
        );
    }

    #[test]
    fn stereo_drum_full_section_stem_and_part_exports_keep_distinct_channels() {
        let directory = tempfile::tempdir().unwrap();
        let mut kit = evil_beats::DrumKit::empty("Stereo export");
        let mut pad = evil_beats::KitPad::new(evil_beats::DrumPad::new_stereo(
            vec![0.0, 0.5, -0.5, 0.25],
            vec![0.0, -0.25, 0.75, -0.5],
            48_000,
        ));
        pad.gate_seconds = Some(0.01);
        pad.voice.envelope.release_seconds = 0.0;
        kit.pads[0] = Some(pad);
        std::fs::write(
            directory.path().join("stereo.evilkit"),
            kit.to_json().unwrap(),
        )
        .unwrap();

        let mut project = Project::new("Stereo drums");
        let instrument = project
            .add_instrument(evil_project::Instrument::drum_kit(
                "Drums",
                "stereo.evilkit",
            ))
            .unwrap();
        project
            .add_track(Track {
                name: "Drums".to_owned(),
                instrument,
                notes: vec![Note {
                    pitch: 36,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let pattern = project
            .add_pattern(Pattern {
                name: "Beat".to_owned(),
                length_ticks: 480,
                notes: vec![Note {
                    pitch: 36,
                    ..Note::default()
                }],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 480,
                    length_ticks: 480,
                },
            )
            .unwrap();
        let snapshot = || {
            Arc::new(RenderSnapshot::new(
                project.clone(),
                directory.path().to_path_buf(),
            ))
        };
        let owner = RenderOwner::default();
        let full = directory.path().join("full.wav");
        RenderJob::start_wav(snapshot(), full.clone(), &owner)
            .wait()
            .unwrap();
        let section = directory.path().join("section.wav");
        RenderJob::start_range_wav(snapshot(), 0, Some(480), section.clone(), &owner)
            .wait()
            .unwrap();
        assert_stereo_wav(&full);
        assert_stereo_wav(&section);

        let plans = [
            evil_render::stem_export_plan(&project, None).unwrap(),
            evil_render::part_export_plan(&project, None, None).unwrap(),
        ];
        for plan in plans {
            let mut queue = WavExportQueue::new(directory.path().to_path_buf(), plan, snapshot());
            while let Some(spec) = queue.next_spec() {
                let output = queue.output_for(&spec);
                RenderJob::start_scoped_wav(queue.snapshot(), spec.scope, output.clone(), &owner)
                    .wait()
                    .unwrap();
                assert_stereo_wav(&output);
                queue.complete_current();
            }
        }
        assert_no_job_temporaries(directory.path());
        assert_eq!(owner.workers.load(Ordering::Acquire), 0);
    }

    #[test]
    fn cancellation_stops_real_synthesis_without_replacing_output_or_leaking_workers() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("cancel.wav");
        std::fs::write(&output, b"original").unwrap();
        let owner = RenderOwner::default();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_worker_test_hook(move || {
            entered_sender.send(()).unwrap();
            let (lock, condition) = &*gate_for_hook;
            let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*allowed {
                allowed = condition
                    .wait(allowed)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        });
        let snapshot = Arc::new(RenderSnapshot::new(expensive_project(), PathBuf::from(".")));
        let job = RenderJob::start_wav(snapshot.clone(), output.clone(), &owner);
        entered_receiver.recv().unwrap();
        job.cancel();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let error = match job.wait() {
            Ok(_) => panic!("cancelled render unexpectedly completed"),
            Err(error) => error,
        };
        assert!(error.cancelled, "{error}");
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
        assert_no_job_temporaries(directory.path());
        assert_eq!(owner.workers.load(Ordering::Acquire), 0);
        assert_eq!(Arc::strong_count(&snapshot), 1);
    }

    #[test]
    fn cancel_before_existing_write_commit_preserves_original() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("before-commit.wav");
        std::fs::write(&output, b"original").unwrap();
        let owner = RenderOwner::default();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_publication_test_hook(&output, move |_, point| {
            if point == PublicationTestPoint::BeforeWrite {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let job = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(expensive_project(), PathBuf::from("."))),
            output.clone(),
            &owner,
        );
        entered_receiver.recv().unwrap();
        job.cancel();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let error = match job.wait() {
            Ok(_) => panic!("cancellation before commit unexpectedly published"),
            Err(error) => error,
        };
        assert!(error.cancelled);
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
        assert_no_job_temporaries(directory.path());
    }

    #[test]
    fn cancel_after_existing_write_commit_completes_published_file() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("writing.wav");
        std::fs::write(&output, b"original").unwrap();
        #[cfg(unix)]
        {
            std::fs::set_permissions(&output, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                .unwrap();
        }
        #[cfg(unix)]
        let expected_group =
            std::os::unix::fs::MetadataExt::gid(&std::fs::metadata(&output).unwrap());
        let owner = RenderOwner::default();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_result_test_hook(move || {
            entered_sender.send(()).unwrap();
            let (lock, condition) = &*gate_for_hook;
            let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*allowed {
                allowed = condition
                    .wait(allowed)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        });
        let job = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(expensive_project(), PathBuf::from("."))),
            output.clone(),
            &owner,
        );
        entered_receiver.recv().unwrap();
        assert!(output.exists());
        job.cancel();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let rendered = job.wait().unwrap();
        assert!(rendered.published);
        assert!(output.exists());
        assert_ne!(std::fs::read(&output).unwrap(), b"original");
        assert_eq!(&std::fs::read(&output).unwrap()[0..4], b"RIFF");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let metadata = std::fs::metadata(&output).unwrap();
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            assert_eq!(metadata.gid(), expected_group);
        }
        assert_no_job_temporaries(directory.path());
        assert_eq!(owner.worker_count(), 0);
    }

    #[test]
    fn repeated_cancel_retry_keeps_one_worker_and_only_the_retry_publishes() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("retry.wav");
        let owner = RenderOwner::default();
        let mut previous = None;
        for index in 0..4 {
            let cancelled_snapshot =
                Arc::new(RenderSnapshot::new(long_project(), PathBuf::from(".")));
            let cancelled =
                RenderJob::start_wav(cancelled_snapshot.clone(), output.clone(), &owner);
            cancelled.cancel();
            assert!(cancelled.wait().is_err());
            assert_eq!(owner.workers.load(Ordering::Acquire), 0);
            assert_eq!(Arc::strong_count(&cancelled_snapshot), 1);
            if let Some(previous) = &previous {
                assert_eq!(&std::fs::read(&output).unwrap(), previous);
            } else {
                assert!(!output.exists());
            }
            assert_no_job_temporaries(directory.path());

            let mut retry_project = project();
            retry_project.tracks[0].notes[0].pitch = 60 + index;
            let retry = RenderJob::start_wav(
                Arc::new(RenderSnapshot::new(retry_project, PathBuf::from("."))),
                output.clone(),
                &owner,
            );
            retry.wait().unwrap();
            assert_eq!(owner.workers.load(Ordering::Acquire), 0);
            let bytes = std::fs::read(&output).unwrap();
            assert_ne!(Some(&bytes), previous.as_ref());
            previous = Some(bytes);
        }
        assert_no_job_temporaries(directory.path());
    }

    #[test]
    fn stalled_destination_identity_does_not_hold_the_publication_map_lock() {
        let directory = tempfile::tempdir().unwrap();
        let first_output = directory.path().join("first.wav");
        let second_output = directory.path().join("second.wav");
        let owner = RenderOwner::default();
        let first_identity = Arc::new(AtomicBool::new(false));
        let first_identity_for_hook = first_identity.clone();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_identity_test_hook(move || {
            if !first_identity_for_hook.swap(true, Ordering::AcqRel) {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let first_owner = owner.clone();
        let first = std::thread::spawn(move || {
            first_owner.publication_lock(0, &first_output, &RenderCancellation::default())
        });
        entered_receiver.recv().unwrap();
        let second = owner
            .publication_lock(0, &second_output, &RenderCancellation::default())
            .unwrap();
        drop(second);
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        drop(first.join().unwrap().unwrap());
    }

    #[test]
    fn publication_lock_waits_are_per_destination_and_cancellable() {
        let directory = tempfile::tempdir().unwrap();
        let first_output = directory.path().join("first.wav");
        let second_output = directory.path().join("second.wav");
        let owner = RenderOwner::default();
        let first_cancellation = RenderCancellation::default();
        let first = owner
            .publication_lock(0, &first_output, &first_cancellation)
            .unwrap();
        let waiting_cancellation = RenderCancellation::default();
        let waiting_owner = owner.clone();
        let waiting_output = first_output.clone();
        let waiting_token = waiting_cancellation.clone();
        let (waiting_sender, waiting_receiver) = mpsc::channel();
        let waiting = std::thread::spawn(move || {
            waiting_sender.send(()).unwrap();
            waiting_owner.publication_lock(0, &waiting_output, &waiting_token)
        });
        waiting_receiver.recv().unwrap();

        let second_owner = owner.clone();
        let second_token = RenderCancellation::default();
        let (second_sender, second_receiver) = mpsc::channel();
        let second = std::thread::spawn(move || {
            let guard = second_owner
                .publication_lock(0, &second_output, &second_token)
                .unwrap();
            second_sender.send(()).unwrap();
            drop(guard);
        });
        second_receiver.recv().unwrap();
        second.join().unwrap();
        waiting_cancellation.cancel();
        let Err(error) = waiting.join().unwrap() else {
            panic!("cancelled publication wait acquired its lock");
        };
        assert!(error.cancelled);
        drop(first);
    }

    #[test]
    fn temporary_create_and_rename_do_not_wait_for_lifecycle_bookkeeping() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("create.wav");
        let owner = RenderOwner::default();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (create_sender, create_receiver) = mpsc::channel();
        owner.set_publication_test_hook(&output, move |generation, point| {
            if generation == 0 && point == PublicationTestPoint::TemporaryCreate {
                create_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let output_for_worker = output.clone();
        let owner_for_worker = owner.clone();
        let create = std::thread::spawn(move || {
            TemporaryOutput::create(&output_for_worker, &owner_for_worker)
        });
        create_receiver.recv().unwrap();
        let lifecycle = temporary_lifecycle();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !directory.path().read_dir().unwrap().flatten().any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(is_test_job_temporary)
        }) && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        assert!(directory.path().read_dir().unwrap().flatten().any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(is_test_job_temporary)
        }));
        drop(lifecycle);
        let temporary = create.join().unwrap().unwrap();
        drop(temporary);
        assert_no_job_temporaries(directory.path());

        let generation = owner.begin();
        let cancellation = RenderCancellation::default();
        let publication = owner.publication_lock(0, &output, &cancellation).unwrap();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_publication_test_hook(&output, move |hook_generation, point| {
            if hook_generation == generation && point == PublicationTestPoint::BeforeLock {
                entered_sender.send(()).unwrap();
            }
        });
        let worker_owner = owner.clone();
        let worker_cancellation = cancellation.clone();
        let output_for_worker = output.clone();
        let publish = std::thread::spawn(move || {
            publish_wav(
                &rendered,
                &output_for_worker,
                &worker_owner,
                generation,
                &worker_cancellation,
            )
        });
        entered_receiver.recv().unwrap();
        let lifecycle = temporary_lifecycle();
        drop(publication);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !output.exists() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(output.exists());
        drop(lifecycle);
        publish.join().unwrap().unwrap();
        assert_no_job_temporaries(directory.path());
    }

    #[test]
    fn drum_cancellation_is_typed_as_cancellation() {
        let error = RenderError::from_render(
            RenderStage::Render,
            evil_render::Error::Beats(evil_beats::Error::Synth(evil_synth::Error::RenderCancelled)),
        );
        assert!(error.cancelled);
        assert_eq!(error.stage, RenderStage::Render);
        assert_eq!(error.message, "render cancelled");
    }

    #[test]
    fn older_stalled_rename_cannot_overwrite_newer_generation_output() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("shared.wav");
        let owner = RenderOwner::default();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let gate_for_hook = gate.clone();
        let older_generation = owner.begin();
        owner.set_publication_test_hook(&output, move |generation, point| {
            if generation == older_generation && point == PublicationTestPoint::BeforeLock {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let mut older_project = project();
        older_project.tracks[0].notes[0].pitch = 60;
        let older = evil_render::render_project(&older_project, Path::new(".")).unwrap();
        let older_owner = owner.clone();
        let older_cancellation = RenderCancellation::default();
        let output_for_older = output.clone();
        let older = std::thread::spawn(move || {
            publish_wav(
                &older,
                &output_for_older,
                &older_owner,
                older_generation,
                &older_cancellation,
            )
        });
        entered_receiver.recv().unwrap();

        let mut newer_project = project();
        newer_project.tracks[0].notes[0].pitch = 72;
        let newer = evil_render::render_project(&newer_project, Path::new(".")).unwrap();
        let newer_generation = owner.begin();
        let newer_owner = owner.clone();
        let newer_cancellation = RenderCancellation::default();
        let output_for_newer = output.clone();
        let newer = std::thread::spawn(move || {
            publish_wav(
                &newer,
                &output_for_newer,
                &newer_owner,
                newer_generation,
                &newer_cancellation,
            )
        });
        newer.join().unwrap().unwrap();
        let newer_bytes = std::fs::read(&output).unwrap();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        assert!(older.join().unwrap().is_err());
        assert_eq!(std::fs::read(output).unwrap(), newer_bytes);
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn directory_alias_shares_publication_lock_after_final_generation_check() {
        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let real_output = real.join("shared.wav");
        let alias_output = alias.join("shared.wav");
        assert_eq!(
            destination_identity(&real_output),
            destination_identity(&alias_output)
        );
        let owner = RenderOwner::default();
        let older_generation = owner.begin();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let newer_gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (older_sender, older_receiver) = mpsc::channel();
        let (newer_sender, newer_receiver) = mpsc::channel();
        let gate_for_hook = gate.clone();
        let newer_gate_for_hook = newer_gate.clone();
        owner.set_publication_test_hook(&real_output, move |generation, point| {
            if generation == older_generation && point == PublicationTestPoint::BeforeRename {
                older_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            } else if generation != older_generation && point == PublicationTestPoint::BeforeLock {
                newer_sender.send(()).unwrap();
            } else if generation != older_generation && point == PublicationTestPoint::BeforeRename
            {
                newer_sender.send(()).unwrap();
                let (lock, condition) = &*newer_gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let mut older_project = project();
        older_project.tracks[0].notes[0].pitch = 60;
        let older = evil_render::render_project(&older_project, Path::new(".")).unwrap();
        let older_owner = owner.clone();
        let older_cancellation = RenderCancellation::default();
        let output_for_older = real_output.clone();
        let older = std::thread::spawn(move || {
            publish_wav(
                &older,
                &output_for_older,
                &older_owner,
                older_generation,
                &older_cancellation,
            )
        });
        older_receiver.recv().unwrap();

        let mut newer_project = project();
        newer_project.tracks[0].notes[0].pitch = 72;
        newer_project.tracks[0].notes[0].duration_ticks = 960;
        let newer = evil_render::render_project(&newer_project, Path::new(".")).unwrap();
        let newer_generation = owner.begin();
        let newer_owner = owner.clone();
        let newer_cancellation = RenderCancellation::default();
        let newer = std::thread::spawn(move || {
            publish_wav(
                &newer,
                &alias_output,
                &newer_owner,
                newer_generation,
                &newer_cancellation,
            )
        });
        newer_receiver.recv().unwrap();
        assert_eq!(
            owner
                .publication
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            1
        );
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        older.join().unwrap().unwrap();
        newer_receiver.recv().unwrap();
        let older_bytes = std::fs::read(&real_output).unwrap();
        let (lock, condition) = &*newer_gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        assert!(newer.join().unwrap().is_err());
        let newer_bytes = std::fs::read(&real_output).unwrap();
        assert_eq!(older_bytes, newer_bytes);
        assert_no_job_temporaries(&real);
    }

    #[test]
    fn reversed_completion_cannot_publish_a_stale_generation_over_a_newer_wav() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("generation.wav");
        let owner = RenderOwner::default();
        let stale_generation = owner.begin();
        let current_generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
        let cancellation = RenderCancellation::default();
        publish_wav(
            &rendered,
            &output,
            &owner,
            current_generation,
            &cancellation,
        )
        .unwrap();
        let current = std::fs::read(&output).unwrap();
        assert!(publish_wav(&rendered, &output, &owner, stale_generation, &cancellation,).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), current);
        assert_no_job_temporaries(directory.path());
    }

    #[test]
    fn near_component_limit_output_name_uses_short_job_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join(format!("{}.wav", "a".repeat(240)));
        RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(project(), PathBuf::from("."))),
            output.clone(),
            &RenderOwner::default(),
        )
        .wait()
        .unwrap();
        assert!(output.exists());
        assert_no_job_temporaries(directory.path());
    }

    #[test]
    fn export_start_removes_only_recognized_stale_temporaries_and_preserves_live_ownership() {
        let directory = tempfile::tempdir().unwrap();
        let stale = directory.path().join(".old.midi-evil-1-9.tmp");
        let short_stale = directory.path().join(".midi-evil-1-8.tmp");
        let unrelated = directory.path().join("keep.txt");
        let malformed = directory.path().join(".old.midi-evil-x-9.tmp");
        let not_hidden = directory.path().join("old.midi-evil-1-9.tmp");
        let recent = directory.path().join(".recent.midi-evil-999999-1.tmp");
        std::fs::write(&stale, b"stale").unwrap();
        std::fs::write(&short_stale, b"stale").unwrap();
        for path in [&stale, &short_stale] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(
                    std::time::SystemTime::now() - std::time::Duration::from_secs(25 * 60 * 60),
                )
                .unwrap();
        }
        std::fs::write(&recent, b"recent").unwrap();
        std::fs::write(&unrelated, b"keep").unwrap();
        std::fs::write(&malformed, b"keep").unwrap();
        std::fs::write(&not_hidden, b"keep").unwrap();
        let active_output = directory.path().join("active.wav");
        let active = TemporaryOutput::create(&active_output, &RenderOwner::default()).unwrap();
        let active_name = active.path.file_name().unwrap().to_str().unwrap();
        assert!(is_test_job_temporary(active_name));
        assert!(is_job_temporary_name(active_name));
        remove_stale_temporaries(&active_output);
        assert!(active.path.exists());
        drop(active);
        assert!(!active_output.exists());

        let output = directory.path().join("song.wav");
        RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(project(), PathBuf::from("."))),
            output,
            &RenderOwner::default(),
        )
        .wait()
        .unwrap();
        assert_eq!(std::fs::read(&stale).unwrap(), b"stale");
        assert!(!short_stale.exists());
        assert_eq!(std::fs::read(&recent).unwrap(), b"recent");
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"keep");
        assert_eq!(std::fs::read(&malformed).unwrap(), b"keep");
        assert_eq!(std::fs::read(&not_hidden).unwrap(), b"keep");
        std::fs::remove_file(recent).unwrap();
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn existing_destination_symlink_is_preserved_while_its_target_is_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.wav");
        let link = directory.path().join("alias.wav");
        std::fs::write(&target, b"original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();

        publish_wav(
            &rendered,
            &link,
            &owner,
            generation,
            &RenderCancellation::default(),
        )
        .unwrap();

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read(&link).unwrap(),
            std::fs::read(&target).unwrap()
        );
        assert_eq!(&std::fs::read(&target).unwrap()[0..4], b"RIFF");
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn chained_destination_symlinks_are_preserved_by_direct_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.wav");
        let second = directory.path().join("second.wav");
        let first = directory.path().join("first.wav");
        std::fs::write(&target, b"original").unwrap();
        std::os::unix::fs::symlink(&target, &second).unwrap();
        std::os::unix::fs::symlink(&second, &first).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();

        publish_wav(
            &rendered,
            &first,
            &owner,
            generation,
            &RenderCancellation::default(),
        )
        .unwrap();

        for link in [&first, &second] {
            assert!(
                std::fs::symlink_metadata(link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
        assert_eq!(&std::fs::read(&target).unwrap()[0..4], b"RIFF");
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn relative_and_absolute_dangling_symlinks_publish_to_their_targets() {
        for absolute in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let name = if absolute {
                "absolute.wav"
            } else {
                "relative.wav"
            };
            let target = directory.path().join(name);
            let link = directory.path().join("alias.wav");
            if absolute {
                std::os::unix::fs::symlink(&target, &link).unwrap();
            } else {
                std::os::unix::fs::symlink(name, &link).unwrap();
            }
            let owner = RenderOwner::default();
            let generation = owner.begin();
            let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();

            publish_wav(
                &rendered,
                &link,
                &owner,
                generation,
                &RenderCancellation::default(),
            )
            .unwrap();

            assert!(
                std::fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(&std::fs::read(&target).unwrap()[0..4], b"RIFF");
            assert_no_job_temporaries(directory.path());
        }
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_retry_through_two_link_dangling_chain_preserves_links_and_new_output() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.wav");
        let second = directory.path().join("second.wav");
        let first = directory.path().join("first.wav");
        std::os::unix::fs::symlink(&target, &second).unwrap();
        std::os::unix::fs::symlink(&second, &first).unwrap();
        assert_eq!(destination_identity(&first), target);
        let owner = RenderOwner::default();
        let first_write = Arc::new(AtomicBool::new(true));
        let first_write_for_hook = first_write.clone();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = mpsc::channel();
        owner.set_publication_test_hook(&first, move |_, point| {
            if point == PublicationTestPoint::BeforeWrite
                && first_write_for_hook.swap(false, Ordering::AcqRel)
            {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        let older = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(expensive_project(), PathBuf::from("."))),
            first.clone(),
            &owner,
        );
        entered_receiver.recv().unwrap();
        older.cancel();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        assert!(older.wait().is_err());
        assert!(!target.exists());

        let mut retry_project = project();
        retry_project.tracks[0].notes[0].duration_ticks = 960;
        let retry = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(retry_project, PathBuf::from("."))),
            first.clone(),
            &owner,
        );
        assert!(retry.wait().unwrap().published);
        for link in [&first, &second] {
            assert!(
                std::fs::symlink_metadata(link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
        assert_eq!(&std::fs::read(&target).unwrap()[0..4], b"RIFF");
        assert_eq!(destination_identity(&first), target);
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cycles_have_a_bounded_stable_identity() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.wav");
        let second = directory.path().join("second.wav");
        std::os::unix::fs::symlink(&second, &first).unwrap();
        std::os::unix::fs::symlink(&first, &second).unwrap();
        assert_eq!(destination_identity(&first), destination_identity(&first));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_with_a_missing_target_parent_fails_without_replacing_the_link() {
        let directory = tempfile::tempdir().unwrap();
        let link = directory.path().join("alias.wav");
        std::os::unix::fs::symlink("missing/target.wav", &link).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();

        let error = publish_wav(
            &rendered,
            &link,
            &owner,
            generation,
            &RenderCancellation::default(),
        )
        .unwrap_err();

        assert!(error.message.contains("could not write WAV"), "{error}");
        assert!(error.message.contains("alias.wav"), "{error}");
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn read_only_destination_fails_effective_access_check_without_temporary() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("read-only.wav");
        std::fs::write(&output, b"original").unwrap();
        std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o444)).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
        let error = publish_wav(
            &rendered,
            &output,
            &owner,
            generation,
            &RenderCancellation::default(),
        )
        .unwrap_err();
        assert!(!error.cancelled);
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn existing_output_owner_group_and_mode_survive_direct_overwrite() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("metadata.wav");
        std::fs::write(&output, b"original").unwrap();
        std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o640)).unwrap();
        let original = std::fs::metadata(&output).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();

        publish_wav(
            &rendered,
            &output,
            &owner,
            generation,
            &RenderCancellation::default(),
        )
        .unwrap();

        let metadata = std::fs::metadata(&output).unwrap();
        assert_eq!(metadata.uid(), original.uid());
        assert_eq!(metadata.gid(), original.gid());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
        assert_eq!(&std::fs::read(&output).unwrap()[0..4], b"RIFF");
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn destination_permissions_survive_overwrite_under_a_different_umask() {
        const OUTPUT: &str = "MIDI_EVIL_PERMISSION_FIXTURE_OUTPUT";
        use std::os::unix::fs::PermissionsExt;

        if let Some(output) = std::env::var_os(OUTPUT) {
            let owner = RenderOwner::default();
            let generation = owner.begin();
            let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
            publish_wav(
                &rendered,
                Path::new(&output),
                &owner,
                generation,
                &RenderCancellation::default(),
            )
            .unwrap();
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("private.wav");
        std::fs::write(&output, b"original").unwrap();
        std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o600)).unwrap();
        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("umask 000; exec \"$1\" --exact render_job::tests::destination_permissions_survive_overwrite_under_a_different_umask --nocapture")
            .arg("sh")
            .arg(std::env::current_exe().unwrap())
            .env(OUTPUT, &output)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_no_job_temporaries(directory.path());
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_destination_creation_conflicts_without_overwrite() {
        for create_symlink in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("shared.wav");
            let owner = RenderOwner::default();
            let generation = owner.begin();
            let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
            let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
            let gate_for_hook = gate.clone();
            let (entered_sender, entered_receiver) = mpsc::channel();
            owner.set_publication_test_hook(&output, move |_, point| {
                if point == PublicationTestPoint::BeforeRename {
                    entered_sender.send(()).unwrap();
                    let (lock, condition) = &*gate_for_hook;
                    let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    while !*allowed {
                        allowed = condition
                            .wait(allowed)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                }
            });
            let worker_owner = owner.clone();
            let output_for_worker = output.clone();
            let publish = std::thread::spawn(move || {
                publish_wav(
                    &rendered,
                    &output_for_worker,
                    &worker_owner,
                    generation,
                    &RenderCancellation::default(),
                )
            });
            entered_receiver.recv().unwrap();
            if create_symlink {
                let target = directory.path().join("competitor.wav");
                std::fs::write(&target, b"competitor").unwrap();
                std::os::unix::fs::symlink(&target, &output).unwrap();
            } else {
                std::fs::write(&output, b"competitor").unwrap();
            }
            let (lock, condition) = &*gate;
            *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
            condition.notify_all();
            let error = publish.join().unwrap().unwrap_err();
            assert!(error.message.contains("without replacing"), "{error}");
            if create_symlink {
                assert!(
                    std::fs::symlink_metadata(&output)
                        .unwrap()
                        .file_type()
                        .is_symlink()
                );
                assert_eq!(
                    std::fs::read(directory.path().join("competitor.wav")).unwrap(),
                    b"competitor"
                );
            } else {
                assert_eq!(std::fs::read(&output).unwrap(), b"competitor");
            }
            assert_no_job_temporaries(directory.path());
        }
    }

    #[test]
    fn publication_errors_and_app_owned_drop_clean_job_temporaries() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("directory-target");
        std::fs::create_dir(&output).unwrap();
        let owner = RenderOwner::default();
        let generation = owner.begin();
        let rendered = evil_render::render_project(&project(), Path::new(".")).unwrap();
        assert!(
            publish_wav(
                &rendered,
                &output,
                &owner,
                generation,
                &RenderCancellation::default(),
            )
            .is_err()
        );
        assert_no_job_temporaries(directory.path());

        let cancelled_output = directory.path().join("shutdown.wav");
        let job = RenderJob::start_wav(
            Arc::new(RenderSnapshot::new(long_project(), PathBuf::from("."))),
            cancelled_output.clone(),
            &owner,
        );
        drop(job);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while owner.worker_count() != 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(owner.worker_count(), 0);
        assert!(!cancelled_output.exists());
        assert_no_job_temporaries(directory.path());
    }
}
