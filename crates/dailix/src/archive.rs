//! After filing (Omnix gap 119; INGEST.md steps 9 and 11): TMP's videos
//! copied into the archive (the sources place), each checked there by its
//! BLAKE3 hash, then TMP freed of them, to the trash, only once their
//! archive copy still reads the same. A video goes the way
//! new → filed (into a trip) → archived → freed (gone from TMP).
//!
//! The archive keeps the originals as they came, by trip and day, as
//! Kos's does: `<sources>/<trip>/<day>/<the original's name>`. The ledger
//! in TMP says what's archived and its hash.

use crate::tmp::{Filed, Ledger};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A TMP video's archive copy: where, and its content's hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Archived {
    pub path: PathBuf,
    pub hash: String,
}

/// Where TMP's videos in no trip go in the archive (the user,
/// 2026-11-17: a day's unsorted videos had no way into it), by day.
pub const NOT_IN_A_TRIP: &str = "_not-in-a-trip";

/// Where a TMP video goes in the archive: the trip and day it was filed
/// under, its own name; one in no trip under `_not-in-a-trip/<its day>`
/// (`no-date` without one).
pub fn target(sources: &Path, filed: Option<&Filed>, day: Option<&str>, original: &Path) -> PathBuf {
    let name = original.file_name().unwrap_or_default();
    match filed {
        Some(f) => sources.join(&f.trip).join(&f.day).join(name),
        None => sources.join(NOT_IN_A_TRIP).join(day.unwrap_or("no-date")).join(name),
    }
}

/// How many of a group can go into the archive (not there yet), and how
/// many TMP can let go (there, checked): a day offers both.
pub fn counts(stages: &[Stage]) -> (usize, usize) {
    let archived = stages.iter().filter(|s| **s == Stage::Archived).count();
    (stages.len() - archived, archived)
}

/// The whole file's BLAKE3, as hex.
pub fn hash(path: &Path) -> std::io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(path)?)?;
    Ok(h.finalize().to_hex().to_string())
}

/// Where a TMP video is on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    New,
    Filed,
    Archived,
}

/// Video `rel`'s stage, by the ledger.
pub fn stage(ledger: &Ledger, rel: &str) -> Stage {
    if ledger.archived.contains_key(rel) {
        Stage::Archived
    } else if ledger.filed(rel).is_empty() {
        Stage::New
    } else {
        Stage::Filed
    }
}

/// The next step for a group of TMP's videos (a day, or the picked), and
/// how many it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Some aren't in a trip yet: file them.
    File(usize),
    /// Filed, not in the archive yet.
    Archive(usize),
    /// In the archive, checked: TMP can let them go.
    Free(usize),
    /// Nothing left.
    Done,
}

/// The next step for videos at `stages`: the earliest one any of them
/// waits for (TMP is freed only of what's archived, and archived only
/// what's filed; files first, so a day isn't half in the archive).
pub fn next(stages: &[Stage]) -> Next {
    let n = |s: Stage| stages.iter().filter(|x| **x == s).count();
    match (n(Stage::New), n(Stage::Filed), n(Stage::Archived)) {
        (0, 0, 0) => Next::Done,
        (0, 0, a) => Next::Free(a),
        (0, f, _) => Next::Archive(f),
        (new, _, _) => Next::File(new),
    }
}

/// TMP's video `original` (at `rel`) copied into the archive at `sources`
/// under where it was filed, and checked there: its hash read back from
/// the copy is the original's. A copy already there with the same hash
/// is taken as it is; another file in its place is never written over.
pub fn archive_one(original: &Path, filed: Option<&Filed>, day: Option<&str>, sources: &Path) -> Result<Archived, String> {
    let to = target(sources, filed, day, original);
    let want = hash(original).map_err(|e| format!("{}: {e}", original.display()))?;
    if to.exists() {
        let there = hash(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        return if there == want { Ok(Archived { path: to, hash: want }) } else { Err(format!("{} is in the archive already and isn't this file", to.display())) };
    }
    let size = std::fs::metadata(original).map_err(|e| e.to_string())?.len();
    crate::import::copy(&crate::import::Copy { from: original.to_path_buf(), to: to.clone(), size }).map_err(|e| format!("{}: {e}", to.display()))?;
    let got = hash(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    if got != want {
        let _ = std::fs::remove_file(&to);
        return Err(format!("{}: the copy reads differently from the original; not kept", to.display()));
    }
    Ok(Archived { path: to, hash: want })
}

/// TMP's video `original` to the trash (with its poster), once its archive
/// copy still has the hash it was archived with.
pub fn free_one(original: &Path, poster: &Path, archived: &Archived) -> Result<(), String> {
    // The original itself first: only the very file that was archived
    // goes (the audit, 2026-11-17: another file at its place would have).
    match hash(original) {
        Ok(h) if h == archived.hash => {}
        Ok(_) => return Err(format!("{}: not the file that was archived; kept", original.display())),
        Err(e) => return Err(format!("{}: {e}", original.display())),
    }
    match hash(&archived.path) {
        Ok(h) if h == archived.hash => {}
        Ok(_) => return Err(format!("{}: the archive copy has changed; TMP keeps the original", archived.path.display())),
        Err(e) => return Err(format!("{}: {e}; TMP keeps the original", archived.path.display())),
    }
    let mut files = vec![original];
    if poster.is_file() {
        files.push(poster);
    }
    trash::delete_all(files).map_err(|e| format!("{}: {e}", original.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filed(trip: &str, day: &str) -> Filed {
        Filed { trip: trip.into(), day: day.into(), name: "x.MP4".into() }
    }

    #[test]
    fn the_archive_keeps_originals_by_trip_and_day() {
        assert_eq!(target(Path::new("/a"), Some(&filed("2026-08_kos", "2026-08-22")), Some("2026-08-22"), Path::new("/tmp-place/card/DJI_0001.MP4")), PathBuf::from("/a/2026-08_kos/2026-08-22/DJI_0001.MP4"));
        assert_eq!(target(Path::new("/a"), None, Some("2026-08-22"), Path::new("/t/DJI_0002.MP4")), PathBuf::from("/a/_not-in-a-trip/2026-08-22/DJI_0002.MP4"));
        assert_eq!(target(Path::new("/a"), None, None, Path::new("/t/x.mp4")), PathBuf::from("/a/_not-in-a-trip/no-date/x.mp4"));
        assert_eq!(counts(&[Stage::New, Stage::Filed, Stage::Archived]), (2, 1));
    }

    #[test]
    fn the_next_step_is_the_earliest_any_video_waits_for() {
        use Stage::*;
        assert_eq!(next(&[]), Next::Done);
        assert_eq!(next(&[New, Filed, Archived]), Next::File(1));
        assert_eq!(next(&[Filed, Filed, Archived]), Next::Archive(2));
        assert_eq!(next(&[Archived, Archived]), Next::Free(2));
    }

    #[test]
    fn a_video_is_archived_checked_and_freed_only_while_its_copy_holds() {
        let dir = std::env::temp_dir().join(format!("dailix-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (tmp, sources) = (dir.join("tmp"), dir.join("sources"));
        std::fs::create_dir_all(tmp.join("card")).unwrap();
        let original = tmp.join("card/DJI_0001.MP4");
        std::fs::write(&original, b"the picture").unwrap();
        let f = filed("kos", "2026-08-22");
        let a = archive_one(&original, Some(&f), None, &sources).unwrap();
        assert_eq!((std::fs::read(&a.path).unwrap(), a.hash.clone()), (b"the picture".to_vec(), hash(&original).unwrap()));
        // Again: the copy there is taken; another file in its place isn't overwritten.
        assert_eq!(archive_one(&original, Some(&f), None, &sources).unwrap(), a);
        std::fs::write(&a.path, b"something else").unwrap();
        assert!(archive_one(&original, Some(&f), None, &sources).is_err());
        // A changed copy keeps the original in TMP; so does another original.
        assert!(free_one(&original, &tmp.join("none.jpg"), &a).is_err() && original.is_file());
        std::fs::write(&a.path, b"the picture").unwrap();
        std::fs::write(&original, b"another card's DJI_0001").unwrap();
        assert!(free_one(&original, &tmp.join("none.jpg"), &a).is_err() && original.is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_videos_stage_comes_from_the_ledger() {
        let mut l = Ledger::default();
        assert_eq!(stage(&l, "a"), Stage::New);
        l.record("a", filed("kos", "2026-08-22"));
        assert_eq!(stage(&l, "a"), Stage::Filed);
        l.archived.insert("a".into(), Archived { path: "/s/a".into(), hash: "h".into() });
        assert_eq!(stage(&l, "a"), Stage::Archived);
    }
}
