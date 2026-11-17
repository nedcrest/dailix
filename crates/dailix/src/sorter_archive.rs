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
    /// Both: each video into the archive, then out of TMP.
    pub both: bool,
    /// The videos themselves, not places in the sorter's list: the list
    /// is read again while a file goes (the audit, 2026-11-17: an index
    /// into a newer, shorter list panicked).
    pub queue: VecDeque<crate::tmp::Raw>,
    pub total: usize,
    pub done: usize,
    pub failed: Vec<String>,
}

impl Moving {
    /// The line under the bar while it goes.
    pub fn line(&self, now: &str) -> String {
        let verb = match (self.both, self.free) {
            (true, _) => "Into the archive and out of TMP",
            (_, true) => "Freeing TMP",
            _ => "Copying into the archive",
        };
        format!("{verb}: {} of {}: {now}", self.done + 1, self.total)
    }
}

impl Sorter {
    pub fn stages(&self, list: &[usize]) -> Vec<Stage> {
        list.iter().map(|i| archive::stage(&self.ledger, &self.raws[*i].rel)).collect()
    }

    /// Filing, archiving or freeing under way: one at a time, and the
    /// sorter isn't left meanwhile (the audit: leaving dropped the rest).
    pub fn busy(&self) -> bool {
        self.filing.is_some() || self.moving.is_some()
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
            SortMsg::Archive(list) => return self.start_moving(list, false, false),
            SortMsg::AllAsk => {
                if let Some(s) = &mut self.sorter {
                    s.asking_all = true;
                }
            }
            SortMsg::AllCancel => {
                if let Some(s) = &mut self.sorter {
                    s.asking_all = false;
                }
            }
            SortMsg::AllConfirm => {
                let Some(s) = &mut self.sorter else { return Task::none() };
                s.asking_all = false;
                let all = (0..s.raws.len()).collect();
                return self.start_moving(all, false, true);
            }
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
                return self.start_moving(list, true, false);
            }
            SortMsg::MovedOne(i, done) => return self.moved_one(i, done),
            _ => {}
        }
        Task::none()
    }

    /// Archiving (or freeing) `list` started: those at the right stage
    /// only, a file at a time.
    fn start_moving(&mut self, list: Vec<usize>, free: bool, both: bool) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        if s.busy() {
            return Task::none();
        }
        // Freeing: what's archived; archiving: everything not yet, in a
        // trip or not.
        let queue: VecDeque<crate::tmp::Raw> = list.into_iter().filter_map(|i| s.raws.get(i)).filter(|r| both || (archive::stage(&s.ledger, &r.rel) == Stage::Archived) == free).cloned().collect();
        if queue.is_empty() {
            return Task::none();
        }
        s.moving = Some(Moving { free, both, total: queue.len(), queue, done: 0, failed: Vec::new() });
        self.move_next()
    }

    fn move_next(&mut self) -> Task<Message> {
        let (tmp, sources) = (self.settings.tmp.clone(), self.sources_place());
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(m) = &mut s.moving else { return Task::none() };
        let Some(raw) = m.queue.pop_front() else { return self.moved_all() };
        let rel = raw.rel.clone();
        self.notice = Some(m.line(&raw.rel));
        let failed = |e: &str| archive::Moved { error: Some(e.to_owned()), ..Default::default() };
        let poster = crate::tmp::poster(&tmp.unwrap_or_default(), &raw.rel);
        let already = s.ledger.archived.get(&raw.rel).cloned();
        if m.free {
            let Some(archived) = already else { return Task::done(Message::Sort(SortMsg::MovedOne(rel, failed("not in the archive")))) };
            return Task::perform(blocking(move || match archive::free_one(&raw.path, &poster, &archived) {
                Ok(()) => archive::Moved { freed: true, ..Default::default() },
                Err(e) => archive::Moved { error: Some(e), ..Default::default() },
            }), move |m| Message::Sort(SortMsg::MovedOne(rel.clone(), m)));
        }
        let Some(sources) = sources else { return Task::done(Message::Sort(SortMsg::MovedOne(rel, failed("the archive (Sources) isn't set, or isn't there")))) };
        let filed = s.ledger.filed(&raw.rel).first().cloned();
        if m.both {
            return Task::perform(blocking(move || archive::archive_and_free(&raw.path, filed.as_ref(), raw.day.as_deref(), &sources, &poster, already)), move |m| Message::Sort(SortMsg::MovedOne(rel.clone(), m)));
        }
        Task::perform(blocking(move || match archive::archive_one(&raw.path, filed.as_ref(), raw.day.as_deref(), &sources) {
            Ok(a) => archive::Moved { archived: Some(a), ..Default::default() },
            Err(e) => archive::Moved { error: Some(e), ..Default::default() },
        }), move |m| Message::Sort(SortMsg::MovedOne(rel.clone(), m)))
    }

    /// One done: an archived copy written into the ledger (before the next,
    /// so a stop midway loses nothing), the next started.
    fn moved_one(&mut self, rel: String, done: archive::Moved) -> Task<Message> {
        let tmp = self.settings.tmp.clone().unwrap_or_default();
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(m) = &mut s.moving else { return Task::none() };
        m.done += 1;
        // Archived: said in the ledger before the next; freed: forgotten,
        // so its place can hold another video.
        let (archived, freed) = (done.archived.clone(), done.freed);
        let write = crate::tmp::Ledger::update(&tmp, |l| {
            if let Some(a) = &archived {
                l.archived.insert(rel.clone(), a.clone());
            }
            if freed {
                l.forget(&rel);
            }
        });
        match write {
            Ok(()) => {
                if let Some(a) = archived {
                    s.ledger.archived.insert(rel.clone(), a);
                }
                if freed {
                    s.ledger.forget(&rel);
                }
            }
            Err(e) => m.failed.push(format!("{rel}: the ledger: {e}")),
        }
        if let Some(e) = done.error {
            m.failed.push(e);
        }
        self.move_next()
    }

    /// All done: said in a line; TMP read again when files left it.
    fn moved_all(&mut self) -> Task<Message> {
        let Some(m) = self.sorter.as_mut().and_then(|s| s.moving.take()) else { return Task::none() };
        let ok = m.done - m.failed.len();
        let did = match (m.both, m.free) {
            (true, _) => format!("{} into the archive, checked, and out of TMP", crate::library::count(ok, "video")),
            (_, true) => format!("TMP freed of {}", crate::library::count(ok, "video")),
            _ => format!("{} copied into the archive and checked", crate::library::count(ok, "video")),
        };
        self.notice = Some(did);
        if let Some(first) = m.failed.first() {
            self.error = Some(format!("{} didn't: {first}", crate::library::count(m.failed.len(), "video")));
        }
        self.refresh_home();
        if m.free || m.both {
            if let Some(t) = &self.settings.tmp {
                crate::tmp::drop_empty_folders(t);
            }
            return self.update(Message::Sort(SortMsg::Open));
        }
        Task::none()
    }

    /// A day's head: where its videos are, and its next step as a button.
    pub(super) fn day_head<'a>(&'a self, s: &'a Sorter, day: Option<&str>, list: &[usize]) -> Element<'a, Message> {
        let stages = s.stages(list);
        let n = |st: Stage| stages.iter().filter(|x| **x == st).count();
        let in_parts = list.iter().filter(|i| self.in_a_part(s, &s.raws[**i].rel)).count();
        let facts = format!("{} · {} · {} in a trip's days · {in_parts} in a part · {} in the archive", day.unwrap_or("No date"), crate::library::count(list.len(), "video"), n(Stage::Filed) + n(Stage::Archived), n(Stage::Archived));
        let busy = s.busy();
        // Both steps, each when it has something: a day half sorted can
        // still go into the archive (its unsorted under _not-in-a-trip).
        let (to_archive, to_free) = archive::counts(&stages);
        let archive = (to_archive > 0).then(|| match self.sources_place() {
            Some(_) => tip(ui::small(format!("Into the archive ({to_archive})"), ui::Kind::Primary).on_press_maybe((!busy).then(|| Message::Sort(SortMsg::Archive(list.to_vec())))), text("Copied into the Sources folder, each checked by its hash there: by trip and day, or under _not-in-a-trip/<day> if in no trip yet; TMP keeps them till you free it"), tooltip::Position::Bottom),
            None => tip(ui::small("Set the archive…", ui::Kind::Secondary).on_press(Message::ChoosePlace(PlaceKind::Sources)), text("The archive (Sources) isn't set, or its disk isn't there: choose its folder"), tooltip::Position::Bottom),
        });
        let free = (to_free > 0).then(|| tip(ui::small(format!("Free TMP ({to_free})"), ui::Kind::Secondary).on_press_maybe((!busy).then(|| Message::Sort(SortMsg::FreeAsk(list.to_vec())))), text("To the trash: the archive holds them, checked again before each goes"), tooltip::Position::Bottom));
        row![text(facts).size(14).style(text::secondary)].push(archive).push(free).spacing(12).align_y(iced::Alignment::Center).into()
    }

    /// The question before every video goes into the archive and out of
    /// TMP: how many, how big, where the archive is.
    pub(super) fn all_layer(&self) -> Element<'_, Message> {
        let Some(s) = self.sorter.as_ref().filter(|s| s.asking_all) else { return space().into() };
        let bytes: u64 = s.raws.iter().filter_map(|r| std::fs::metadata(&r.path).ok()).map(|m| m.len()).sum();
        let archive = self.sources_place().map_or_else(|| "(not set)".to_owned(), |p| p.display().to_string());
        let said = format!(
            "{} ({:.1} GB) are copied into the archive, {archive}: by trip and day, those in no trip under {}/<day>. Each copy is checked by its hash; only then the video leaves TMP, to the trash (restorable from there). One that fails stays in TMP.",
            crate::library::count(s.raws.len(), "video"),
            bytes as f64 / 1e9,
            archive::NOT_IN_A_TRIP
        );
        let ready = self.sources_place().is_some();
        let buttons = row![
            space().width(Length::Fill),
            ui::act("Cancel", ui::Kind::Secondary).on_press(Message::Sort(SortMsg::AllCancel)),
            ui::act("Archive all and free TMP", ui::Kind::Danger).on_press_maybe(ready.then_some(Message::Sort(SortMsg::AllConfirm))),
        ]
        .spacing(8);
        let card = column![text("Everything into the archive, TMP emptied?").size(20), text(said).size(14).style(text::secondary), buttons].spacing(ui::DIALOG_GAP);
        modal(ui::dialog(card, ui::DIALOG_WIDTH), Message::Sort(SortMsg::AllCancel))
    }

    /// The bar under the top bar while the sorter files, archives or
    /// frees: the line of what's going on, a progress bar, «n / total».
    pub(super) fn job_bar(&self) -> Option<Element<'_, Message>> {
        let s = self.sorter.as_ref()?;
        let (done, total) = s.filing.as_ref().map(|f| (f.done, f.total)).or_else(|| s.moving.as_ref().map(|m| (m.done, m.total)))?;
        let said = self.notice.clone().unwrap_or_default();
        let bar = row![
            text(format!("⟳ {said}")).size(15).width(Length::Fill),
            iced::widget::progress_bar(0.0..=total.max(1) as f32, done as f32).girth(8).length(260).style(|t: &iced::Theme| iced::widget::progress_bar::Style {
                background: iced::Color { a: 0.35, ..iced::Color::BLACK }.into(),
                bar: t.extended_palette().primary.strong.text.into(),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
            }),
            text(format!("{done} / {total}")).size(15),
        ]
        .spacing(14)
        .align_y(iced::Alignment::Center);
        Some(
            iced::widget::container(bar)
                .padding([8, 16])
                .width(Length::Fill)
                .style(|t: &iced::Theme| iced::widget::container::Style {
                    background: Some(t.extended_palette().primary.strong.color.into()),
                    text_color: Some(t.extended_palette().primary.strong.text),
                    ..Default::default()
                })
                .into(),
        )
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
        let m = Moving { free: true, both: false, queue: VecDeque::new(), total: 4, done: 1, failed: Vec::new() };
        assert_eq!(m.line("card/a.MP4"), "Freeing TMP: 2 of 4: card/a.MP4");
    }
}
