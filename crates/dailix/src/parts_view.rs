//! A trip's parts in the window (Omnix gap 117): on the trip's page a row
//! for each (its clips, its days, «→ Omnix» of it, the sheet showing it
//! alone); in the contact sheet a «Part» filter beside the ratings', and
//! the picked clip's parts to tick in the pane. Every change is a
//! `PartOp` through the sheet's core; a part's edit is Omnix's
//! `from_sheet::write_part`.

use super::{App, Message, blocking, tip};
use crate::library::Trip;
use crate::ui;
use dailix_sheet::{Op, PartOp};
use iced::widget::{checkbox, column, pick_list, row, space, text, text_input, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::path::PathBuf;

/// The field a part is renamed in, focused when it opens.
const PART_NAME: &str = "part-name";

#[derive(Debug, Clone)]
pub enum PartMsg {
    /// The contact sheet shows one part's clips (none: every clip).
    Filter(Option<String>),
    /// A clip put into a part, or taken out.
    Clip(String, String, bool),
    /// A trip's part opened in the contact sheet, alone.
    Open(PathBuf, String),
    /// A part's edit made and opened in Omnix; made (or why not).
    ToOmnix(PathBuf, String),
    Made(String, Result<PathBuf, String>),
    /// A part's name clicked: renamed there (typed, kept with Enter).
    Rename(PathBuf, String),
    Typed(String),
    RenameSave,
    /// A part taken away; its clips stay in the trip.
    Remove(PathBuf, String),
    /// The question before a part goes: cancelled, the part only, or the
    /// part and the videos only it holds.
    RemoveCancel,
    RemoveConfirm(bool),
    /// The pointer on a part's card, or a click on it (opened in the sheet).
    Card(PathBuf, String, omnix::cards::view::Event),
}

/// The filter's choice: every clip, or a part's.
#[derive(Debug, Clone, PartialEq)]
pub struct PartChoice(pub Option<String>);

impl std::fmt::Display for PartChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_deref().unwrap_or("Every part"))
    }
}

/// The days a part's clips are from, in order, each once.
pub fn days_of(clips: &[String]) -> Vec<&str> {
    let mut days: Vec<&str> = clips.iter().filter_map(|c| c.split_once('/').map(|(d, _)| d)).collect();
    days.sort_unstable();
    days.dedup();
    days
}

/// A part asked about before it goes: the videos only it holds (none
/// rated, noted or marked: those stay), and how many stay for being
/// elsewhere or marked.
#[derive(Debug, Clone)]
pub struct Removal {
    pub root: PathBuf,
    pub part: String,
    pub gone: Vec<String>,
    pub kept: usize,
}

/// The clips of part `part` that can leave the trip with it: in no other
/// part, nothing the user did to them in the sheet (a rating, a note, a
/// mark), and not one `keep` holds (an edit uses it, or its original is
/// in neither TMP nor the archive, so this copy may be the only one: the
/// audit, 2026-11-17); and how many of its clips stay.
pub fn removable(sheet: &dailix_sheet::Sheet, part: &str, keep: impl Fn(&str) -> bool) -> (Vec<String>, usize) {
    let Some(p) = sheet.part(part) else { return (Vec::new(), 0) };
    let untouched = |k: &String| sheet.clips.get(k).is_none_or(|c| c.rating.is_none() && c.note.is_empty() && c.marks.is_empty());
    let elsewhere = |k: &String| sheet.parts.iter().any(|o| !dailix_sheet::parts::same_name(&o.name, part) && o.clips.contains(k));
    let gone: Vec<String> = p.clips.iter().filter(|k| untouched(k) && !elsewhere(k) && !keep(k)).cloned().collect();
    let kept = p.clips.len() - gone.len();
    (gone, kept)
}

/// The key a part's card is hovered by (beside trips' and edits' paths).
pub fn card_key(root: &std::path::Path, part: &str) -> PathBuf {
    root.join(".part").join(part)
}

/// Whether the sheet's part filter `part` shows clip `key` (`parts`: the
/// sheet's parts; a part since gone shows every clip).
pub fn part_shows(parts: &[dailix_sheet::Part], part: Option<&str>, key: &str) -> bool {
    let Some(name) = part else { return true };
    parts.iter().find(|p| dailix_sheet::parts::same_name(&p.name, name)).is_none_or(|p| p.clips.iter().any(|c| c == key))
}

impl App {
    pub(super) fn on_part(&mut self, msg: PartMsg) -> Task<Message> {
        match msg {
            PartMsg::Filter(part) => self.part = part,
            PartMsg::Clip(clip, part, on) => {
                let clips = vec![clip];
                let op = if on { PartOp::Put { part, clips } } else { PartOp::Take { part, clips } };
                self.change(Op::Part(op));
                self.refresh_home();
            }
            PartMsg::Open(root, part) => {
                let first = self.trips.iter().find(|t| t.root == root).and_then(|t| t.parts.iter().find(|p| p.name == part)).and_then(|p| days_of(&p.clips).first().map(|d| (*d).to_owned()));
                self.part = Some(part);
                if let Some(day) = first {
                    return self.update(Message::OpenDay(root, day));
                }
            }
            PartMsg::ToOmnix(root, part) => {
                let name = part.clone();
                self.making_edits.insert((root.clone(), Some(format!("part {part}"))));
                return Task::perform(blocking(move || omnix::from_sheet::write_part(&root, &part).map_err(|e| format!("{e:#}"))), move |made| Message::Part(PartMsg::Made(name.clone(), made)));
            }
            PartMsg::Rename(root, part) => {
                self.part_naming = Some((root, part.clone(), part));
                return iced::widget::operation::focus(PART_NAME);
            }
            PartMsg::Typed(t) => {
                if let Some((_, _, typed)) = &mut self.part_naming {
                    *typed = t;
                }
            }
            PartMsg::RenameSave => {
                if let Some((root, part, name)) = self.part_naming.take()
                    && name.trim() != part
                {
                    self.part_op(&root, PartOp::Rename { part, name });
                }
            }
            PartMsg::Remove(root, part) => {
                let sheet = dailix_sheet::Store::new(dailix_sheet::Layout::new(&root).sheet()).read().ok().and_then(|d| d.sheet().ok()).unwrap_or_default();
                let keep = self.clips_to_keep(&root);
                let (gone, kept) = removable(&sheet, &part, |k| keep.contains(k));
                self.part_removing = Some(Removal { root, part, gone, kept });
            }
            PartMsg::RemoveCancel => self.part_removing = None,
            PartMsg::RemoveConfirm(videos) => {
                let Some(r) = self.part_removing.take() else { return Task::none() };
                if videos && let Err(e) = crate::delete::trash_clips(&r.root, &r.gone) {
                    self.error = Some(format!("Not every video went to the trash: {e}"));
                }
                self.part_op(&r.root, PartOp::Remove { part: r.part });
                if videos && self.project.as_ref().is_some_and(|p| p.footage.layout.root == r.root) {
                    return self.load(r.root);
                }
            }
            PartMsg::Card(root, part, e) => {
                use omnix::cards::view::Event;
                let key = card_key(&root, &part);
                match e {
                    Event::Open => {
                        self.card_hover = None;
                        return self.on_part(PartMsg::Open(root, part));
                    }
                    Event::Enter => self.card_hover = Some((key, std::time::Instant::now())),
                    Event::Exit => {
                        if self.card_hover.as_ref().is_some_and(|(c, _)| *c == key) {
                            self.card_hover = None;
                        }
                    }
                }
            }
            PartMsg::Made(part, made) => {
                self.making_edits.retain(|(_, d)| d.as_deref() != Some(format!("part {part}").as_str()));
                match made.and_then(|edit| crate::editor::open(&edit).map(|()| edit)) {
                    Ok(edit) => self.notice = Some(format!("«{}» made from the part «{part}», opening in Omnix.", dailix_sheet::project::edit_name(&edit))),
                    Err(e) => self.error = Some(e),
                }
                self.refresh_home();
            }
        }
        Task::none()
    }

    /// The trip's clips that never leave it with a part: those an edit
    /// of it uses, and those whose original is neither in TMP (its
    /// ledger) nor in the archive (`<Sources>/<trip>/<day>/<name>`).
    fn clips_to_keep(&self, root: &std::path::Path) -> std::collections::HashSet<String> {
        let layout = dailix_sheet::Layout::new(root);
        let key = |p: &std::path::Path| p.strip_prefix(layout.footage()).ok().map(|r| r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
        let mut keep: std::collections::HashSet<String> = dailix_sheet::project::edits(root).iter().flat_map(|e| omnix::from_sheet::clips_of_edit(e)).filter_map(|p| key(&p)).collect();
        let trip = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let in_tmp: std::collections::HashSet<String> = self.settings.tmp.as_deref().map(crate::tmp::Ledger::load).map(|l| l.files.values().flatten().filter(|f| f.trip == trip).map(|f| format!("{}/{}", f.day, f.name)).collect()).unwrap_or_default();
        let sources = self.settings.place(crate::settings::PlaceKind::Sources);
        let footage = crate::library::clip_keys(root);
        keep.extend(footage.into_iter().filter(|k| !in_tmp.contains(k) && !sources.as_ref().is_some_and(|s| s.join(&trip).join(k).is_file())));
        keep
    }

    /// `op` applied to the parts of the trip at `root` (open in the sheet
    /// or not); the trips read again.
    fn part_op(&mut self, root: &std::path::Path, op: PartOp) {
        let store = dailix_sheet::Store::new(dailix_sheet::Layout::new(root).sheet());
        match store.apply(&[Op::Part(op)]).map_err(|e| e.to_string()).and_then(|(doc, _)| doc.sheet().map_err(|e| e.to_string())) {
            Ok(sheet) => {
                if let Some(p) = self.project.as_mut().filter(|p| p.footage.layout.root == root) {
                    p.sheet = sheet;
                }
            }
            Err(e) => self.error = Some(e),
        }
        self.refresh_home();
    }

    /// The trip page's parts: a card each, its clips' frames as a mosaic
    /// (as a trip's card; a click opens it in the sheet), its name and
    /// facts, and under it «→ Omnix», rename and ✕.
    pub(super) fn trip_parts<'a>(&'a self, trip: &'a Trip) -> Element<'a, Message> {
        let what = text("This trip's videos from TMP, in groups of your own across days («Rides on Kos»): the TMP sorter fills them as it files, the sheet's pane ticks a clip in.").size(12).style(text::secondary);
        let col = column![self.fold_head("Parts", trip.parts.len()), what].spacing(6);
        if self.trip_folded.contains("Parts") || trip.parts.is_empty() {
            return col.into();
        }
        let cards = trip.parts.iter().zip(&trip.part_frames).map(|(p, frames)| self.part_card(trip, p, frames));
        col.push(iced::widget::row(cards).spacing(14).wrap().vertical_spacing(14)).into()
    }

    fn part_card<'a>(&'a self, trip: &'a Trip, p: &'a dailix_sheet::Part, frames: &'a omnix::cards::view::Frames) -> Element<'a, Message> {
        let days = days_of(&p.clips);
        let when = match (days.first(), days.last()) {
            (Some(a), Some(b)) if a != b => format!("{a} – {b}"),
            (Some(a), _) => (*a).to_owned(),
            _ => "empty".to_owned(),
        };
        let info = column![text(p.name.clone()).size(17), text(format!("{} · {when}", crate::library::count(p.clips.len(), "clip"))).size(12).style(text::secondary)].spacing(4);
        let key = card_key(&trip.root, &p.name);
        let hovered = self.card_hover.as_ref().filter(|(c, _)| *c == key).map(|(_, since)| self.settings.card_motion().then(|| since.elapsed()));
        let (root, name) = (trip.root.clone(), p.name.clone());
        let card = omnix::cards::view::card(frames, hovered, super::home::TRIP_CARD, true, info.into(), 48.0, move |e| Message::Part(PartMsg::Card(root.clone(), name.clone(), e)));
        let below: Element<'a, Message> = match &self.part_naming {
            Some((root, part, typed)) if *root == trip.root && *part == p.name => text_input("the part's name", typed).id(PART_NAME).size(14).on_input(|t| Message::Part(PartMsg::Typed(t))).on_submit(Message::Part(PartMsg::RenameSave)).into(),
            _ => {
                let go = tip(ui::small("→ Omnix", ui::Kind::Secondary).on_press_maybe((!p.clips.is_empty()).then(|| Message::Part(PartMsg::ToOmnix(trip.root.clone(), p.name.clone())))), text("An edit of this part's clips, from any day (junk left out), opened in Omnix"), tooltip::Position::Bottom);
                let rename = tip(ui::small("Rename", ui::Kind::Quiet).on_press(Message::Part(PartMsg::Rename(trip.root.clone(), p.name.clone()))), text("Rename the part"), tooltip::Position::Bottom);
                let remove = tip(ui::small("✕", ui::Kind::Quiet).on_press(Message::Part(PartMsg::Remove(trip.root.clone(), p.name.clone()))), text("Take the part away: its clips stay in the trip"), tooltip::Position::Bottom);
                row![go, space().width(Length::Fill), rename, remove].spacing(6).align_y(Alignment::Center).into()
            }
        };
        column![card, below].spacing(4).width(super::home::TRIP_CARD + 12.0).into()
    }

    /// The question before a part goes, over the window.
    pub(super) fn part_remove_layer(&self) -> Element<'_, Message> {
        let Some(r) = &self.part_removing else { return space().into() };
        let to = |m: PartMsg| Message::Part(m);
        let mut said = format!("Its videos can stay in the trip, or leave it with the part (to the trash, restorable): {} would.", crate::library::count(r.gone.len(), "video"));
        if r.kept > 0 {
            said += &format!(" {} stay either way: in another part, rated, noted or marked, used by an edit, or with no copy in TMP or the archive.", crate::library::count(r.kept, "video"));
        }
        said += " A video that leaves the trip is new in TMP again, if TMP still has it.";
        let buttons = row![
            space().width(Length::Fill),
            ui::act("Cancel", ui::Kind::Secondary).on_press(to(PartMsg::RemoveCancel)),
            ui::act("The part only", ui::Kind::Secondary).on_press(to(PartMsg::RemoveConfirm(false))),
            ui::act(format!("With its {}", crate::library::count(r.gone.len(), "video")), ui::Kind::Danger).on_press_maybe((!r.gone.is_empty()).then(|| to(PartMsg::RemoveConfirm(true)))),
        ]
        .spacing(8);
        let card = column![text(format!("Remove the part «{}»?", r.part)).size(20), text(said).size(14).style(text::secondary), buttons].spacing(ui::DIALOG_GAP);
        super::modal(ui::dialog(card, ui::DIALOG_WIDTH), to(PartMsg::RemoveCancel))
    }

    /// The contact sheet's «Part» filter, when the trip has parts.
    pub(super) fn part_picker<'a>(&'a self, parts: &'a [dailix_sheet::Part]) -> Option<Element<'a, Message>> {
        if parts.is_empty() {
            return None;
        }
        let choices: Vec<PartChoice> = std::iter::once(PartChoice(None)).chain(parts.iter().map(|p| PartChoice(Some(p.name.clone())))).collect();
        let list = pick_list(choices, Some(PartChoice(self.part.clone())), |c: PartChoice| Message::Part(PartMsg::Filter(c.0))).text_size(13);
        Some(row![text("Part").size(13).style(text::secondary), list].spacing(8).align_y(Alignment::Center).into())
    }

    /// The picked clip's parts to tick.
    pub(super) fn clip_parts<'a>(&'a self, parts: &'a [dailix_sheet::Part], key: &'a str) -> Element<'a, Message> {
        if parts.is_empty() {
            return text("The trip has no parts: the TMP sorter makes them.").size(12).style(text::secondary).into();
        }
        let boxes = parts.iter().map(|p| {
            let (name, clip) = (p.name.clone(), key.to_owned());
            checkbox(p.clips.iter().any(|c| c == key)).label(p.name.clone()).on_toggle(move |on| Message::Part(PartMsg::Clip(clip.clone(), name.clone(), on))).size(15).text_size(13).into()
        });
        iced::widget::Row::with_children(boxes).spacing(14).wrap().vertical_spacing(6).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_takes_with_it_only_the_videos_nothing_else_holds() {
        let sheet: dailix_sheet::Sheet = serde_json::from_str(r#"{
            "clips": {"2026-08-21/rated.MP4": {"rating": 2}, "2026-08-21/marked.MP4": {"marks": [{"t": 1, "end": 3}]}, "2026-08-21/seen.MP4": {"rating": null, "note": "", "marks": []}},
            "parts": [{"name": "Walking", "clips": ["2026-08-21/a.MP4", "2026-08-21/rated.MP4", "2026-08-21/marked.MP4", "2026-08-21/both.MP4", "2026-08-21/seen.MP4"]},
                      {"name": "Beach", "clips": ["2026-08-21/both.MP4"]}]}"#).unwrap();
        assert_eq!(removable(&sheet, "walking", |_| false), (vec!["2026-08-21/a.MP4".to_owned(), "2026-08-21/seen.MP4".to_owned()], 3));
        // One an edit uses, or whose only copy is the trip's, stays.
        assert_eq!(removable(&sheet, "walking", |k| k.ends_with("seen.MP4")), (vec!["2026-08-21/a.MP4".to_owned()], 4));
        assert_eq!(removable(&sheet, "Sea", |_| false), (Vec::new(), 0));
    }

    #[test]
    fn a_parts_days_are_its_clips_days_once_in_order() {
        let clips = ["2026-08-23/b.MP4", "2026-08-21/a.MP4", "2026-08-23/c.MP4"].map(String::from);
        assert_eq!(days_of(&clips), ["2026-08-21", "2026-08-23"]);
    }

    #[test]
    fn the_part_filter_shows_its_clips_or_every_clip() {
        let parts = [dailix_sheet::Part { name: "Rides".into(), clips: vec!["2026-08-21/a.MP4".into()] }];
        assert!(part_shows(&parts, None, "2026-08-22/x.MP4"));
        assert!(part_shows(&parts, Some("rides"), "2026-08-21/a.MP4") && !part_shows(&parts, Some("Rides"), "2026-08-22/x.MP4"));
        assert!(part_shows(&parts, Some("Gone"), "2026-08-22/x.MP4"), "a part since removed hides nothing");
    }
}
