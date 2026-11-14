//! Dailix's settings, kept between runs: the three places (README) and the
//! project last opened. In the OS's usual place for configuration
//! (`~/.config/dailix/settings.json` on Linux); nothing about the user's
//! disks is in the code.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where the originals live. A folder covers a local disk, a removable
/// one and a mounted network share; a bucket will be another kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sources {
    Folder { path: PathBuf },
}

/// One of the three places (README).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceKind {
    Tmp,
    Projects,
    Sources,
}

impl PlaceKind {
    /// In the order they're shown everywhere: the trips first.
    pub const ALL: [PlaceKind; 3] = [PlaceKind::Projects, PlaceKind::Tmp, PlaceKind::Sources];

    /// What the window calls it (the UI review, 2026-11-14: one name and
    /// one order on every screen).
    pub fn label(self) -> &'static str {
        match self {
            PlaceKind::Projects => "Trips",
            PlaceKind::Tmp => "TMP",
            PlaceKind::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Scratch space: where a voice is made before it's moved into the
    /// project (`voice::Scratch`); the system's temp folder without it.
    pub tmp: Option<PathBuf>,
    /// The projects.
    pub projects: Option<PathBuf>,
    /// The originals.
    pub sources: Option<Sources>,
    /// Where the window was left: a project (none: the home view) and the
    /// clip picked in it; the next start opens there (user, 2026-11-07:
    /// the first run at home, every run after where it was closed).
    pub last: Option<PathBuf>,
    pub last_clip: Option<String>,
    /// The trip whose page was shown on the home view, if one was.
    pub last_trip: Option<PathBuf>,
    /// The interface's size; none: as it comes (1.0).
    pub scale: Option<f32>,
    /// The player pane's share of the window's width, as dragged.
    pub pane_share: Option<f32>,
    /// Whether a hovered card's tiles change by themselves (Omnix's project
    /// cards, gap 110); none: they do.
    pub card_motion: Option<bool>,
}


impl Settings {
    /// Whether a hovered card's tiles change by themselves.
    pub fn card_motion(&self) -> bool {
        self.card_motion.unwrap_or(true)
    }

    /// The settings saved, or the defaults (no places set).
    pub fn load() -> Settings {
        // Where Omnix looks for it too (its projects place at first).
        omnix::dailix_settings().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let file = omnix::dailix_settings().ok_or_else(|| std::io::Error::other("no configuration folder (HOME unset)"))?;
        std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(file, text)
    }

    /// Place `kind`'s folder, if set.
    pub fn place(&self, kind: PlaceKind) -> Option<PathBuf> {
        match kind {
            PlaceKind::Tmp => self.tmp.clone(),
            PlaceKind::Projects => self.projects.clone(),
            PlaceKind::Sources => self.sources.as_ref().map(|Sources::Folder { path }| path.clone()),
        }
    }

    /// Place `kind` set to `folder` (the sources as a folder).
    pub fn set_place(&mut self, kind: PlaceKind, folder: PathBuf) {
        match kind {
            PlaceKind::Tmp => self.tmp = Some(folder),
            PlaceKind::Projects => self.projects = Some(folder),
            PlaceKind::Sources => self.sources = Some(Sources::Folder { path: folder }),
        }
    }

    /// The places, a line each, for `dailix places`.
    pub fn describe(&self) -> String {
        let show = |p: &Option<PathBuf>| p.as_ref().map_or_else(|| "(not set)".to_owned(), |p| p.display().to_string());
        let sources = match &self.sources {
            Some(Sources::Folder { path }) => format!("{} (a folder)", path.display()),
            None => "(not set)".to_owned(),
        };
        format!("tmp       {}\nprojects  {}\nsources   {sources}\n", show(&self.tmp), show(&self.projects))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_place_is_set_and_read_by_its_kind() {
        let mut s = Settings::default();
        for (k, p) in PlaceKind::ALL.into_iter().zip(["/p", "/t", "/s"]) {
            s.set_place(k, p.into());
            assert_eq!(s.place(k), Some(p.into()));
        }
        assert_eq!(s.describe(), "tmp       /t\nprojects  /p\nsources   /s (a folder)\n");
    }

    #[test]
    fn omnix_reads_the_projects_place_as_saved() {
        let s = Settings { projects: Some("/p".into()), ..Settings::default() };
        assert_eq!(omnix::projects_place_in(&serde_json::to_vec_pretty(&s).unwrap()), Some("/p".into()));
        assert_eq!(omnix::projects_place_in(&serde_json::to_vec(&Settings::default()).unwrap()), None);
    }

    #[test]
    fn settings_read_back_and_old_files_still_open() {
        let s = Settings { tmp: Some("/t".into()), projects: Some("/p".into()), sources: Some(Sources::Folder { path: "/s".into() }), last: None, last_clip: None, last_trip: None, scale: None, pane_share: None, card_motion: Some(false) };
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains(r#""sources":{"kind":"folder","path":"/s"}"#), "{text}");
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
        // A field unknown or missing: the rest still read.
        assert_eq!(serde_json::from_str::<Settings>(r#"{"tmp": "/t", "later": 1}"#).unwrap().tmp, Some("/t".into()));
        assert_eq!(s.describe(), "tmp       /t\nprojects  /p\nsources   /s (a folder)\n");
    }
}
