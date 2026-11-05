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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where new material lands first.
    pub tmp: Option<PathBuf>,
    /// The projects.
    pub projects: Option<PathBuf>,
    /// The originals.
    pub sources: Option<Sources>,
    /// The project opened last, opened again at start.
    pub last: Option<PathBuf>,
    /// The interface's size; none: as it comes (1.0).
    pub scale: Option<f32>,
    /// The player pane's width, as dragged.
    pub pane_width: Option<f32>,
}

/// The folder of Dailix's settings.
pub fn config_dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
    };
    Some(base.join("dailix"))
}

impl Settings {
    /// The settings saved, or the defaults (no places set).
    pub fn load() -> Settings {
        config_dir().and_then(|d| std::fs::read(d.join("settings.json")).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = config_dir().ok_or_else(|| std::io::Error::other("no configuration folder (HOME unset)"))?;
        std::fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("settings.json"), text)
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
    fn settings_read_back_and_old_files_still_open() {
        let s = Settings { tmp: Some("/t".into()), projects: Some("/p".into()), sources: Some(Sources::Folder { path: "/s".into() }), last: None, scale: None, pane_width: None };
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains(r#""sources":{"kind":"folder","path":"/s"}"#), "{text}");
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
        // A field unknown or missing: the rest still read.
        assert_eq!(serde_json::from_str::<Settings>(r#"{"tmp": "/t", "later": 1}"#).unwrap().tmp, Some("/t".into()));
        assert_eq!(s.describe(), "tmp       /t\nprojects  /p\nsources   /s (a folder)\n");
    }
}
