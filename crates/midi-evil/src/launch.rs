use std::ffi::OsString;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchOptions {
    pub file: Option<PathBuf>,
    pub track: Option<String>,
    /// The 0-based index of the track in the file's own track order. The engine sends it
    /// beside `--track` so the second of two same-named tracks is named exactly; it wins
    /// over the name when both are given. Defaults so an older primary still reads a
    /// forwarded launch.
    #[serde(default)]
    pub track_index: Option<usize>,
    pub project: Option<PathBuf>,
    #[serde(skip)]
    pub help: bool,
}

impl LaunchOptions {
    pub fn parse() -> Result<Self, String> {
        Self::parse_from(std::env::args_os().skip(1))
    }

    pub fn parse_from(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            if argument == "--help" || argument == "-h" {
                options.help = true;
                continue;
            }
            if argument == "--track" {
                options.track = Some(parse_text(
                    next_value(&mut arguments, "--track")?,
                    "--track",
                )?);
                continue;
            }
            if argument == "--track-index" {
                options.track_index = Some(parse_index(&parse_text(
                    next_value(&mut arguments, "--track-index")?,
                    "--track-index",
                )?)?);
                continue;
            }
            if argument == "--project" {
                options.project = Some(PathBuf::from(next_value(&mut arguments, "--project")?));
                continue;
            }
            let text = argument
                .to_str()
                .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
            if let Some(track) = text.strip_prefix("--track=") {
                options.track = Some(track.to_owned());
                continue;
            }
            if let Some(index) = text.strip_prefix("--track-index=") {
                options.track_index = Some(parse_index(index)?);
                continue;
            }
            if let Some(project) = text.strip_prefix("--project=") {
                options.project = Some(PathBuf::from(project));
                continue;
            }
            if text.starts_with('-') {
                return Err(format!("unknown option: {text}"));
            }
            if options.file.is_some() {
                return Err("only one input file may be provided".to_owned());
            }
            options.file = Some(PathBuf::from(argument));
        }
        Ok(options)
    }
}

fn next_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_index(value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("--track-index must be a whole number, not '{value}'"))
}

fn parse_text(value: OsString, option: &str) -> Result<String, String> {
    value
        .into_string()
        .map_err(|_| format!("{option} value must be valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_launch_arguments() {
        let options = LaunchOptions::parse_from([
            OsString::from("--track"),
            OsString::from("Lead & co"),
            OsString::from("--project=/assets/game"),
            OsString::from("song.mid"),
        ])
        .unwrap();
        assert_eq!(options.file, Some(PathBuf::from("song.mid")));
        assert_eq!(options.track.as_deref(), Some("Lead & co"));
        assert_eq!(options.track_index, None);
        assert_eq!(options.project, Some(PathBuf::from("/assets/game")));
    }

    #[test]
    fn parses_the_engines_track_index_in_both_spellings() {
        let spaced = LaunchOptions::parse_from([
            OsString::from("--track"),
            OsString::from("drums"),
            OsString::from("--track-index"),
            OsString::from("2"),
            OsString::from("/game/assets/music/theme.mid"),
        ])
        .unwrap();
        assert_eq!(spaced.track.as_deref(), Some("drums"));
        assert_eq!(spaced.track_index, Some(2));

        let joined = LaunchOptions::parse_from([
            OsString::from("--track-index=0"),
            OsString::from("theme.mid"),
        ])
        .unwrap();
        assert_eq!(joined.track, None);
        assert_eq!(joined.track_index, Some(0));
    }

    #[test]
    fn refuses_a_track_index_that_is_not_a_whole_number() {
        let error =
            LaunchOptions::parse_from([OsString::from("--track-index"), OsString::from("drums")])
                .unwrap_err();
        assert_eq!(error, "--track-index must be a whole number, not 'drums'");
        assert!(LaunchOptions::parse_from([OsString::from("--track-index=-1")]).is_err());
        assert!(LaunchOptions::parse_from([OsString::from("--track-index")]).is_err());
    }

    #[test]
    fn a_forwarded_launch_from_an_older_build_reads_without_a_track_index() {
        let options: LaunchOptions =
            serde_json::from_str(r#"{"file":"song.mid","track":"Lead","project":null}"#).unwrap();
        assert_eq!(options.track.as_deref(), Some("Lead"));
        assert_eq!(options.track_index, None);
    }

    #[test]
    fn rejects_unknown_options_and_duplicate_files() {
        assert!(LaunchOptions::parse_from([OsString::from("--unknown")]).is_err());
        assert!(
            LaunchOptions::parse_from([OsString::from("one.mid"), OsString::from("two.mid")])
                .is_err()
        );
        assert!(LaunchOptions::parse_from([OsString::from("--track")]).is_err());
    }
}
