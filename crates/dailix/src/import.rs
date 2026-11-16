//! `dailix import-legacy`: a footage-agent project made into a Dailix one,
//! beside it. The old project is only read, as are the originals in the
//! archive it names; the new one gets the clips copied in, the marking,
//! the analysis and the cleaned voices in `dailix/`, and the numbered
//! folders' material.
//!
//! What's left behind on purpose (don't inherit, improve: the old project's
//! leftovers would only be carried along): the
//! web page's view state, DJI's LRF proxies (they stay in the archive),
//! the web sheet's thumbnails and browser previews, finished exports, old
//! edits (they name the old paths) and the old trash.
//!
//! A copy goes to a hidden name, is checked by size and synced to the disk,
//! then renamed; a file already there with the same size is skipped, so an
//! import stopped half way just runs again.

use dailix_sheet::{Doc, Layout, Store};
use std::path::{Path, PathBuf};

/// A file to copy, and its size.
#[derive(Debug, Clone, PartialEq)]
pub struct Copy {
    pub from: PathBuf,
    pub to: PathBuf,
    pub size: u64,
}

/// What an import will do.
#[derive(Debug)]
pub struct Plan {
    pub old: PathBuf,
    pub layout: Layout,
    pub archive: Option<PathBuf>,
    pub copies: Vec<Copy>,
}

impl Plan {
    pub fn bytes(&self) -> u64 {
        self.copies.iter().map(|c| c.size).sum()
    }
}

type Error = Box<dyn std::error::Error>;

/// The numbered folders whose material is carried over as it is; the
/// pictures' too, under their name now (`PICTURES`), from the old name or
/// the new (audit A7: the old was carried as it was, the new not at all).
const CARRIED: [&str; 4] = ["02_audio", "06_external", "07_sfx", "09_luts"];
const PICTURES_FROM: [&str; 2] = [dailix_sheet::layout::OLD_PICTURES, dailix_sheet::layout::PICTURES];

/// Every file under `from`, to the same place under `to` (hidden files and
/// those `skip` refuses left out).
fn tree(from: &Path, to: &Path, skip: &dyn Fn(&Path) -> bool, out: &mut Vec<Copy>) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(from) else { return Ok(()) };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') || skip(&path) {
            continue;
        }
        if path.is_dir() {
            tree(&path, &to.join(&name), skip, out)?;
        } else if path.is_file() {
            out.push(Copy { size: e.metadata()?.len(), from: path, to: to.join(&name) });
        }
    }
    Ok(())
}

/// Plans the import of the footage-agent project `old` into `new`.
pub fn plan(old: &Path, new: &Path) -> Result<Plan, Error> {
    if !old.join("report").join("notes.json").is_file() {
        return Err(format!("{} has no report/notes.json: not a footage-agent project", old.display()).into());
    }
    let layout = Layout::new(new);
    if new.exists() && !layout.is_project() && std::fs::read_dir(new)?.next().is_some() {
        return Err(format!("{} exists and isn't a Dailix project: pick another name", new.display()).into());
    }
    let source: Option<serde_json::Value> = std::fs::read(old.join("report").join("_source.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let archive = source.as_ref().and_then(|s| s["archive"].as_str()).map(PathBuf::from).filter(|a| a.is_dir());
    let mut copies = Vec::new();
    let lrf = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lrf"));
    // The clips: the archive's days (LRF left there), then the old
    // project's own footage (days that never were in the archive).
    if let Some(archive) = &archive {
        let mut days: Vec<PathBuf> = std::fs::read_dir(archive)?.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().is_some_and(|n| dailix_sheet::is_day(&n.to_string_lossy()))).collect();
        days.sort();
        for day in days {
            tree(&day, &layout.footage().join(day.file_name().unwrap()), &lrf, &mut copies)?;
        }
    }
    let taken: std::collections::HashSet<PathBuf> = copies.iter().map(|c| c.to.clone()).collect();
    let mut own = Vec::new();
    tree(&old.join("01_footage"), &layout.footage(), &lrf, &mut own)?;
    copies.extend(own.into_iter().filter(|c| !taken.contains(&c.to)));
    for dir in CARRIED {
        tree(&old.join(dir), &new.join(dir), &|_| false, &mut copies)?;
    }
    for dir in PICTURES_FROM {
        tree(&old.join(dir), &new.join(dailix_sheet::layout::PICTURES), &|_| false, &mut copies)?;
    }
    tree(&old.join("analysis"), &layout.analysis(), &|_| false, &mut copies)?;
    tree(&old.join("voice"), &layout.voice(), &|_| false, &mut copies)?;
    Ok(Plan { old: old.to_owned(), layout, archive, copies })
}

/// Copies `c`, unless it's there already, whole.
pub(crate) fn copy(c: &Copy) -> std::io::Result<bool> {
    if std::fs::metadata(&c.to).is_ok_and(|m| m.len() == c.size) {
        return Ok(false);
    }
    let dir = c.to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let part = dir.join(format!(".{}.part", c.to.file_name().unwrap_or_default().to_string_lossy()));
    let result = std::fs::copy(&c.from, &part).and_then(|n| {
        if n != c.size {
            return Err(std::io::Error::other(format!("{}: copied {n} bytes of {}", c.from.display(), c.size)));
        }
        // On the disk before it gets its name: a copy renamed but not yet
        // written out, cut off by a crash, kept its size and lost its
        // content, and a rerun (which checks sizes) would have kept it
        // (2026-11-04, the machine froze mid-import).
        std::fs::File::open(&part)?.sync_all()?;
        std::fs::rename(&part, &c.to)
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|_| true)
}

/// Runs `plan`: the folders made, the files copied (`progress` told the
/// bytes done and in all after each), the marking carried over without
/// the web page's view state and with every mark given an id, and
/// `dailix/project.json` written last, so a project is complete once it
/// has one.
pub fn run(plan: &Plan, mut progress: impl FnMut(u64, u64)) -> Result<(), Error> {
    let layout = &plan.layout;
    layout.create()?;
    let total = plan.bytes();
    let mut done = 0;
    for c in &plan.copies {
        copy(c)?;
        done += c.size;
        progress(done, total);
    }
    // The marking: the old sheet as read, its page's view state dropped.
    if !layout.sheet().is_file() {
        let mut doc = Doc::parse(&std::fs::read_to_string(plan.old.join("report").join("notes.json"))?)?;
        doc.forget_view_state();
        std::fs::write(layout.sheet(), doc.text())?;
    }
    Store::new(layout.sheet()).read()?;
    // Into what's there: a name given since an earlier run stays.
    dailix_sheet::project::update(layout, |p| {
        p.insert("imported_from".into(), plan.old.to_string_lossy().into_owned().into());
        p.insert("sources".into(), plan.archive.as_ref().map(|a| a.to_string_lossy().into_owned()).into());
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, body: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn an_old_project_becomes_a_new_one_beside_it_and_runs_again_safely() {
        let base = std::env::temp_dir().join(format!("dailix-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (archive, old, new) = (base.join("archive"), base.join("old"), base.join("new"));
        write(&archive.join("2030-05-01/A.MP4"), b"aaaa");
        write(&archive.join("2030-05-01/A.LRF"), b"proxy");
        write(&archive.join("MANIFEST.md"), b"# trip");
        write(&archive.join("_edits/x.MOV"), b"edit");
        write(&old.join("report/_source.json"), format!(r#"{{"archive": "{}"}}"#, archive.display()).as_bytes());
        write(&old.join("report/notes.json"), br#"{"days": {}, "clips": {"2030-05-01/A.MP4": {"rating": 2, "marks": [{"t": 1, "end": 3, "note": "x"}]}}, "ui": {"shut": []}}"#);
        write(&old.join("report/thumbs/2030-05-01/A/0001.jpg"), b"t");
        write(&old.join("report/preview/2030-05-01/A.mp4"), b"preview");
        write(&old.join("01_footage/2030-04-30/phone.mp4"), b"pp");
        write(&old.join("02_audio/music/song.wav"), b"song");
        write(&old.join("08_graphics/logo.png"), b"png");
        write(&old.join("08_pictures/photo.jpg"), b"jpg");
        write(&old.join("05_export/film.mp4"), b"film");
        write(&old.join("analysis/2030-05-01/A.footage.json"), b"{}");
        write(&old.join("voice/2030-05-01/A/dnn6.flac"), b"v");
        write(&old.join("_trash/old.mp3"), b"t");

        let p = plan(&old, &new).unwrap();
        let to: Vec<String> = p.copies.iter().map(|c| c.to.strip_prefix(&new).unwrap().display().to_string()).collect();
        assert_eq!(to, ["01_footage/2030-05-01/A.MP4", "01_footage/2030-04-30/phone.mp4", "02_audio/music/song.wav", "08_pictures/logo.png", "08_pictures/photo.jpg", "dailix/analysis/2030-05-01/A.footage.json", "dailix/voice/2030-05-01/A/dnn6.flac"]);
        let mut seen = Vec::new();
        run(&p, |done, total| seen.push((done, total))).unwrap();
        assert_eq!(seen.last(), Some(&(p.bytes(), p.bytes())));
        assert_eq!(std::fs::read(new.join("01_footage/2030-05-01/A.MP4")).unwrap(), b"aaaa");
        // The marking: ids given, the page's view state gone, the rest kept.
        let sheet = std::fs::read_to_string(new.join("dailix/sheet.json")).unwrap();
        assert!(sheet.contains("\"id\": \"m1\"") && !sheet.contains("\"ui\"") && sheet.contains("\"rating\": 2"), "{sheet}");
        assert!(Layout::new(&new).is_project() && new.join("09_luts").is_dir());
        // The old project and the archive untouched; nothing left behind
        // that shouldn't come.
        assert!(old.join("report/notes.json").is_file() && archive.join("2030-05-01/A.LRF").is_file());
        assert!(!new.join("05_export/film.mp4").exists() && !new.join("_trash").exists());

        // Again: nothing to copy (all there), the marking as it was, the
        // name given since kept.
        dailix_sheet::project::set_title(&new, "Sea").unwrap();
        let again = plan(&old, &new).unwrap();
        run(&again, |_, _| {}).unwrap();
        assert_eq!(std::fs::read_to_string(new.join("dailix/sheet.json")).unwrap(), sheet);
        assert_eq!(dailix_sheet::project::title(&new), "Sea");
        // A folder that isn't a Dailix project isn't written into.
        write(&base.join("other/file"), b"x");
        assert!(plan(&old, &base.join("other")).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
