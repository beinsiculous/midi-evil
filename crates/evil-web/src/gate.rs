//! The gate's bundle: the workstation started from one of the gate's tracked projects, named by
//! the page's `?fixture=`, and shown as the desktop is by `?shows=desktop`. A visitor's bundle is
//! built without this module, so it has no fixtures and reads neither.

use wasm_bindgen::prelude::*;

/// Compiled in by this feature alone, so a check of the bundle can tell the gate's from a
/// visitor's: the fixtures' own names are ordinary words that prove nothing.
const MARKER: &str = "midi-evil-gate-bundle-with-fixtures";

/// The fixture set this bundle was compiled with, as sixteen hexadecimal digits of a 64-bit
/// FNV-1a over each fixture's name and text in turn, each followed by a zero byte. The gate
/// computes the same over the files and refuses a bundle built from any other set, so both gates
/// always open the same projects. Kept in a static so the digits stay in the wasm as written.
static FIXTURES_DIGEST: [u8; 16] = hexadecimal(digest(&FIXTURES));

const fn digest(fixtures: &[(&str, &str)]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fixture = 0;
    while fixture < fixtures.len() {
        let (name, text) = fixtures[fixture];
        hash = fold(fold(hash, name.as_bytes()), text.as_bytes());
        fixture += 1;
    }
    hash
}

/// The bytes, then a zero byte, folded into the hash.
const fn fold(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut position = 0;
    while position <= bytes.len() {
        let byte = if position < bytes.len() {
            bytes[position]
        } else {
            0
        };
        hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        position += 1;
    }
    hash
}

const fn hexadecimal(value: u64) -> [u8; 16] {
    let digits = b"0123456789abcdef";
    let mut text = [0; 16];
    let mut place = 0;
    while place < 16 {
        text[place] = digits[((value >> (60 - 4 * place)) & 0xf) as usize];
        place += 1;
    }
    text
}

/// The gate's tracked projects, as the native gate copies them, by name in order.
const FIXTURES: [(&str, &str); 12] = [
    (
        "arrangement",
        include_str!("../../../coordination/web/probes/fixtures/arrangement.evil"),
    ),
    (
        "bach",
        include_str!("../../../coordination/web/probes/fixtures/bach.evil"),
    ),
    (
        "bass",
        include_str!("../../../coordination/web/probes/fixtures/bass.evil"),
    ),
    (
        "clean",
        include_str!("../../../coordination/web/probes/fixtures/clean.evil"),
    ),
    (
        "lane-point",
        include_str!("../../../coordination/web/probes/fixtures/lane-point.evil"),
    ),
    (
        "long",
        include_str!("../../../coordination/web/probes/fixtures/long.evil"),
    ),
    (
        "same-pitch-overlap",
        include_str!("../../../coordination/web/probes/fixtures/same-pitch-overlap.evil"),
    ),
    (
        "saved",
        include_str!("../../../coordination/web/probes/fixtures/saved.evil"),
    ),
    (
        "tempo",
        include_str!("../../../coordination/web/probes/fixtures/tempo.evil"),
    ),
    (
        "three-notes",
        include_str!("../../../coordination/web/probes/fixtures/three-notes.evil"),
    ),
    (
        "twins",
        include_str!("../../../coordination/web/probes/fixtures/twins.evil"),
    ),
    (
        "two-tracks",
        include_str!("../../../coordination/web/probes/fixtures/two-tracks.evil"),
    ),
];

/// The value of `name` in the page's query string, as given.
fn query(search: &str, name: &str) -> Option<String> {
    search.trim_start_matches('?').split('&').find_map(|pair| {
        pair.strip_prefix(name)?
            .strip_prefix('=')
            .map(str::to_owned)
    })
}

/// The workstation the page's query asks for: its default project, or the named fixture.
pub fn application(
    creation: &eframe::CreationContext<'_>,
    canvas: &web_sys::HtmlCanvasElement,
) -> Result<midi_evil::StudioApp, JsValue> {
    canvas.set_attribute("data-gate-bundle", MARKER)?;
    let digest = std::str::from_utf8(&FIXTURES_DIGEST).map_err(|error| error.to_string())?;
    canvas.set_attribute("data-gate-fixtures", digest)?;
    let search = web_sys::window()
        .ok_or("the page has no window")?
        .location()
        .search()?;
    if query(&search, "shows").as_deref() == Some("desktop") {
        midi_evil::StudioApp::show_what_the_desktop_shows(&creation.egui_ctx);
    }
    let Some(name) = query(&search, "fixture") else {
        return Ok(midi_evil::StudioApp::new(
            creation,
            midi_evil::LaunchOptions::default(),
            None,
            Box::new(()),
        ));
    };
    let (_, text) = FIXTURES
        .iter()
        .find(|(fixture, _)| *fixture == name)
        .ok_or_else(|| format!("the gate has no fixture named {name}"))?;
    Ok(midi_evil::StudioApp::with_project_text(
        creation,
        Box::new(()),
        &format!("{name}.evil"),
        text,
    )?)
}
