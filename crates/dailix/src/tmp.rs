//! TMP sorted into trips (Omnix gap 117): the videos that landed in the
//! TMP place, from cards, phones or a friend, each filed into a trip under
//! the day it was filmed, and into the trip's parts if asked («Rides on
//! Kos»: the scooter's videos, every day).
//!
//! Filing copies (`add::add`): TMP keeps the original till it goes to the
//! archive (`docs/INGEST.md`: never only one copy), and says where each
//! went in a ledger beside them, `.dailix-filed.json`. The window's sorter
//! and `dailix tmp` both work through here.

use crate::add::{self, Pick, Said, What};
use dailix_sheet::{Layout, Op, PartOp, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error>;

/// The ledger's name, in TMP's top folder.
pub const LEDGER: &str = ".dailix-filed.json";

/// A video in TMP: where it is (and from TMP's top, with `/`), the day it
/// was filmed if its name or its file says, its length.
#[derive(Debug, Clone, PartialEq)]
pub struct Raw {
    pub path: PathBuf,
    pub rel: String,
    pub day: Option<String>,
    pub duration: f64,
}

/// Where a video went: the trip's folder name, the day, its name there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filed {
    pub trip: String,
    pub day: String,
    pub name: String,
}

/// What TMP's videos became, by their place from TMP's top.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub files: BTreeMap<String, Vec<Filed>>,
    /// The videos copied into the archive and checked there (gap 119).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub archived: BTreeMap<String, crate::archive::Archived>,
    /// Each recorded video's size when it was: a place says which video
    /// only while the file there is that one (the audit, 2026-11-17: a
    /// new card's DJI_0001 at a freed one's place was taken for it, and
    /// «Free TMP» would have trashed it).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sizes: BTreeMap<String, u64>,
}

impl Ledger {
    /// The ledger in `tmp` to show: empty when there's none or it can't
    /// be read.
    pub fn load(tmp: &Path) -> Ledger {
        Ledger::load_checked(tmp).unwrap_or_default()
    }

    /// The ledger in `tmp` to change: empty when there's none, an error
    /// when there's one that can't be read (written over, every record of
    /// what went where would be gone: the audit, 2026-11-17).
    pub fn load_checked(tmp: &Path) -> Result<Ledger, String> {
        match std::fs::read_to_string(tmp.join(LEDGER)) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}; not written over", tmp.join(LEDGER).display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::default()),
            Err(e) => Err(format!("{}: {e}", tmp.join(LEDGER).display())),
        }
    }

    /// Written whole or not at all, on the disk before it takes the name
    /// (as `import::copy`), through a file of this process's own.
    pub fn save(&self, tmp: &Path) -> std::io::Result<()> {
        let part = tmp.join(format!("{LEDGER}.{}.part", std::process::id()));
        std::fs::write(&part, serde_json::to_string_pretty(self).map_err(std::io::Error::other)? + "\n")?;
        std::fs::File::open(&part)?.sync_all()?;
        std::fs::rename(part, tmp.join(LEDGER))
    }

    /// The ledger in `tmp` changed by `change` and written, read just
    /// before (the window and `dailix tmp` may both write it).
    pub fn update(tmp: &Path, change: impl FnOnce(&mut Ledger)) -> Result<(), String> {
        let mut l = Ledger::load_checked(tmp)?;
        change(&mut l);
        l.save(tmp).map_err(|e| format!("{}: {e}", tmp.join(LEDGER).display()))
    }

    /// Video `rel` forgotten: freed from TMP, its place may hold another.
    pub fn forget(&mut self, rel: &str) {
        self.files.remove(rel);
        self.archived.remove(rel);
        self.sizes.remove(rel);
    }

    /// Where the video at `rel` went, if anywhere.
    pub fn filed(&self, rel: &str) -> &[Filed] {
        self.files.get(rel).map_or(&[], Vec::as_slice)
    }

    /// Only what's still there (`there`: its copy is in its trip): a
    /// video whose copy was taken out of the trip is new in TMP again.
    pub fn keep(&self, there: impl Fn(&Filed) -> bool) -> Ledger {
        let files = self.files.iter().map(|(rel, went)| (rel.clone(), went.iter().filter(|f| there(f)).cloned().collect::<Vec<_>>())).filter(|(_, went)| !went.is_empty()).collect();
        // What's in the archive stays so, whatever became of the trip's copy.
        Ledger { files, archived: self.archived.clone(), sizes: self.sizes.clone() }
    }

    /// Only the videos whose place still holds them (`size`: the size of
    /// the file there now, none if it's gone): one of another size there
    /// is another video, new.
    pub fn same_files(&self, size: impl Fn(&str) -> Option<u64>) -> Ledger {
        let same = |rel: &str| match (self.sizes.get(rel), size(rel)) {
            (Some(was), Some(now)) => *was == now,
            (None, Some(_)) => true,
            (_, None) => false,
        };
        let files = self.files.iter().filter(|(r, _)| same(r)).map(|(r, f)| (r.clone(), f.clone())).collect();
        let archived = self.archived.iter().filter(|(r, _)| same(r)).map(|(r, a)| (r.clone(), a.clone())).collect();
        Ledger { files, archived, sizes: self.sizes.clone() }
    }

    /// The ledger in `tmp` as it stands now: only the copies still in
    /// their trips (in the `projects` place).
    pub fn now(tmp: &Path, projects: Option<&Path>) -> Ledger {
        let ledger = Ledger::load(tmp).same_files(|rel| std::fs::metadata(tmp.join(rel)).ok().map(|m| m.len()));
        match projects {
            Some(p) => ledger.keep(|f| Layout::new(p.join(&f.trip)).clip(&format!("{}/{}", f.day, f.name)).is_file()),
            None => ledger,
        }
    }

    /// `rel` went to `to`; said once however often it's filed there.
    pub fn record(&mut self, rel: &str, to: Filed) {
        let list = self.files.entry(rel.to_owned()).or_default();
        if !list.contains(&to) {
            list.push(to);
        }
    }
}

/// Every video under `tmp` (not in folders or files starting with `.`:
/// Dailix's own, a system's), by its place from the top.
pub fn videos(tmp: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs = vec![tmp.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => dirs.push(path),
                Ok(t) if t.is_file() && dailix_sheet::footage::is_video(&path) => found.push(path),
                _ => {}
            }
        }
    }
    found.sort();
    found
}

/// `path`'s place from `tmp`'s top, with `/` between folders.
pub fn rel(tmp: &Path, path: &Path) -> String {
    let r = path.strip_prefix(tmp).unwrap_or(path);
    r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// What ffprobe says of each of TMP's videos: its day and length (one
/// that can't be read keeps none, and shows under «No date»).
pub fn scan(tmp: &Path) -> Vec<Raw> {
    videos(tmp)
        .into_iter()
        .map(|path| {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let tech = add::probe(&path).ok();
            let day = dailix_sheet::footage::day_of(&name, tech.as_ref().and_then(|t| t.created_at.as_deref()));
            Raw { rel: rel(tmp, &path), day, duration: tech.map_or(0.0, |t| t.duration_s), path }
        })
        .collect()
}

/// The folders under `tmp` left empty (TMP freed of their videos) taken
/// away, deepest first; TMP itself and Dailix's own `.dailix` stay. How
/// many went.
pub fn drop_empty_folders(tmp: &Path) -> usize {
    let mut dirs = Vec::new();
    let mut todo = vec![tmp.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) && !e.file_name().to_string_lossy().starts_with('.') {
                todo.push(e.path());
                dirs.push(e.path());
            }
        }
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.into_iter().filter(|d| std::fs::remove_dir(d).is_ok()).count()
}

/// Where the poster of TMP's video `rel` is kept: in TMP's own
/// `.dailix/posters`, a name per place (`card/DJI_0001.MP4` →
/// `card__DJI_0001.MP4.jpg`), never beside the videos.
pub fn poster(tmp: &Path, rel: &str) -> PathBuf {
    tmp.join(".dailix").join("posters").join(format!("{}.jpg", rel.replace('/', "__")))
}

/// The month of the first of `days` (`2026-08`): a new trip for them is
/// named by when they were filmed, not by when they're sorted.
pub fn month_of<'a>(days: impl IntoIterator<Item = &'a str>) -> Option<String> {
    days.into_iter().filter(|d| dailix_sheet::footage::is_day(d)).min().map(|d| d[..7].to_owned())
}

/// What filing says while it copies: how far, into what, which file.
pub fn filing_line(done: usize, total: usize, trip: &str, now: &str) -> String {
    format!("Copying {} of {total} into {trip}: {now}", done + 1)
}

/// What a filed video's tile says, two things apart (the user,
/// 2026-11-17: «✓ trip» read as «in a part»): it's in the trip's days;
/// and the parts that hold it, or none.
pub fn went_lines(trip: &str, parts: &[&str]) -> (String, String) {
    let part = if parts.is_empty() { "In no part".to_owned() } else { format!("Part: {}", parts.join(", ")) };
    (format!("✓ In {trip}'s days"), part)
}

/// The player's share of the sorter's height, kept where both it and the
/// tiles stay usable.
pub fn player_share(share: f32) -> f32 {
    share.clamp(0.15, 0.75)
}

/// TMP's videos by day, the days in order and those with none last; each
/// day's by its place in TMP. Indices into `raws`.
pub fn by_day(raws: &[Raw]) -> Vec<(Option<String>, Vec<usize>)> {
    let mut days: BTreeMap<(bool, Option<String>), Vec<usize>> = BTreeMap::new();
    for (i, r) in raws.iter().enumerate() {
        days.entry((r.day.is_none(), r.day.clone())).or_default().push(i);
    }
    days.into_iter()
        .map(|((_, day), mut list)| {
            list.sort_by(|a, b| raws[*a].rel.cmp(&raws[*b].rel));
            (day, list)
        })
        .collect()
}

/// How many of `rels` the ledger has no place for: what's new in TMP.
pub fn new_count<'a>(ledger: &Ledger, rels: impl IntoIterator<Item = &'a str>) -> usize {
    rels.into_iter().filter(|r| crate::archive::stage(ledger, r) == crate::archive::Stage::New).count()
}

/// The sorter's tiles picked after a click on `at` (`order`: the tiles as
/// shown): alone, it alone; with Ctrl, it added or taken away; with Shift,
/// every tile from the last one clicked (`anchor`) to it. The picked in
/// the order shown, and the new anchor.
pub fn click<T: Copy + PartialEq>(picked: &[T], anchor: Option<T>, at: T, order: &[T], shift: bool, ctrl: bool) -> (Vec<T>, Option<T>) {
    let place = |i: T| order.iter().position(|o| *o == i);
    let in_order = |set: Vec<T>| order.iter().copied().filter(|o| set.contains(o)).collect::<Vec<_>>();
    match (shift, anchor.and_then(place), place(at)) {
        (true, Some(a), Some(b)) => {
            let (lo, hi) = (a.min(b), a.max(b));
            let mut set = if ctrl { picked.to_vec() } else { Vec::new() };
            set.extend(&order[lo..=hi]);
            (in_order(set), anchor)
        }
        _ if ctrl => {
            let set = if picked.contains(&at) { picked.iter().copied().filter(|p| *p != at).collect() } else { [picked, &[at]].concat() };
            (in_order(set), Some(at))
        }
        _ => (vec![at], Some(at)),
    }
}

/// The tile `by` places from `from` in `order` (the first with none),
/// kept within it.
pub fn step(order: &[usize], from: Option<usize>, by: isize) -> Option<usize> {
    let last = order.len().checked_sub(1)?;
    let at = from.and_then(|f| order.iter().position(|o| *o == f));
    let next = at.map_or(0, |a| a.saturating_add_signed(by).min(last));
    order.get(next).copied()
}

/// The clip a filed video is in its trip (`<day>/<name>`), once it's
/// there: added, there already, or seen.
fn clip_key(said: &Said) -> Option<String> {
    let there = matches!(said.what, What::Added(_) | What::There | What::Seen);
    there.then(|| said.day.as_ref().map(|d| format!("{d}/{}", said.name))).flatten()
}

/// The ops putting `clips` into `parts`, each part made first if the trip
/// hasn't it (`known`: the trip's parts now).
pub fn part_ops(known: &[String], parts: &[String], clips: &[String]) -> Vec<Op> {
    let mut ops = Vec::new();
    for part in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !known.iter().any(|k| dailix_sheet::parts::same_name(k, part)) && !ops.iter().any(|op| matches!(op, Op::Part(PartOp::Add { name }) if dailix_sheet::parts::same_name(name, part))) {
            ops.push(Op::Part(PartOp::Add { name: part.to_owned() }));
        }
        if !clips.is_empty() {
            ops.push(Op::Part(PartOp::Put { part: part.to_owned(), clips: clips.to_vec() }));
        }
    }
    ops
}

/// Files `files` (TMP's videos) into the trip at `root`, each under its
/// day (or `day`), and into `parts`; the ledger in `tmp` says where each
/// went. A line said for each, as `add` says it.
pub fn file(tmp: &Path, root: &Path, files: &[PathBuf], day: Option<&str>, parts: &[String], sources: Option<&Path>) -> Result<Vec<Said>, Error> {
    let picks: Vec<Pick> = files.iter().map(|f| Pick { file: f.clone(), span: None }).collect();
    let said = add::add(root, &picks, day)?;
    // The ledger first: the copies are made, whatever the parts do.
    let trip = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ledger::update(tmp, |ledger| {
        for (f, s) in files.iter().zip(&said) {
            if clip_key(s).is_some()
                && let Some(d) = &s.day
            {
                let r = rel(tmp, f);
                // A place now holding another file: what was recorded there goes.
                let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or_default();
                if ledger.sizes.get(&r).is_some_and(|was| *was != size) {
                    ledger.forget(&r);
                }
                ledger.sizes.insert(r.clone(), size);
                ledger.record(&r, Filed { trip: trip.clone(), day: d.clone(), name: s.name.clone() });
            }
        }
    })?;
    // The posters the sorter made, kept: the trip's day cards show them
    // at once, not when the sheet first opens (the same frame, a third in).
    let layout = Layout::new(root);
    for (f, s) in files.iter().zip(&said) {
        if let Some(key) = clip_key(s)
            && !layout.poster(&key).is_file()
            && poster(tmp, &rel(tmp, f)).is_file()
        {
            let to = layout.poster(&key);
            let _ = to.parent().map(std::fs::create_dir_all);
            let _ = std::fs::copy(poster(tmp, &rel(tmp, f)), to);
        }
    }
    let moved = follow_into_trip(tmp, files, sources);
    let keys: Vec<String> = said.iter().filter_map(clip_key).collect();
    let store = Store::new(layout.sheet());
    let known: Vec<String> = store.read().ok().and_then(|d| d.sheet().ok()).map(|s| s.parts.into_iter().map(|p| p.name).collect()).unwrap_or_default();
    let ops = part_ops(&known, parts, &keys);
    if !ops.is_empty() {
        store.apply(&ops)?;
    }
    moved?;
    Ok(said)
}

/// The archive copies of `files` kept under `_not-in-a-trip` moved into
/// the trip and day each was just filed under (archive::relocate), the
/// ledger saying where they are now.
fn follow_into_trip(tmp: &Path, files: &[PathBuf], sources: Option<&Path>) -> Result<(), String> {
    let Some(sources) = sources else { return Ok(()) };
    let ledger = Ledger::load_checked(tmp)?;
    for f in files {
        let r = rel(tmp, f);
        let (Some(a), Some(filed)) = (ledger.archived.get(&r), ledger.filed(&r).first()) else { continue };
        if !crate::archive::not_in_a_trip(a, sources) {
            continue;
        }
        let moved = crate::archive::relocate(a, filed, sources).map_err(|e| format!("filed, but its archive copy didn't follow: {e}"))?;
        Ledger::update(tmp, |l| {
            l.archived.insert(r.clone(), moved);
        })?;
    }
    Ok(())
}

/// TMP as text, a day at a time: each video, its length, and where it
/// went (or «new»). What `dailix tmp` prints.
pub fn describe(raws: &[Raw], ledger: &Ledger) -> String {
    let mut out = String::new();
    for (day, list) in by_day(raws) {
        out.push_str(&format!("{}  {}\n", day.as_deref().unwrap_or("no date"), crate::library::count(list.len(), "video")));
        for i in list {
            let r = &raws[i];
            let went: Vec<String> = ledger.filed(&r.rel).iter().map(|f| format!("{}/{}/{}", f.trip, f.day, f.name)).collect();
            let went = if went.is_empty() { "new".to_owned() } else { format!("→ {}", went.join(", ")) };
            out.push_str(&format!("  {}  {:.1} s  {went}\n", r.rel, r.duration));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(rel: &str, day: Option<&str>) -> Raw {
        Raw { path: PathBuf::from("/tmp-place").join(rel), rel: rel.into(), day: day.map(str::to_owned), duration: 10.0 }
    }

    #[test]
    fn tmps_videos_go_by_day_and_those_with_none_last() {
        let raws = [raw("phone/b.mp4", Some("2026-08-19")), raw("friend/x.mp4", None), raw("card/DJI_2.MP4", Some("2026-08-18")), raw("card/DJI_1.MP4", Some("2026-08-19"))];
        assert_eq!(by_day(&raws), vec![(Some("2026-08-18".into()), vec![2]), (Some("2026-08-19".into()), vec![3, 0]), (None, vec![1])]);
    }

    #[test]
    fn the_ledger_says_where_each_went_once() {
        let mut l = Ledger::default();
        let to = Filed { trip: "2026-08_kos".into(), day: "2026-08-22".into(), name: "DJI_0001.MP4".into() };
        l.record("card/DJI_0001.MP4", to.clone());
        l.record("card/DJI_0001.MP4", to.clone());
        assert_eq!(l.filed("card/DJI_0001.MP4"), [to]);
        assert!(l.filed("card/DJI_0002.MP4").is_empty());
        assert_eq!(new_count(&l, ["card/DJI_0001.MP4", "card/DJI_0002.MP4"]), 1);
        let dir = std::env::temp_dir().join(format!("dailix-ledger-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        l.save(&dir).unwrap();
        assert_eq!(Ledger::load(&dir), l);
        assert_eq!(Ledger::load(&dir.join("none")), Ledger::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn another_file_at_a_videos_place_is_a_new_video() {
        let mut l = Ledger::default();
        l.record("card/DJI_0001.MP4", Filed { trip: "kos".into(), day: "2026-08-22".into(), name: "DJI_0001.MP4".into() });
        l.sizes.insert("card/DJI_0001.MP4".into(), 100);
        l.archived.insert("card/DJI_0001.MP4".into(), crate::archive::Archived { path: "/s/x".into(), hash: "h".into() });
        let same = l.same_files(|_| Some(100));
        assert_eq!(crate::archive::stage(&same, "card/DJI_0001.MP4"), crate::archive::Stage::Archived);
        let other = l.same_files(|_| Some(222));
        assert_eq!(crate::archive::stage(&other, "card/DJI_0001.MP4"), crate::archive::Stage::New);
        assert_eq!(crate::archive::stage(&l.same_files(|_| None), "card/DJI_0001.MP4"), crate::archive::Stage::New);
        l.forget("card/DJI_0001.MP4");
        assert_eq!(l, Ledger::default());
    }

    #[test]
    fn a_ledger_that_cant_be_read_is_never_written_over() {
        let dir = std::env::temp_dir().join(format!("dailix-ledger-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(LEDGER), b"{ not json").unwrap();
        assert!(Ledger::update(&dir, |l| l.record("a", Filed { trip: "t".into(), day: "d".into(), name: "n".into() })).is_err());
        assert_eq!(std::fs::read(dir.join(LEDGER)).unwrap(), b"{ not json");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_video_whose_copy_left_its_trip_is_new_again() {
        let mut l = Ledger::default();
        let at = |trip: &str| Filed { trip: trip.into(), day: "2026-08-22".into(), name: "a.MP4".into() };
        l.record("card/a.MP4", at("kos"));
        l.record("card/a.MP4", at("island"));
        l.record("card/b.MP4", at("island"));
        let now = l.keep(|f| f.trip == "kos");
        assert_eq!(now.filed("card/a.MP4"), [at("kos")]);
        assert!(now.filed("card/b.MP4").is_empty() && !now.files.contains_key("card/b.MP4"));
    }

    #[test]
    fn a_part_the_trip_hasnt_is_made_before_its_clips_go_in() {
        let clips = vec!["2026-08-22/DJI_0001.MP4".to_owned()];
        let ops = part_ops(&["Beach".into()], &["beach".into(), " Rides ".into(), "rides".into(), "".into()], &clips);
        assert_eq!(
            ops,
            [
                Op::Part(PartOp::Put { part: "beach".into(), clips: clips.clone() }),
                Op::Part(PartOp::Add { name: "Rides".into() }),
                Op::Part(PartOp::Put { part: "Rides".into(), clips: clips.clone() }),
                Op::Part(PartOp::Put { part: "rides".into(), clips: clips.clone() }),
            ]
        );
        // Nothing filed: the part is still made, empty.
        assert_eq!(part_ops(&[], &["Rides".into()], &[]), [Op::Part(PartOp::Add { name: "Rides".into() })]);
    }

    #[test]
    fn only_a_video_in_the_trip_goes_into_its_parts() {
        let s = |what| Said { day: Some("2026-08-22".into()), name: "a.mp4".into(), what };
        assert_eq!(clip_key(&s(What::Added(3.0))).as_deref(), Some("2026-08-22/a.mp4"));
        assert_eq!(clip_key(&s(What::There)).as_deref(), Some("2026-08-22/a.mp4"));
        assert_eq!(clip_key(&s(What::Failed("no".into()))), None);
        assert_eq!(clip_key(&Said { day: None, name: "x".into(), what: What::Failed("no date".into()) }), None);
    }

    #[test]
    fn videos_are_found_below_tmp_and_dailixs_own_files_are_not() {
        let dir = std::env::temp_dir().join(format!("dailix-tmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in ["card/DJI_0001.MP4", "card/DJI_0001.LRF", "phone/a.mp4", ".dailix/posters/x.mp4", "notes.txt"] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        let found: Vec<String> = videos(&dir).iter().map(|p| rel(&dir, p)).collect();
        assert_eq!(found, ["card/DJI_0001.MP4", "phone/a.mp4"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_click_picks_one_ctrl_adds_shift_takes_the_run() {
        let order = [4, 2, 0, 1, 3];
        assert_eq!(click(&[], None, 0, &order, false, false), (vec![0], Some(0)));
        // Shift from 2 to 1: the run as shown, whichever way.
        assert_eq!(click(&[2], Some(2), 1, &order, true, false), (vec![2, 0, 1], Some(2)));
        assert_eq!(click(&[1], Some(1), 4, &order, true, false), (vec![4, 2, 0, 1], Some(1)));
        // Ctrl adds and takes away, in the order shown.
        assert_eq!(click(&[3], Some(3), 4, &order, false, true), (vec![4, 3], Some(4)));
        assert_eq!(click(&[4, 3], Some(4), 4, &order, false, true), (vec![3], Some(4)));
        // Shift with no anchor: just the one.
        assert_eq!(click(&[], None, 3, &order, true, false), (vec![3], Some(3)));
    }

    #[test]
    fn the_arrows_step_through_the_tiles_and_stop_at_the_ends() {
        let order = [4, 2, 0];
        assert_eq!((step(&order, None, 1), step(&order, Some(2), 1), step(&order, Some(0), 1), step(&order, Some(4), -1)), (Some(4), Some(0), Some(0), Some(4)));
        assert_eq!(step(&[], None, 1), None);
    }

    #[test]
    fn a_new_trip_is_named_by_the_month_its_videos_were_filmed() {
        assert_eq!(month_of(["2026-09-01", "2026-08-30", "soon"]).as_deref(), Some("2026-08"));
        assert_eq!(month_of([]), None);
    }

    #[test]
    fn the_sorter_says_how_far_filing_is_and_where_a_tile_went() {
        assert_eq!(filing_line(2, 7, "Kos", "DJI_0001.MP4"), "Copying 3 of 7 into Kos: DJI_0001.MP4");
        assert_eq!(went_lines("Kos", &[]), ("✓ In Kos's days".to_owned(), "In no part".to_owned()));
        assert_eq!(went_lines("Kos", &["Rides", "Beach"]).1, "Part: Rides, Beach");
        assert_eq!((player_share(0.05), player_share(0.4), player_share(0.9)), (0.15, 0.4, 0.75));
    }

    #[test]
    fn an_emptied_tmp_loses_its_empty_folders_not_its_own() {
        let dir = std::env::temp_dir().join(format!("dailix-tmp-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["card/DCIM/100", "phone", ".dailix/posters", "kept"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        std::fs::write(dir.join("kept/a.mp4"), b"x").unwrap();
        assert_eq!(drop_empty_folders(&dir), 4);
        assert!(dir.join(".dailix/posters").is_dir() && dir.join("kept/a.mp4").is_file() && !dir.join("card").exists() && dir.is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tmp_is_described_a_day_at_a_time() {
        let raws = [raw("card/DJI_1.MP4", Some("2026-08-19")), raw("friend/x.mp4", None)];
        let mut l = Ledger::default();
        l.record("card/DJI_1.MP4", Filed { trip: "kos".into(), day: "2026-08-19".into(), name: "DJI_1.MP4".into() });
        assert_eq!(describe(&raws, &l), "2026-08-19  1 video\n  card/DJI_1.MP4  10.0 s  → kos/2026-08-19/DJI_1.MP4\nno date  1 video\n  friend/x.mp4  10.0 s  new\n");
    }
}
