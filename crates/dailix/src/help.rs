//! F1: the keys and the mouse, a group each, over the window. The window
//! itself carries no help text (Omnix's rule): this list, and tooltips.
//! The same list is the guide's reference page (`dailix keys`, kept in
//! Omnix's `docs/guide/keys-dailix.md`, checked by a test).

/// The groups: a title and its lines (keys, what they do).
pub const GROUPS: [(&str, &[(&str, &str)]); 5] = [
    (
        "Clips",
        &[
            ("click, ← →", "pick a clip"),
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
