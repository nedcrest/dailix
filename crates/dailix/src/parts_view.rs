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
use iced::widget::{checkbox, column, pick_list, row, space, text, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::path::PathBuf;

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

    /// The trip page's parts: a row each.
    pub(super) fn trip_parts<'a>(&'a self, trip: &'a Trip) -> Element<'a, Message> {
        let mut col = column![text("Parts").size(15)].spacing(6);
        if trip.parts.is_empty() {
            return col.push(text("None yet: the TMP sorter puts clips into parts as it files them, and the sheet's pane ticks a clip into one.").size(12).style(text::secondary)).into();
        }
        for p in &trip.parts {
            let days = days_of(&p.clips);
            let when = match (days.first(), days.last()) {
                (Some(a), Some(b)) if a != b => format!("{a} – {b}"),
                (Some(a), _) => (*a).to_owned(),
                _ => "empty".to_owned(),
            };
            let facts = text(format!("{} · {}", crate::library::count(p.clips.len(), "clip"), when)).size(12).style(text::secondary);
            let open = tip(ui::small("Sheet", ui::Kind::Secondary).on_press_maybe((!p.clips.is_empty()).then(|| Message::Part(PartMsg::Open(trip.root.clone(), p.name.clone())))), text("The contact sheet showing this part's clips only"), tooltip::Position::Bottom);
            let go = tip(ui::small("→ Omnix", ui::Kind::Secondary).on_press_maybe((!p.clips.is_empty()).then(|| Message::Part(PartMsg::ToOmnix(trip.root.clone(), p.name.clone())))), text("An edit of this part's clips, from any day (junk left out), opened in Omnix"), tooltip::Position::Bottom);
            col = col.push(row![text(p.name.clone()).size(14).width(220), facts, space().width(Length::Fill), open, go].spacing(10).align_y(Alignment::Center).width(Length::Fixed(860.0)));
        }
        col.into()
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
