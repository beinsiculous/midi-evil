//! The browser build's page carries the keyboard notes and what is desktop-only as static text,
//! there before the application has loaded. They are the browser's tables and the browser's list
//! of what it leaves out, written by the function that writes the Keyboard notes window, so this
//! test writes them as the page's HTML and fails when the page holds anything else.

use super::*;

const FIRST_LINE: &str = "<!-- From here to the line that ends it, the library's test \
     the_page_holds_the_notes_the_browser_build_writes writes this text from the browser's \
     tables: change those, and paste what the test prints. -->";
const LAST_LINE: &str = "<!-- The end of what the library's test writes. -->";

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The notes as the page holds them: the keyboard notes under one heading, a heading for each
/// surface, one item a chord; then what is desktop-only.
fn page_notes() -> String {
    let mut html = vec![FIRST_LINE.to_owned(), "<h2>Keyboard notes</h2>".to_owned()];
    for section in keyboard_notes("Control", Platform::Browser, |_| true) {
        let heading = if section.title == DESKTOP_ONLY_TITLE {
            "h2"
        } else {
            "h3"
        };
        html.push(format!("<{heading}>{}</{heading}>", escaped(section.title)));
        html.push("<ul>".to_owned());
        html.extend(
            section
                .lines
                .iter()
                .map(|line| format!("<li>{}</li>", escaped(line))),
        );
        html.push("</ul>".to_owned());
    }
    html.push(LAST_LINE.to_owned());
    html.join("\n")
}

#[test]
fn the_page_holds_the_notes_the_browser_build_writes() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../evil-web/index.html");
    let page = std::fs::read_to_string(path).unwrap();
    let wanted = page_notes();
    let held = page.find(FIRST_LINE).and_then(|start| {
        let end = page[start..].find(LAST_LINE)? + start + LAST_LINE.len();
        Some(&page[start..end])
    });
    assert!(
        held == Some(wanted.as_str()),
        "crates/evil-web/index.html must hold exactly this:\n{wanted}"
    );
}

#[test]
fn the_notes_say_why_a_refused_chord_does_nothing_and_name_no_chord_a_browser_keeps() {
    let notes = page_notes();
    assert!(notes.contains("Space: plays or stops the song, where the focused control does not take Space — not in the browser build yet"));
    assert!(
        notes.contains("B: starts playback at the part's first bar — not in the browser build yet")
    );
    assert!(notes.contains("Alt+Page Down: "));
    for kept in ["F6", "Alt+Left", "Alt+Right", "Control+N", "Control+O"] {
        assert!(!notes.contains(kept), "{kept}");
    }
    for &thing in Platform::Browser.left_out() {
        assert!(notes.contains(speech::left_out_name(thing)), "{thing:?}");
    }
    let window = help_text_on("Control", Platform::Browser, |_| true);
    for section in keyboard_notes("Control", Platform::Browser, |_| true) {
        for line in section.lines {
            assert!(window.contains(&line), "the window lacks {line:?}");
        }
    }
}
