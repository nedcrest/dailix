//! F1: the keys and the mouse, a group each, over the window. The window
//! itself carries no help text (Omnix's rule): this list, and tooltips.
//! The same list is the guide's reference page (`dailix keys`, kept in
//! Omnix's `docs/guide/keys-dailix.md`, checked by a test).

/// The way footage goes through Dailix, a step each (the user,
/// 2026-11-17: «what next?» wasn't said anywhere): what it is, where in
/// the window it's done. «How Dailix works» in ☰ shows it.
pub const WAY: [(&str, &str); 7] = [
    ("Into TMP", "Copy what the cameras and phones filmed into the TMP folder (by hand for now; Dailix doesn't read cards yet)."),
    ("Sort into trips", "Home's «TMP» button: pick videos, choose a trip (or make one) and its parts, «File». Copies go into the trip by day; TMP keeps the originals."),
    ("Into the archive", "In the sorter, a day's «Into the archive»: the originals copied into the Sources folder by trip and day (those in no trip under _not-in-a-trip), each checked by its hash."),
    ("Free TMP", "Then «Free TMP»: the originals the archive holds (checked again) go to the trash."),
    ("Choose", "The trip's contact sheet: rate (P ? X), mark segments (I O), write each day's story."),
    ("Parts", "Your own groups across days («Island rides»): from the sorter, the sheet's pane, the trip's page."),
    ("Into Omnix", "«→ Omnix» on a day, the trip or a part: an edit of what you chose, opened in Omnix."),
];

/// The groups: a title and its lines (keys, what they do).
pub const GROUPS: [(&str, &[(&str, &str)]); 6] = [
    (
        "Clips",
        &[
            ("click, ← →", "pick a clip"),
            ("Ctrl+click / Shift+click", "a group of clips (from the one in hand): into a part, a new part, out of the part shown"),
            ("P", "take (✓), then the next"),
            ("?", "think about it (?), then the next"),
            ("X", "junk (✕), then the next"),
            ("U", "unrated"),
            ("Tab", "the keys to the player, and back"),
        ],
    ),
    (
        "Player",
        &[
            ("Space", "play, pause"),
            ("← →", "a frame (the player's keys, after Tab)"),
            ("Shift+← →", "a second"),
            ("drag on the strip", "scrub"),
        ],
    ),
    (
        "Marks",
        &[
            ("I", "an in point"),
            ("O", "a segment from the in point to here"),
            ("I / O in a picked segment", "move its start / end"),
            ("M", "a flag: a note to self, not material"),
            ("N", "a moment: a segment a second either side (on a flag, its role)"),
            ("O on a picked flag", "the flag becomes a segment to here"),
            ("Page Up / Down", "the mark before / after"),
            ("Delete / Backspace", "remove the picked mark"),
            ("drag an edge or a flag", "move it"),
        ],
    ),
    (
        "A new mark",
        &[
            ("↑ ↓, Enter", "its role"),
            ("Enter", "its note saved, done"),
            ("Esc, Tab", "take it away again"),
        ],
    ),
    (
        "Window",
        &[
            ("Ctrl+= / Ctrl+−", "bigger / smaller text"),
            ("Ctrl+0", "text as it was"),
            ("drag the pane's edge", "a wider player (to half the window)"),
            ("Alt+←", "back: the sheet to its trip, a trip to the trips"),
            ("Ctrl+N", "a new trip"),
            ("Ctrl+,", "settings: the places, the look"),
            ("F1", "this list"),
            ("Esc", "close a menu or a window over the page"),
        ],
    ),
    (
        "TMP sorter",
        &[
            ("click", "pick a video, and play it"),
            ("Ctrl+click", "add it to the picked, or take it out"),
            ("Shift+click", "pick the run from the last clicked"),
            ("← →", "the next video (Shift: the run grows)"),
            ("Ctrl+A", "pick every new one"),
            ("Space", "play, pause"),
            ("Enter", "file the picked into the trip"),
            ("Esc", "back to the trips"),
        ],
    ),
];

/// The guide's reference page, as Omnix's own is made.
pub fn page() -> String {
    omnix::keys::markdown("Keys: Dailix", "Made by `dailix keys`", &GROUPS)
}

#[cfg(test)]
mod tests {
    /// The guide (in Omnix's repo, beside this one's, as CI checks it out)
    /// shows this list as it is now: `OMNIX_BLESS=1 cargo test` writes it.
    #[test]
    fn the_guides_keys_page_is_the_programs_list() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../omnix-360-studio/docs/guide/keys-dailix.md");
        let page = super::page();
        if std::env::var_os("OMNIX_BLESS").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), page, "{} is behind the keys: OMNIX_BLESS=1 cargo test writes it", path.display());
    }
}
