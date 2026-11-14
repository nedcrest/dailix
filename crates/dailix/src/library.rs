//! What the home view shows (UI-RESEARCH 3, 4): the trips, each a Dailix
//! project in the projects place, as rows of day cards (how many clips,
//! how many rated, whether the day's story is written, where the files
//! are), and the places with their state. Only read: the home view
//! changes nothing (it doesn't even give marks their ids).

use dailix_sheet::footage;
use crate::settings::{PlaceKind, Settings};
use dailix_sheet::{Doc, Layout};
use std::path::{Path, PathBuf};

/// A day of a trip, summed up.
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub day: String,
    pub clips: usize,
    pub rated: usize,
    /// The day's story is written.
    pub story: bool,
    /// The first clip's poster that's made, for the card.
    pub poster: Option<PathBuf>,
    /// Its clips found in the project's footage.
    pub in_project: usize,
    /// Its folder in the sources.
    pub in_sources: InSources,
}

/// A day's folder in the sources: there or missing; unknown while their
/// folder is away; nothing to say for a trip that names none (one made
/// with `dailix add`: the card said «sources offline» there, as if a disk
/// were missing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InSources {
    There,
    Missing,
    Offline,
    NotNamed,
}

impl InSources {
    fn of(sources: Option<&Path>, day: &str) -> InSources {
        match sources {
            None => InSources::NotNamed,
            Some(s) if !s.is_dir() => InSources::Offline,
            Some(s) if s.join(day).is_dir() => InSources::There,
            Some(_) => InSources::Missing,
        }
    }
}

/// A trip: a project, what was written about it, its days and its edits.
#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    pub root: PathBuf,
    pub title: String,
    pub about: dailix_sheet::project::About,
    pub days: Vec<Day>,
    pub edits: Vec<PathBuf>,
    /// Its card's frames (Omnix's project cards, gap 110): up to 16 of
    /// its clips' posters made so far, spread over its days in order.
    pub frames: omnix::cards::view::Frames,
}

impl Trip {
    pub fn clips(&self) -> usize {
        self.days.iter().map(|d| d.clips).sum()
    }

    /// Its first and last day: «a – b», or one day.
    pub fn dates(&self) -> String {
        match (self.days.first(), self.days.last()) {
            (Some(a), Some(b)) if a.day != b.day => format!("{} – {}", a.day, b.day),
            (Some(a), _) => a.day.clone(),
            _ => String::new(),
        }
    }
}

/// The trips in the projects place `projects`, by name (dated names sort
/// by date). One that can't be read is left out, said in the second value.
pub fn trips(projects: &Path) -> (Vec<Trip>, Vec<String>) {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for root in dailix_sheet::project::all(projects) {
        match trip(&root) {
            Ok(t) => out.push(t),
            Err(e) => skipped.push(format!("{}: {e}", root.display())),
        }
    }
    (out, skipped)
}

/// One trip, read.
pub fn trip(root: &Path) -> Result<Trip, String> {
    let layout = Layout::new(root);
    let (footage, _) = footage::read(&layout).map_err(|e| e.to_string())?;
    let sheet = std::fs::read_to_string(layout.sheet()).map_err(|e| e.to_string()).and_then(|t| Doc::parse(&t).map_err(|e| e.to_string())).and_then(|d| d.sheet().map_err(|e| e.to_string()))?;
    let sources = dailix_sheet::project::read(&layout).ok().and_then(|p| p.get("sources").and_then(|s| s.as_str()).map(PathBuf::from));
    let title = dailix_sheet::project::title(root);
    let days = footage
        .days
        .iter()
        .map(|d| Day {
            day: d.day.clone(),
            clips: d.clips.len(),
            rated: d.clips.iter().filter(|c| sheet.clips.get(&c.key).is_some_and(|s| s.rating.is_some())).count(),
            story: sheet.days.get(&d.day).is_some_and(|n| !n.note.trim().is_empty()),
            poster: d.clips.iter().map(|c| c.poster.clone()).find(|p| p.is_file()),
            in_project: d.clips.iter().filter(|c| c.path.is_file()).count(),
            in_sources: InSources::of(sources.as_deref(), &d.day),
        })
        .collect();
    let posters: Vec<PathBuf> = footage.days.iter().flat_map(|d| d.clips.iter().map(|c| c.poster.clone())).filter(|p| p.is_file()).collect();
    let frames = omnix::cards::view::Frames::from_files(&omnix::cards::spread(&posters, omnix::cards::FRAMES));
    Ok(Trip { root: root.to_owned(), title, about: dailix_sheet::project::about(root), days, edits: dailix_sheet::project::edits(root), frames })
}

/// `n` of `one` in words: «1 clip», «2 clips», «0 clips».
pub fn count(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// The trips as text, a line per trip and per day: what the home view
/// shows, for the command line, tests and an AI.
pub fn describe(trips: &[Trip]) -> String {
    let mut out = String::new();
    for t in trips {
        out.push_str(&format!("{}  {}  {}\n", t.title, count(t.days.len(), "day"), count(t.clips(), "clip")));
        for d in &t.days {
            let sources = match d.in_sources {
                InSources::There => "sources",
                InSources::Missing => "not in sources",
                InSources::Offline => "sources offline",
                InSources::NotNamed => "",
            };
            let line = format!("  {}  {}  {} rated  {}  project {}/{}  {sources}", d.day, count(d.clips, "clip"), d.rated, if d.story { "story" } else { "no story" }, d.in_project, d.clips);
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out
}

/// A place (TMP, the projects, the sources): where, whether it's there
/// now, and its free space.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    pub path: Option<PathBuf>,
    pub online: bool,
    pub free: Option<u64>,
}

/// The three places as set.
pub fn places(s: &Settings) -> Vec<Place> {
    PlaceKind::ALL
        .into_iter()
        .map(|kind| {
            let path = s.place(kind);
            let online = path.as_ref().is_some_and(|p| p.is_dir());
            let free = path.as_deref().filter(|_| online).and_then(free_space);
            Place { kind, path, online, free }
        })
        .collect()
}

/// Whether a place needs a look: away, not set, or under 10 GB free.
pub fn place_trouble(p: &Place) -> bool {
    !p.online || p.free.is_some_and(|f| f < 10_000_000_000)
}

/// Where a place goes if the user takes the defaults: one `Dailix` folder in
/// `home`, a folder for each place in it.
pub fn default_place(kind: PlaceKind, home: &Path) -> PathBuf {
    home.join("Dailix").join(kind.name().to_lowercase())
}

/// The places not set yet, each with its default folder in `home`: what
/// «Use these folders» sets (and makes).
pub fn defaults_for(places: &[Place], home: &Path) -> Vec<(PlaceKind, PathBuf)> {
    places.iter().filter(|p| p.path.is_none()).map(|p| (p.kind, default_place(p.kind, home))).collect()
}

/// `path` as people say it: the home folder as `~`.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => Path::new("~").join(rest).display().to_string(),
        _ => path.display().to_string(),
    }
}

/// `path` cut to `most` characters in the middle, so its start and its
/// last folder (the part people know) stay: `/home/someone/…/projects`.
pub fn middle(path: &str, most: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= most || most < 5 {
        return path.to_owned();
    }
    let tail = (most - 1) / 2 + (most - 1) % 2;
    let head = most - 1 - tail;
    chars[..head].iter().chain(['…'].iter()).chain(&chars[chars.len() - tail..]).collect()
}

/// What Home shows before any trip can.
#[derive(Debug, PartialEq)]
pub enum FirstRun {
    /// A stranger's first run, the projects place not set: the welcome,
    /// each place to choose.
    Welcome,
    /// Something to say, and the place to choose if one is wanted: a disk
    /// not plugged in, an empty projects place.
    Say(String, Option<PlaceKind>),
}

/// What Home shows before any trip can; none once there are trips.
pub fn first_run(places: &[Place], trips: usize) -> Option<FirstRun> {
    if trips > 0 {
        return None;
    }
    let projects = places.iter().find(|p| p.kind == PlaceKind::Projects)?;
    Some(match (&projects.path, projects.online) {
        (None, _) => FirstRun::Welcome,
        (Some(p), false) => FirstRun::Say(format!("The Trips folder {} isn't there: is its disk plugged in?", p.display()), Some(PlaceKind::Projects)),
        (Some(p), true) => FirstRun::Say(format!("No trips in {} yet: + New trip makes one.", p.display()), None),
    })
}

/// Free bytes on the disk holding `path` (Unix; none elsewhere, for now).
#[cfg(unix)]
pub fn free_space(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path, `s` a statvfs to fill.
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(not(unix))]
pub fn free_space(_: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_said_one_or_many() {
        assert_eq!((count(1, "clip"), count(2, "clip"), count(0, "edit")), ("1 clip".to_owned(), "2 clips".to_owned(), "0 edits".to_owned()));
    }

    #[test]
    fn the_home_folder_is_said_as_a_tilde() {
        let home = Path::new("/home/someone");
        assert_eq!(tilde(Path::new("/home/someone/Dailix/projects"), home), "~/Dailix/projects");
        assert_eq!(tilde(Path::new("/media/disk/trips"), home), "/media/disk/trips");
    }

    #[test]
    fn a_trip_naming_no_sources_says_nothing_of_them() {
        let here = std::env::temp_dir();
        assert_eq!(InSources::of(None, "2030-05-01"), InSources::NotNamed);
        assert_eq!(InSources::of(Some(&here.join("no-such-sources-folder")), "2030-05-01"), InSources::Offline);
        assert_eq!(InSources::of(Some(&here), "no-such-day-2030-05-01"), InSources::Missing);
    }

    #[test]
    fn a_place_needs_a_look_when_away_unset_or_nearly_full() {
        let p = |online, free| Place { kind: crate::settings::PlaceKind::Tmp, path: None, online, free };
        assert!(!place_trouble(&p(true, Some(200_000_000_000))) && !place_trouble(&p(true, None)));
        assert!(place_trouble(&p(false, None)) && place_trouble(&p(true, Some(3_000_000_000))));
    }

    #[test]
    fn a_long_path_is_cut_in_the_middle_keeping_its_last_folder() {
        assert_eq!(middle("/home/someone/Dailix/projects", 40), "/home/someone/Dailix/projects");
        let cut = middle("/media/someone/Big Disk/Trips/2026/everything/projects", 30);
        assert_eq!(cut.chars().count(), 30);
        assert!(cut.starts_with("/media/") && cut.ends_with("/projects") && cut.contains('…'), "{cut}");
    }

    #[test]
    fn the_defaults_are_one_folder_in_home_and_only_for_places_not_set() {
        let home = Path::new("/home/someone");
        assert_eq!(default_place(PlaceKind::Projects, home), PathBuf::from("/home/someone/Dailix/projects"));
        assert_eq!(default_place(PlaceKind::Tmp, home), PathBuf::from("/home/someone/Dailix/tmp"));
        let places = [Place { kind: PlaceKind::Tmp, path: Some("/fast".into()), online: true, free: None }, Place { kind: PlaceKind::Sources, path: None, online: false, free: None }];
        assert_eq!(defaults_for(&places, home), [(PlaceKind::Sources, PathBuf::from("/home/someone/Dailix/sources"))]);
    }

    #[test]
    fn a_first_run_says_which_folder_to_choose_and_why() {
        let place = |kind, path: Option<&str>, online| Place { kind, path: path.map(PathBuf::from), online, free: None };
        let unset = [place(PlaceKind::Tmp, None, false), place(PlaceKind::Projects, None, false), place(PlaceKind::Sources, None, false)];
        assert_eq!(first_run(&unset, 0), Some(FirstRun::Welcome));
        let away = [place(PlaceKind::Projects, Some("/trips"), false)];
        assert!(matches!(first_run(&away, 0), Some(FirstRun::Say(_, Some(PlaceKind::Projects)))), "its disk not plugged in");
        let empty = [place(PlaceKind::Projects, Some("/trips"), true)];
        assert_eq!(first_run(&empty, 0), Some(FirstRun::Say("No trips in /trips yet: + New trip makes one.".to_owned(), None)));
        assert_eq!(first_run(&empty, 2), None, "trips to show");
    }

    #[test]
    fn a_trip_reads_as_days_with_their_counts_and_places() {
        let base = std::env::temp_dir().join(format!("dailix-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (projects, sources) = (base.join("projects"), base.join("sources"));
        let root = projects.join("2030-05_sea");
        let layout = Layout::new(&root);
        layout.create().unwrap();
        std::fs::write(layout.project(), format!(r#"{{"version": 1, "sources": "{}"}}"#, sources.display())).unwrap();
        std::fs::create_dir_all(sources.join("2030-05-01")).unwrap();
        let clip = |day: &str, file: &str, on_disk: bool| {
            let dir = layout.analysis().join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let stem = file.split('.').next().unwrap();
            std::fs::write(dir.join(format!("{stem}.footage.json")), format!(r#"{{"source": "{file}", "day": "{day}", "technical": {{"duration_s": 5}}}}"#)).unwrap();
            if on_disk {
                let p = layout.clip(&format!("{day}/{file}"));
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, b"v").unwrap();
            }
        };
        clip("2030-05-01", "A.MP4", true);
        clip("2030-05-01", "B.MP4", true);
        clip("2030-05-02", "C.MP4", false);
        std::fs::write(layout.sheet(), r#"{"days": {"2030-05-01": {"note": "the sea"}}, "clips": {"2030-05-01/A.MP4": {"rating": 2}, "2030-05-01/B.MP4": {"rating": null}}}"#).unwrap();
        // Not a project: left out.
        std::fs::create_dir_all(projects.join("loose")).unwrap();

        let (trips, skipped) = trips(&projects);
        assert!(skipped.is_empty());
        assert_eq!(trips.len(), 1);
        let t = &trips[0];
        assert_eq!((t.title.as_str(), t.clips()), ("2030-05_sea", 3));
        let d = &t.days[0];
        assert_eq!((d.clips, d.rated, d.story, d.in_project, d.in_sources), (2, 1, true, 2, InSources::There));
        let d = &t.days[1];
        assert_eq!((d.rated, d.story, d.in_project, d.in_sources), (0, false, 0, InSources::Missing));
        // The sheet wasn't written (no ids given).
        assert!(!std::fs::read_to_string(layout.sheet()).unwrap().contains("\"id\""));
        assert_eq!(
            describe(&trips),
            "2030-05_sea  2 days  3 clips\n  2030-05-01  2 clips  1 rated  story  project 2/2  sources\n  2030-05-02  1 clip  0 rated  no story  project 0/1  not in sources\n"
        );

        // Its name as people say it, when it has one.
        dailix_sheet::project::set_title(&root, "Sea, May 2030").unwrap();
        assert_eq!(super::trips(&projects).0[0].title, "Sea, May 2030");

        let s = Settings { projects: Some(projects.clone()), sources: Some(crate::settings::Sources::Folder { path: base.join("gone") }), ..Default::default() };
        let p = places(&s);
        assert_eq!(p.iter().map(|p| (p.kind.name(), p.online)).collect::<Vec<_>>(), [("projects", true), ("TMP", false), ("sources", false)]);
        assert!(p[0].free.is_some_and(|f| f > 0));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
