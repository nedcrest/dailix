//! The TMP sorter's way after filing (Omnix gap 119): each day says where
//! its videos are (filed, in the archive) and offers its next step — «Into
//! the archive» (copied to the sources place, checked by hash), then «Free
//! TMP» (to the trash, asked once, only what the archive holds checked).
//! The picked get a «Next:» line. A file at a time, the line under the bar
//! saying how far. The rules are `archive.rs`'s, tested.

use super::sorter::{SortMsg, Sorter};
use super::{App, Message, blocking, modal, tip};
use crate::archive::{self, Next, Stage};
use crate::settings::PlaceKind;
use crate::ui;
use iced::widget::{column, row, space, text, tooltip};
use iced::{Element, Length, Task};
use std::collections::VecDeque;
use std::path::PathBuf;

/// Videos being archived or freed: which (indices into the sorter's),
/// how many there were and are done, what went wrong.
#[derive(Debug, Clone)]
pub struct Moving {
    pub free: bool,
    pub queue: VecDeque<usize>,
    pub total: usize,
    pub done: usize,
    pub failed: Vec<String>,
}

impl Moving {
    /// The line under the bar while it goes.
    pub fn line(&self, now: &str) -> String {
        let verb = if self.free { "Freeing TMP" } else { "Copying into the archive" };
        format!("{verb}: {} of {}: {now}", self.done + 1, self.total)
    }
}

impl Sorter {
    pub fn stages(&self, list: &[usize]) -> Vec<Stage> {
        list.iter().map(|i| archive::stage(&self.ledger, &self.raws[*i].rel)).collect()
    }
}

/// The «Next:» line for the picked.
pub fn next_line(next: Next) -> String {
    match next {
        Next::File(n) => format!("Next: file {} into a trip (below).", crate::library::count(n, "video")),
        Next::Archive(n) => format!("Next: {} into the archive, then TMP can let them go.", crate::library::count(n, "video")),
        Next::Free(n) => format!("Next: free TMP of {}: the archive holds them, checked.", crate::library::count(n, "video")),
        Next::Done => "Nothing left to do with these.".to_owned(),
    }
}

impl App {
    fn sources_place(&self) -> Option<PathBuf> {
        self.settings.place(PlaceKind::Sources).filter(|p| p.is_dir())
    }

    pub(super) fn on_archive(&mut self, msg: SortMsg) -> Task<Message> {
        match msg {
            SortMsg::Archive(list) => return self.start_moving(list, false),
            SortMsg::FreeAsk(list) => {
                if let Some(s) = &mut self.sorter {
                    s.asking_free = Some(list);
                }
            }
            SortMsg::FreeCancel => {
                if let Some(s) = &mut self.sorter {
                    s.asking_free = None;
                }
            }
            SortMsg::FreeConfirm => {
                let list = self.sorter.as_mut().and_then(|s| s.asking_free.take()).unwrap_or_default();
                return self.start_moving(list, true);
            }
            SortMsg::MovedOne(i, done) => return self.moved_one(i, done),
            _ => {}
        }
        Task::none()
    }

    /// Archiving (or freeing) `list` started: those at the right stage
    /// only, a file at a time.
    fn start_moving(&mut self, list: Vec<usize>, free: bool) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        if s.moving.is_some() || s.filing.is_some() {
            return Task::none();
        }
        let want = if free { Stage::Archived } else { Stage::Filed };
        let queue: VecDeque<usize> = list.into_iter().filter(|i| archive::stage(&s.ledger, &s.raws[*i].rel) == want).collect();
        if queue.is_empty() {
            return Task::none();
        }
        s.moving = Some(Moving { free, total: queue.len(), queue, done: 0, failed: Vec::new() });
        self.move_next()
    }

    fn move_next(&mut self) -> Task<Message> {
        let (tmp, sources) = (self.settings.tmp.clone(), self.sources_place());
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(m) = &mut s.moving else { return Task::none() };
        let Some(i) = m.queue.pop_front() else { return self.moved_all() };
        let raw = s.raws[i].clone();
        self.notice = Some(m.line(&raw.rel));
        if m.free {
            let Some(archived) = s.ledger.archived.get(&raw.rel).cloned() else { return Task::done(Message::Sort(SortMsg::MovedOne(i, Err("not in the archive".into())))) };
            let poster = crate::tmp::poster(&tmp.unwrap_or_default(), &raw.rel);
            return Task::perform(blocking(move || archive::free_one(&raw.path, &poster, &archived).map(|()| None)), move |r| Message::Sort(SortMsg::MovedOne(i, r)));
        }
        let (Some(sources), Some(filed)) = (sources, s.ledger.filed(&raw.rel).first().cloned()) else {
            return Task::done(Message::Sort(SortMsg::MovedOne(i, Err("the archive (Sources) isn't set, or isn't there".into()))));
        };
        Task::perform(blocking(move || archive::archive_one(&raw.path, &filed, &sources).map(Some)), move |r| Message::Sort(SortMsg::MovedOne(i, r)))
    }

    /// One done: an archived copy written into the ledger (before the next,
    /// so a stop midway loses nothing), the next started.
    fn moved_one(&mut self, i: usize, done: Result<Option<archive::Archived>, String>) -> Task<Message> {
        let tmp = self.settings.tmp.clone();
        let Some(s) = &mut self.sorter else { return Task::none() };
        let rel = s.raws[i].rel.clone();
        let Some(m) = &mut s.moving else { return Task::none() };
        m.done += 1;
        match done {
            Ok(Some(a)) => {
                if let Some(t) = &tmp {
                    let mut ledger = crate::tmp::Ledger::load(t);
                    ledger.archived.insert(rel.clone(), a.clone());
                    if let Err(e) = ledger.save(t) {
                        m.failed.push(format!("{rel}: the ledger: {e}"));
                    }
                }
                s.ledger.archived.insert(rel, a);
            }
            Ok(None) => {}
            Err(e) => m.failed.push(e),
        }
        self.move_next()
    }

    /// All done: said in a line; TMP read again when files left it.
    fn moved_all(&mut self) -> Task<Message> {
        let Some(m) = self.sorter.as_mut().and_then(|s| s.moving.take()) else { return Task::none() };
        let ok = m.done - m.failed.len();
        let did = if m.free { format!("TMP freed of {}", crate::library::count(ok, "video")) } else { format!("{} copied into the archive and checked", crate::library::count(ok, "video")) };
        self.notice = Some(did);
        if let Some(first) = m.failed.first() {
            self.error = Some(format!("{} didn't: {first}", crate::library::count(m.failed.len(), "video")));
        }
        self.refresh_home();
        if m.free { self.update(Message::Sort(SortMsg::Open)) } else { Task::none() }
    }

    /// A day's head: where its videos are, and its next step as a button.
    pub(super) fn day_head<'a>(&'a self, s: &'a Sorter, day: Option<&str>, list: &[usize]) -> Element<'a, Message> {
        let stages = s.stages(list);
        let n = |st: Stage| stages.iter().filter(|x| **x == st).count();
        let facts = format!("{} · {} · {} filed · {} in the archive", day.unwrap_or("No date"), crate::library::count(list.len(), "video"), n(Stage::Filed) + n(Stage::Archived), n(Stage::Archived));
        let busy = s.moving.is_some() || s.filing.is_some();
        let step: Option<Element<'a, Message>> = match archive::next(&stages) {
            Next::Archive(k) => Some(match self.sources_place() {
                Some(_) => tip(ui::small(format!("Into the archive ({k})"), ui::Kind::Primary).on_press_maybe((!busy).then(|| Message::Sort(SortMsg::Archive(list.to_vec())))), text("Copied into the Sources folder by trip and day, each checked by its hash there; TMP keeps them till you free it"), tooltip::Position::Bottom),
                None => tip(ui::small("Set the archive…", ui::Kind::Secondary).on_press(Message::ChoosePlace(PlaceKind::Sources)), text("The archive (Sources) isn't set, or its disk isn't there: choose its folder"), tooltip::Position::Bottom),
            }),
            Next::Free(k) => Some(tip(ui::small(format!("Free TMP ({k})"), ui::Kind::Secondary).on_press_maybe((!busy).then(|| Message::Sort(SortMsg::FreeAsk(list.to_vec())))), text("To the trash: the archive holds them, checked again before each goes"), tooltip::Position::Bottom)),
            _ => None,
        };
        row![text(facts).size(14).style(text::secondary)].push(step).spacing(12).align_y(iced::Alignment::Center).into()
    }

    /// The question before TMP lets videos go.
    pub(super) fn free_layer(&self) -> Element<'_, Message> {
        let Some(list) = self.sorter.as_ref().and_then(|s| s.asking_free.as_ref()) else { return space().into() };
        let s = self.sorter.as_ref().expect("asked in the sorter");
        let bytes: u64 = list.iter().filter_map(|i| std::fs::metadata(&s.raws[*i].path).ok()).map(|m| m.len()).sum();
        let said = format!("{} ({:.1} GB) go to the trash, restorable from there. Each one's archive copy is read again first; one that changed stays in TMP.", crate::library::count(list.len(), "video"), bytes as f64 / 1e9);
        let buttons = row![
            space().width(Length::Fill),
            ui::act("Cancel", ui::Kind::Secondary).on_press(Message::Sort(SortMsg::FreeCancel)),
            ui::act("Free TMP", ui::Kind::Danger).on_press(Message::Sort(SortMsg::FreeConfirm)),
        ]
        .spacing(8);
        let card = column![text("Free TMP of these videos?").size(20), text(said).size(14).style(text::secondary), buttons].spacing(ui::DIALOG_GAP);
        modal(ui::dialog(card, ui::DIALOG_WIDTH), Message::Sort(SortMsg::FreeCancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_step_is_said_in_a_line() {
        assert_eq!(next_line(Next::Archive(2)), "Next: 2 videos into the archive, then TMP can let them go.");
        assert_eq!(next_line(Next::Done), "Nothing left to do with these.");
        let m = Moving { free: true, queue: VecDeque::new(), total: 4, done: 1, failed: Vec::new() };
        assert_eq!(m.line("card/a.MP4"), "Freeing TMP: 2 of 4: card/a.MP4");
    }
}
