//! The top bar, the same on every page (the UI review, 2026-11-14, step
//! 3): back and where you are at the start (Trips › a trip › its contact
//! sheet, each a way back), what's under way, the main menu ☰ at the end.
//! The app-wide things (text size, open, keys, settings, about) moved from
//! the bar into ☰; ⌂ Home gave way to the path.

use super::{App, Message, menu, modal, tip};
use crate::ui;
use iced::widget::{column, container, row, space, text, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::path::PathBuf;

/// Where the window is.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    /// The trips' cards.
    Trips,
    /// A trip's page.
    Trip(PathBuf),
    /// A trip's contact sheet, at a day when a clip of it is picked.
    Sheet(PathBuf, Option<String>),
    /// The TMP sorter (gap 117).
    Tmp,
}

/// One step back from `place`: the sheet to its trip, a trip to the
/// trips; none from the trips.
pub fn back_from(place: &Place) -> Option<Place> {
    match place {
        Place::Trips => None,
        Place::Trip(_) | Place::Tmp => Some(Place::Trips),
        Place::Sheet(root, _) => Some(Place::Trip(root.clone())),
    }
}

/// The jobs under way, said in a line (the UI review, step 9): adding
/// videos, making edits, voices, posters; empty with none.
pub fn busy_line(posters: usize, adding: usize, edits: usize, voices: usize, job: Option<String>) -> String {
    let mut said = Vec::new();
    said.extend(job);
    if adding > 0 {
        said.push(if adding == 1 { "adding videos".to_owned() } else { format!("adding videos to {adding} trips") });
    }
    if edits > 0 {
        said.push(if edits == 1 { "making an edit".to_owned() } else { format!("making {edits} edits") });
    }
    if voices > 0 {
        said.push(format!("cleaning {}", crate::library::count(voices, "voice")));
    }
    if posters > 0 {
        said.push(format!("posters: {posters} left"));
    }
    said.join(" · ")
}

/// The window's title: the trip's name first, where there is one.
pub fn title(place: &Place) -> String {
    match place {
        Place::Trips => "Dailix".to_owned(),
        Place::Tmp => "TMP — Dailix".to_owned(),
        Place::Trip(root) | Place::Sheet(root, _) => format!("{} — Dailix", dailix_sheet::project::title(root)),
    }
}

impl App {
    pub(super) fn place(&self) -> Place {
        match (&self.project, self.at_home, &self.trip_open) {
            (Some(p), false, _) => {
                let day = self.selected.as_ref().and_then(|k| p.footage.days.iter().find(|d| d.clips.iter().any(|c| &c.key == k))).map(|d| d.day.clone());
                Place::Sheet(p.footage.layout.root.clone(), day)
            }
            (_, true, _) if self.sorter.is_some() => Place::Tmp,
            (_, true, Some(root)) => Place::Trip(root.clone()),
            _ => Place::Trips,
        }
    }

    /// One step back (‹, Alt+←).
    pub(super) fn back(&mut self) -> Task<Message> {
        match back_from(&self.place()) {
            Some(Place::Trip(root)) => {
                let home = self.update(Message::Home);
                Task::batch([home, self.update(Message::OpenTrip(root))])
            }
            Some(_) => self.update(Message::Home),
            None => Task::none(),
        }
    }

    pub(super) fn top_bar(&self) -> Element<'_, Message> {
        let place = self.place();
        let link = |label: String, msg: Message| ui::act(label, ui::Kind::Quiet).on_press(msg);
        // A link's own padding sits left of «›»: as much on its right.
        let sep = || container(text("›").size(16).style(text::secondary)).padding([0, 8]);
        let mut path = row![].align_y(Alignment::Center);
        if back_from(&place).is_some() {
            path = path.push(tip(ui::act("‹", ui::Kind::Secondary).on_press(Message::Back), text("Back (Alt+←)"), tooltip::Position::Bottom)).push(space().width(6));
        }
        let here = |s: String| container(text(s).size(18)).padding([0, 4]);
        path = match &place {
            Place::Trips => path.push(here("Dailix".to_owned())),
            Place::Tmp => path.push(link("Trips".into(), Message::Home)).push(sep()).push(here("TMP: sort into trips".to_owned())),
            Place::Trip(root) => path.push(link("Trips".into(), Message::Home)).push(sep()).push(here(dailix_sheet::project::title(root))),
            Place::Sheet(root, day) => path
                .push(link("Trips".into(), Message::Home))
                .push(sep())
                .push(link(dailix_sheet::project::title(root), Message::Back))
                .push(sep())
                .push(here(day.as_ref().map_or_else(|| "Contact sheet".to_owned(), |d| format!("Contact sheet · {d}")))),
        };
        let mut bar = row![path, space().width(Length::Fill)].spacing(12).align_y(Alignment::Center);
        // The TMP sorter's job: filing, archiving or freeing, how far.
        let job = self.sorter.as_ref().and_then(|s| {
            let filing = s.filing.as_ref().map(|f| format!("filing {} of {}", f.done + 1, f.total));
            filing.or_else(|| s.moving.as_ref().map(|m| format!("{} {} of {}", if m.free { "freeing" } else { "archiving" }, m.done + 1, m.total)))
        });
        let busy = busy_line(self.making.len(), self.adding.len(), self.making_edits.len(), self.voice_makes.len(), job);
        if !busy.is_empty() {
            bar = bar.push(text(format!("⟳ {busy}")).size(13).style(text::secondary));
        }
        let main = tip(menu::opener("☰", menu::Menu::Main, ui::Kind::Secondary, false), text("Text size, open, keys, settings"), tooltip::Position::Left);
        container(bar.push(main)).padding([10, 16]).into()
    }

    /// How Dailix works: the way footage goes, a step each, and whether
    /// the places it needs are set.
    pub(super) fn how_layer(&self) -> Element<'_, Message> {
        if !self.how_open {
            return space().into();
        }
        let steps = crate::help::WAY.iter().enumerate().map(|(i, (title, what))| row![text(format!("{}.", i + 1)).size(14).width(24), column![text(*title).size(15), text(*what).size(13).style(text::secondary)].spacing(2)].spacing(6).into());
        let place = |kind: crate::settings::PlaceKind| {
            let p = self.places.iter().find(|p| p.kind == kind);
            let state = match p {
                Some(p) if p.online => "set".to_owned(),
                Some(p) if p.path.is_some() => "not there (its disk?)".to_owned(),
                _ => "not set".to_owned(),
            };
            text(format!("{}: {state}", kind.label())).size(13)
        };
        use crate::settings::PlaceKind;
        let places = row![place(PlaceKind::Tmp), place(PlaceKind::Projects), place(PlaceKind::Sources), space().width(Length::Fill), ui::act("Folders…", ui::Kind::Secondary).on_press(Message::Settings)].spacing(16).align_y(Alignment::Center);
        let card = column![
            text("How Dailix works").size(22),
            column(steps).spacing(10),
            iced::widget::rule::horizontal(1),
            places,
            row![space().width(Length::Fill), ui::act("Close", ui::Kind::Secondary).on_press(Message::HowItWorks)],
        ]
        .spacing(ui::DIALOG_GAP);
        modal(ui::dialog(card, 640.0), Message::HowItWorks)
    }

    /// About Dailix: its name, version and what it's for.
    pub(super) fn about_layer(&self) -> Element<'_, Message> {
        if !self.about_open {
            return space().into();
        }
        let card = column![
            text("Dailix").size(24),
            text(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(13).style(text::secondary),
            text("Looking through a trip's footage and choosing it, before the edit in Omnix.").size(14),
            text("Built with Claude (docs/AI-DRIVEN.md in Omnix's repository).").size(13).style(text::secondary),
            row![space().width(Length::Fill), ui::act("Close", ui::Kind::Secondary).on_press(Message::About)],
        ]
        .spacing(ui::DIALOG_GAP);
        modal(ui::dialog(card, 440.0), Message::About)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_goes_up_one_step() {
        let t = PathBuf::from("trip");
        assert_eq!(back_from(&Place::Sheet(t.clone(), Some("2026-08-22".into()))), Some(Place::Trip(t.clone())));
        assert_eq!(back_from(&Place::Trip(t)), Some(Place::Trips));
        assert_eq!(back_from(&Place::Trips), None);
    }

    #[test]
    fn the_jobs_under_way_are_said_in_a_line() {
        assert_eq!(busy_line(0, 0, 0, 0, None), "");
        assert_eq!(busy_line(12, 1, 1, 0, None), "adding videos · making an edit · posters: 12 left");
        assert_eq!(busy_line(0, 2, 0, 1, Some("filing 3 of 7".into())), "filing 3 of 7 · adding videos to 2 trips · cleaning 1 voice");
    }

    #[test]
    fn the_window_is_named_by_its_trip() {
        assert_eq!(title(&Place::Trips), "Dailix");
        assert!(title(&Place::Trip(PathBuf::from("/nowhere/2026-08_sea"))).ends_with(" — Dailix"));
    }
}
