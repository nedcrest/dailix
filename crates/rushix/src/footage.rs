//! A project folder's footage as the contact sheet sees it: the clips the
//! analysis found, day by day, each with its length and the sheet's
//! thumbnails. The folder is footage-agent's layout:
//! `analysis/<day>/<clip>.footage.json` (a clip's sidecar: its source file
//! and length) and `report/thumbs/<day>/<clip>/0001.jpg…` (a frame every
//! few seconds); the marking is `report/notes.json`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One clip: its sheet key, its file's name, its length, its thumbnails.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// `<day>/<file>`, as the sheet keys it.
    pub key: String,
    pub file: String,
    pub duration: f64,
    pub thumbs: Vec<PathBuf>,
}

/// A day's clips, in the order they were filmed (by name: cameras number
/// and time-stamp their files).
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub day: String,
    pub clips: Vec<Clip>,
}

/// A project folder's footage.
#[derive(Debug, Clone, PartialEq)]
pub struct Footage {
    pub root: PathBuf,
    pub days: Vec<Day>,
}

impl Footage {
    /// The marking file.
    pub fn notes(&self) -> PathBuf {
        self.root.join("report").join("notes.json")
    }
}

/// The parts of a sidecar this reads.
#[derive(Deserialize)]
struct Sidecar {
    source: String,
    day: String,
    technical: Technical,
}

#[derive(Deserialize)]
struct Technical {
    duration_s: f64,
}

/// Reads the footage of the project folder `root`. A sidecar that can't
/// be read is skipped (and said, in the second value), not fatal: one bad
/// file shouldn't hide a trip.
pub fn read(root: &Path) -> std::io::Result<(Footage, Vec<String>)> {
    let analysis = root.join("analysis");
    let mut days: Vec<Day> = Vec::new();
    let mut skipped = Vec::new();
    let mut day_dirs: Vec<PathBuf> = std::fs::read_dir(&analysis)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    day_dirs.sort();
    for dir in day_dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".footage.json")).collect();
        files.sort();
        let mut clips = Vec::new();
        for f in files {
            let sidecar: Sidecar = match std::fs::read(&f).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())) {
                Ok(s) => s,
                Err(e) => {
                    skipped.push(format!("{}: {e}", f.display()));
                    continue;
                }
            };
            let stem = Path::new(&sidecar.source).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let thumb_dir = root.join("report").join("thumbs").join(&sidecar.day).join(&stem);
            let mut thumbs: Vec<PathBuf> = std::fs::read_dir(&thumb_dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jpg")).collect()).unwrap_or_default();
            thumbs.sort();
            clips.push(Clip { key: format!("{}/{}", sidecar.day, sidecar.source), file: sidecar.source, duration: sidecar.technical.duration_s, thumbs });
        }
        if let Some(day) = clips.first().map(|c| c.key.split('/').next().unwrap_or_default().to_owned()) {
            days.push(Day { day, clips });
        }
    }
    Ok((Footage { root: root.to_owned(), days }, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_clips_come_from_the_sidecars_with_their_thumbnails() {
        let root = std::env::temp_dir().join(format!("rushix-footage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sidecar = |day: &str, src: &str, d: f64| {
            let dir = root.join("analysis").join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let stem = Path::new(src).file_stem().unwrap().to_string_lossy().into_owned();
            std::fs::write(dir.join(format!("{stem}.footage.json")), format!(r#"{{"source": "{src}", "day": "{day}", "technical": {{"duration_s": {d}}}}}"#)).unwrap();
        };
        sidecar("2030-05-02", "B.MP4", 3.0);
        sidecar("2030-05-01", "A2.MP4", 12.5);
        sidecar("2030-05-01", "A1.MP4", 7.0);
        std::fs::write(root.join("analysis/2030-05-01/broken.footage.json"), "{").unwrap();
        let thumbs = root.join("report/thumbs/2030-05-01/A1");
        std::fs::create_dir_all(&thumbs).unwrap();
        for n in ["0002.jpg", "0001.jpg", "notes.txt"] {
            std::fs::write(thumbs.join(n), b"").unwrap();
        }
        let (f, skipped) = read(&root).unwrap();
        assert_eq!(f.days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(), ["2030-05-01", "2030-05-02"]);
        let first = &f.days[0].clips[0];
        assert_eq!((first.key.as_str(), first.duration), ("2030-05-01/A1.MP4", 7.0));
        assert_eq!(first.thumbs, [thumbs.join("0001.jpg"), thumbs.join("0002.jpg")]);
        assert_eq!(f.days[0].clips[1].file, "A2.MP4");
        assert_eq!(skipped.len(), 1);
        assert_eq!(f.notes(), root.join("report/notes.json"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
