//! Which build is running: the desktop's or the browser's. It is decided once, where the
//! application is made, and kept in egui's memory beside the widgets, so every module reads the
//! same value from the context it already has and a test can run the browser's behaviour
//! natively by installing it. What a platform shows is decided here too, in one list: each
//! control asks it whether to draw itself, and the one place a view is selected asks it whether
//! that view may be shown. So is how much a platform renders before it plays.

use eframe::egui;
use evil_project::{Part, Project, Track};

use crate::speech::Unavailable;
use crate::workspace::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    #[default]
    Desktop,
    Browser,
}

/// Something the desktop shows that a platform may leave out. A thing left out is not drawn, so
/// it is neither a Tab stop nor in the frame's tree, and nothing else leads to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// A view: its tab, and every button that opens it.
    View(View),
    /// The Notes view's Automation panel, its toggle, and the side panel's Automation button.
    Automation,
    /// The MIDI input port and its refresh button.
    MidiInput,
    /// Import MIDI, in the Project menu and in the About window.
    ImportMidi,
    RecoverRhythmFromWav,
    RelinkMidi,
    /// Export MIDI, in the Project menu and in the export row.
    ExportMidi,
    /// The WAV renders, the track stems and the arrangement parts, wherever they are offered.
    WavRenders,
    /// The toolbar's Export toggle and the row it shows.
    ExportRow,
    /// Load files…, in the sound bar and in the sound library.
    LoadSoundFiles,
    /// Moving a window by its title bar, which is then a stop of its own.
    MovingWindows,
    /// Dragging the side panel's edge to resize it, which is then a stop of its own.
    ResizingTheSidePanel,
}

/// What the browser build leaves out, each until the batch that gives it its own gate removes
/// its line: the views that have no keyboard model yet, the controls that need a file, a device
/// or a renderer a page does not give this build, and the stops only a pointer can drag.
const LEFT_OUT_OF_THE_BROWSER: [Shown; 15] = [
    Shown::View(View::Mixer),
    Shown::View(View::Synth),
    Shown::View(View::Keyboard),
    Shown::View(View::Pads),
    Shown::Automation,
    Shown::MidiInput,
    Shown::ImportMidi,
    Shown::RecoverRhythmFromWav,
    Shown::RelinkMidi,
    Shown::ExportMidi,
    Shown::WavRenders,
    Shown::ExportRow,
    Shown::LoadSoundFiles,
    Shown::MovingWindows,
    Shown::ResizingTheSidePanel,
];

impl Platform {
    /// The build this binary is. The only place the library asks the compiler which it is.
    pub const fn of_this_build() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Browser
        } else {
            Self::Desktop
        }
    }

    /// What this platform leaves out of what the desktop shows.
    pub fn left_out(self) -> &'static [Shown] {
        match self {
            Self::Desktop => &[],
            Self::Browser => &LEFT_OUT_OF_THE_BROWSER,
        }
    }

    /// Why a take cannot be recorded: the browser build has no recorder yet.
    pub fn recording_refused(self) -> Option<Unavailable> {
        match self {
            Self::Desktop => None,
            Self::Browser => Some(Unavailable::NoRecordingInBrowser),
        }
    }

    /// Whether a preview of `project` that ends at `end_tick` — at the song's end, when none —
    /// is more than this platform renders. The browser renders on the page's own thread, which
    /// answers nothing until the render is done, so it renders only what it can in a few
    /// seconds; the desktop renders on a thread of its own and renders anything.
    pub fn renders_too_much(self, project: &Project, end_tick: Option<u64>) -> bool {
        match self {
            Self::Desktop => false,
            Self::Browser => track_minutes(project, end_tick) > BROWSER_TRACK_MINUTES,
        }
    }

    /// Whether an unavailable control keeps its place in the Tab order. A desktop screen reader
    /// finds a disabled control by reviewing the window; a canvas in a page offers no review, so
    /// in the browser an unavailable control can still be reached, and says why.
    pub fn keeps_unavailable_controls_reachable(self) -> bool {
        self == Self::Browser
    }

    /// Whether an outcome waits for a second pass, so a front end that writes one string a frame
    /// says it with the controls as its action left them. A desktop screen reader is told the
    /// outcome and then each change as it comes, and nothing there is run twice.
    pub fn pairs_an_outcome_with_the_controls_it_changed(self) -> bool {
        self == Self::Browser
    }

    /// Whether a text field that an Enter or an Escape ended keeps its place. On the desktop
    /// egui leaves nothing focused, as it always has; in the browser that would send the next
    /// Tab back to the page's first control, so the field stays what is current and Tab moves on
    /// from it.
    pub fn keeps_the_place_of_an_ended_edit(self) -> bool {
        self == Self::Browser
    }
}

/// The most the browser renders in place, in track-minutes at the rate below: five stopped
/// Firefox, which renders five to six times slower than Chromium, for no more than five seconds.
/// Notes sounding together cost more than notes in a line, and nothing here counts them, so this
/// is a bound on a render that blocks the page, not a promise about one.
const BROWSER_TRACK_MINUTES: f64 = 5.0;

/// The sample rate the bound was measured at; a project at a higher rate makes more sound per
/// minute, in proportion.
const MEASURED_SAMPLE_RATE: f64 = 48_000.0;

/// How much sound a render up to `end_tick` makes: for each track that sounds before it, the
/// minutes from the song's start to where that track's sound ends — the renderer always works
/// from the start, whatever range it hands back, and a note that starts before the end is made
/// whole, however far past it runs — scaled by the sample rate. Read from the project's timing,
/// without expanding the song into a performance, which is most of what a render costs. A
/// project whose timing is invalid counts nothing: the render refuses it and says why.
fn track_minutes(project: &Project, end_tick: Option<u64>) -> f64 {
    let end_tick = end_tick.unwrap_or_else(|| project.end_tick());
    let Ok(timing) = project.timing() else {
        return 0.0;
    };
    let sounding_seconds: f64 = project
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .filter_map(|track| last_sounding_tick(project, track, end_tick))
        .map(|tick| timing.seconds_at(tick))
        .sum();
    sounding_seconds / 60.0 * f64::from(project.sample_rate) / MEASURED_SAMPLE_RATE
}

/// Where the sound of a track played up to `end_tick` ends, or `None` when nothing of it starts
/// before then: `end_tick`, or later when a note that starts before it runs past it.
fn last_sounding_tick(project: &Project, track: &Track, end_tick: u64) -> Option<u64> {
    let own_notes = track
        .notes
        .iter()
        .filter(|note| note.start_tick < end_tick)
        .map(|note| note.start_tick.saturating_add(note.duration_ticks));
    let part_notes = track
        .parts
        .iter()
        .filter(|part| part.start_tick < end_tick)
        .filter_map(|part| last_tick_of_a_part(project, part, end_tick));
    own_notes
        .chain(part_notes)
        .max()
        .map(|last_tick| last_tick.max(end_tick))
}

/// Where the last of a part's notes ends, of those that start before `end_tick` and before the
/// part's own end. The part's repeats are not walked: each note of the pattern is placed in the
/// last repeat in which it still starts in time, which is where it ends latest.
fn last_tick_of_a_part(project: &Project, part: &Part, end_tick: u64) -> Option<u64> {
    let pattern = project.patterns.get(part.pattern)?;
    let last_start = part
        .start_tick
        .saturating_add(part.length_ticks)
        .min(end_tick)
        .checked_sub(1)?;
    pattern
        .notes
        .iter()
        .filter_map(|note| {
            let first_start = part.start_tick.checked_add(note.start_tick)?;
            let repeats_before = last_start.checked_sub(first_start)? / pattern.length_ticks.max(1);
            Some(
                first_start
                    .saturating_add(repeats_before.saturating_mul(pattern.length_ticks))
                    .saturating_add(note.duration_ticks),
            )
        })
        .max()
}

fn memory_id() -> egui::Id {
    egui::Id::new("platform")
}

fn shown_memory_id() -> egui::Id {
    egui::Id::new("platform-shown")
}

/// Records which build this is, for every later frame of this context.
pub fn install(context: &egui::Context, platform: Platform) {
    context.data_mut(|data| data.insert_temp(memory_id(), platform));
}

/// The build recorded for this context; the desktop's when none was.
pub fn of(context: &egui::Context) -> Platform {
    context.data(|data| data.get_temp(memory_id()).unwrap_or_default())
}

/// Has this context show what `platform` shows while it goes on behaving as the build it is: how
/// a gate shows that its walk would find a control the browser leaves out, were it drawn.
#[cfg(feature = "gate")]
pub fn install_shown(context: &egui::Context, platform: Platform) {
    context.data_mut(|data| data.insert_temp(shown_memory_id(), platform));
}

/// Whether this context draws `thing`.
pub fn shows(context: &egui::Context, thing: Shown) -> bool {
    let shown_as = context.data(|data| data.get_temp(shown_memory_id()));
    !shown_as
        .unwrap_or_else(|| of(context))
        .left_out()
        .contains(&thing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::{Instrument, Note, Pattern};

    fn repository() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fixture(name: &str) -> Project {
        Project::load(repository().join(format!("coordination/web/probes/fixtures/{name}.evil")))
            .unwrap()
    }

    /// Two tracks of one-beat notes at 120 beats a minute, so each beat is half a second.
    fn two_tracks_of(beats: u64) -> Project {
        let mut project = Project::new("Lines");
        project.sample_rate = 48_000;
        for name in ["One", "Two"] {
            let instrument = project
                .add_instrument(Instrument::synth(name, "lead"))
                .unwrap();
            project
                .add_track(Track {
                    name: name.to_owned(),
                    instrument,
                    notes: (0..beats)
                        .map(|beat| Note {
                            pitch: 60,
                            velocity: 100,
                            start_tick: beat * 480,
                            duration_ticks: 480,
                        })
                        .collect(),
                    ..Track::default()
                })
                .unwrap();
        }
        project
    }

    #[test]
    fn the_bach_invention_is_three_and_three_eighths_track_minutes() {
        let minutes = track_minutes(&fixture("bach"), None);
        assert!((minutes - 3.375).abs() < 1e-9, "{minutes}");
        assert!(!Platform::Browser.renders_too_much(&fixture("bach"), None));
    }

    #[test]
    fn the_fixtures_at_the_limit_play_and_the_one_a_bar_longer_does_not() {
        for name in ["bach-at-the-limit", "chords-at-the-limit"] {
            let project = fixture(name);
            let minutes = track_minutes(&project, None);
            assert!(
                (minutes - BROWSER_TRACK_MINUTES).abs() < 1e-9,
                "{name}: {minutes}"
            );
            assert!(
                !Platform::Browser.renders_too_much(&project, None),
                "{name}"
            );
        }
        let longer = fixture("chords-over-the-limit");
        assert!(Platform::Browser.renders_too_much(&longer, None));
        let bar = longer.bar_ticks();
        assert!(!Platform::Browser.renders_too_much(&longer, Some(2 * bar)));
        assert!(!Platform::Desktop.renders_too_much(&longer, None));
    }

    #[test]
    fn a_range_counts_from_the_song_start_to_its_end() {
        let project = two_tracks_of(120);
        assert!((track_minutes(&project, None) - 2.0).abs() < 1e-9);
        assert!((track_minutes(&project, Some(30 * 480)) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_muted_track_and_one_with_nothing_before_the_end_make_no_sound() {
        let mut project = two_tracks_of(120);
        project.tracks[0].muted = true;
        assert!((track_minutes(&project, None) - 1.0).abs() < 1e-9);
        project.tracks[0].muted = false;
        for note in &mut project.tracks[1].notes {
            note.start_tick += 60 * 480;
        }
        assert!((track_minutes(&project, Some(30 * 480)) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn a_note_that_outlasts_the_range_counts_to_its_own_end() {
        let mut project = two_tracks_of(120);
        project.tracks[0].notes[0].duration_ticks = 240 * 480;
        let two_bars = Some(8 * 480);
        let expected = (120.0 + 4.0) / 60.0;
        assert!((track_minutes(&project, two_bars) - expected).abs() < 1e-9);
    }

    #[test]
    fn a_pattern_note_that_outlasts_the_range_counts_to_its_own_end() {
        let mut project = two_tracks_of(0);
        let pattern = project
            .add_pattern(Pattern {
                name: "Drone".to_owned(),
                length_ticks: 1_920,
                notes: vec![Note {
                    pitch: 60,
                    velocity: 100,
                    start_tick: 480,
                    duration_ticks: 120 * 480,
                }],
                ..Pattern::default()
            })
            .unwrap();
        let part = Part {
            pattern,
            start_tick: 0,
            length_ticks: 16 * 1_920,
        };
        project.add_part(0, part).unwrap();
        // Played to the end of bar two, the drone last starts at beat 6 and ends 120 beats on.
        let expected = (5.0 + 120.0) * 0.5 / 60.0;
        assert!((track_minutes(&project, Some(2 * 1_920)) - expected).abs() < 1e-9);
    }

    #[test]
    fn a_part_counts_its_track_and_a_higher_rate_counts_in_proportion() {
        let mut project = two_tracks_of(0);
        let pattern = project
            .add_pattern(Pattern {
                name: "Riff".to_owned(),
                length_ticks: 1_920,
                notes: vec![Note::default()],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 120 * 480,
                },
            )
            .unwrap();
        assert!((track_minutes(&project, None) - 1.0).abs() < 1e-9);
        project.sample_rate = 96_000;
        assert!((track_minutes(&project, None) - 2.0).abs() < 1e-9);
    }
}
