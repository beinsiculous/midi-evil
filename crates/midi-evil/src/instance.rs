use std::fs;
use std::io::{Read, Result as IoResult};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::launch::LaunchOptions;

pub enum InstanceStart {
    Primary(SingleInstance),
    Secondary,
}

pub struct SingleInstance {
    listener: UnixListener,
    path: PathBuf,
}

impl SingleInstance {
    pub fn start(options: &LaunchOptions) -> IoResult<InstanceStart> {
        Self::start_at(options, socket_path())
    }

    fn start_at(options: &LaunchOptions, path: PathBuf) -> IoResult<InstanceStart> {
        if forward(&path, options).is_ok() {
            return Ok(InstanceStart::Secondary);
        }
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        Ok(InstanceStart::Primary(Self { listener, path }))
    }

    pub fn try_recv(&self) -> IoResult<Option<LaunchOptions>> {
        match self.listener.accept() {
            Ok((mut stream, _)) => {
                let mut json = String::new();
                stream.read_to_string(&mut json)?;
                let options = serde_json::from_str(&json)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
                Ok(Some(options))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn forward(path: &Path, options: &LaunchOptions) -> IoResult<()> {
    let mut stream = UnixStream::connect(path)?;
    serde_json::to_writer(&mut stream, options)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    stream.shutdown(std::net::Shutdown::Write)
}

fn socket_path() -> PathBuf {
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("TMPDIR"))
        .unwrap_or_else(|| "/tmp".into());
    let user = std::env::var("USER").unwrap_or_else(|_| "default".to_owned());
    let user: String = user
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    PathBuf::from(directory).join(format!("midi-evil-{user}.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_launch_forwards_options_to_primary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("instance.sock");
        let options = LaunchOptions {
            file: Some(PathBuf::from("song.mid")),
            track: Some("Lead & co".to_owned()),
            project: Some(PathBuf::from("/assets/game")),
            help: false,
        };
        let InstanceStart::Primary(primary) =
            SingleInstance::start_at(&options, path.clone()).unwrap()
        else {
            panic!("first launch should be primary");
        };
        assert!(matches!(
            SingleInstance::start_at(&options, path).unwrap(),
            InstanceStart::Secondary
        ));
        assert_eq!(primary.try_recv().unwrap(), Some(options));
        assert!(primary.try_recv().unwrap().is_none());
    }
}
