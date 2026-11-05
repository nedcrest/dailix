//! The home view (UI-RESEARCH 3.1, 3.3, 3.4): the places at the top (each
//! there or not, its free space), then the trips, each a row of day cards:
//! a poster, the clips and how many are rated, ✎ when the day's story is
//! written, and where its files are (in the project, in the sources).
//! Folders stay out of sight: a card opens its day in the contact sheet.

use super::{App, Message, pane};
use crate::library::{self, Place, Trip};
use iced::widget::{button, column, container, image, progress_bar, row, rule, scrollable, space, text, text_input, tooltip};
use iced::{Alignment, Element, Length, Task, Theme};

/// The field a project is named in, focused when it opens.
pub(super) const NAME: &str = "project-name";

/// A day card's width; its poster is 16:9 inside it.
const CARD: f32 = 200.0;

fn gb(bytes: u64) -> String {
    format!("{:.0} GB", bytes as f64 / 1e9)
}

impl App {
    /// The home view's messages, and going to and from it: a project
    /// named, a day opened, an edit made for Omnix.
    pub(super) fn on_home(&mut self, message: Message) -> Task<Message> {
        match message {
            m @ (Message::ToOmnix(..) | Message::EmptyDay(..)) => {
                self.save_note();
                let made = match &m {
                    Message::EmptyDay(root, day) => omnix::from_sheet::write_empty(root, day.as_deref()),
                    Message::ToOmnix(root, day) => omnix::from_sheet::write(root, day.as_deref()),
                    _ => return Task::none(),
                };
                self.error = made.map_err(|e| format!("{e:#}")).and_then(|edit| crate::editor::open(&edit)).err();
            }
            Message::Name(root) => {
                let typed = dailix_sheet::project::title(&root);
                self.naming = Some((root, typed));
                return iced::widget::operation::focus(NAME);
            }
            Message::NameEdit(typed) => {
                if let Some((_, t)) = &mut self.naming {
                    *t = typed;
                }
            }
            Message::NameSave => {
                if let Some((root, typed)) = self.naming.take() {
                    match dailix_sheet::project::set_title(&root, &typed) {
                        Ok(()) => self.refresh_home(),
                        Err(e) => self.error = Some(format!("{}: {e}", root.display())),
                    }
                }
            }
            Message::Home => {
                self.save_note();
                self.pane = pane::Pane::default();
                self.selected = None;
                self.at_home = true;
                self.refresh_home();
            }
            Message::OpenDay(root, day) => {
                self.at_home = false;
                let open = if self.project.as_ref().map(|p| &p.footage.layout.root) != Some(&root) { self.load(root) } else { Task::none() };
                let first = self.project.as_ref().and_then(|p| p.footage.days.iter().find(|d| d.day == day)).and_then(|d| d.clips.first()).map(|c| c.key.clone());
                let pick = first.map_or_else(Task::none, |k| self.select(k));
                return Task::batch([open, pick]);
            }
            _ => {}
        }
        Task::none()
    }

    /// Reads the trips and the places again.
    pub(super) fn refresh_home(&mut self) {
        let (trips, skipped) = self.settings.projects.as_deref().map(library::trips).unwrap_or_default();
        self.trips = trips;
        self.places = library::places(&self.settings);
        if let Some(first) = skipped.first() {
            self.error = Some(format!("{} projects couldn't be read; first: {first}", skipped.len()));
        }
    }

    pub(super) fn home(&self) -> Element<'_, Message> {
        let mut page = column![self.places_strip()].spacing(18).padding([8, 16]);
        if self.trips.is_empty() {
            page = page.push(text(match &self.settings.projects {
                Some(p) => format!("No projects in {}", p.display()),
                None => "No projects place set (dailix places --projects …)".to_owned(),
            }));
        }
        for trip in &self.trips {
            page = page.push(rule::horizontal(1)).push(self.trip_row(trip));
        }
        scrollable(page).height(Length::Fill).into()
    }

    /// The places, a chip each: there (●) or not (○), and the free space.
    fn places_strip(&self) -> Element<'_, Message> {
        let chip = |p: &Place| -> Element<'_, Message> {
            let state = match (p.online, p.free) {
                (true, Some(f)) => format!("● {} · {} free", p.name, gb(f)),
                (true, None) => format!("● {}", p.name),
                (false, _) if p.path.is_none() => format!("○ {} · not set", p.name),
                (false, _) => format!("○ {} · offline", p.name),
            };
            let path = p.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let online = p.online;
            super::tip(
                container(text(state).size(13)).padding([4, 10]).style(move |t: &Theme| {
                    let mut s = container::rounded_box(t);
                    if !online {
                        s.text_color = Some(t.extended_palette().background.strong.text);
                    }
                    s
                }),
                text(path),
                tooltip::Position::Bottom,
            )
            .into()
        };
        row(self.places.iter().map(chip)).spacing(8).align_y(Alignment::Center).into()
    }

    /// A trip: its name, its dates, its edits of every day, its days as
    /// cards.
    fn trip_row<'a>(&'a self, trip: &'a Trip) -> Element<'a, Message> {
        let dates = match (trip.days.first(), trip.days.last()) {
            (Some(a), Some(b)) if a.day != b.day => format!("{} – {}", a.day, b.day),
            (Some(a), _) => a.day.clone(),
            _ => String::new(),
        };
        // The whole project into Omnix (user, 2026-11-05: an empty edit
        // with every day in the bin, not one day's).
        let all = |label: &'static str, msg: Message, hint: &'static str| super::tip(button(text(label).size(13)).padding([3, 10]).style(button::secondary).on_press(msg), text(hint), tooltip::Position::Bottom);
        let name: Element<'a, Message> = match &self.naming {
            Some((root, typed)) if *root == trip.root => text_input("the folder's name", typed).id(NAME).size(18).width(320).on_input(Message::NameEdit).on_submit(Message::NameSave).into(),
            _ => super::tip(button(text(trip.title.clone()).size(18)).padding(0).style(button::text).on_press(Message::Name(trip.root.clone())), text("Rename (the folder keeps its name)"), tooltip::Position::Bottom).into(),
        };
        let head = row![
            name,
            text(format!("{dates} · {} days · {} clips", trip.days.len(), trip.clips())).size(13).style(text::secondary),
            all("→ Omnix (all days)", Message::ToOmnix(trip.root.clone(), None), "An edit of every day from its marking, opened in Omnix"),
            all("Empty edit", Message::EmptyDay(trip.root.clone(), None), "An empty edit in Omnix, every day's clips in its bin"),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let cards = iced::widget::row(trip.days.iter().map(|d| self.day_card(trip, d))).spacing(10).wrap().vertical_spacing(10);
        column![head, cards].spacing(10).into()
    }

    /// A day as a card; a click opens it in the contact sheet.
    fn day_card<'a>(&'a self, trip: &'a Trip, d: &'a library::Day) -> Element<'a, Message> {
        let h = (CARD * 9.0 / 16.0).round();
        let poster: Element<'a, Message> = match &d.poster {
            Some(p) => image(image::Handle::from_path(p)).width(CARD).height(h).into(),
            None => container(text(d.day.clone()).size(13).style(text::secondary)).width(CARD).height(h).center_x(CARD).center_y(h).style(container::rounded_box).into(),
        };
        let rated = if d.clips == 0 { 0.0 } else { d.rated as f32 / d.clips as f32 };
        let story = if d.story { "✎" } else { "" };
        let sources = match d.in_sources {
            Some(true) => "sources ✓",
            Some(false) => "sources —",
            None => "sources offline",
        };
        let info = column![
            row![text(d.day.clone()).size(14), space().width(Length::Fill), text(story).size(14)].align_y(Alignment::Center),
            text(format!("{} clips · {} rated", d.clips, d.rated)).size(12).style(text::secondary),
            progress_bar(0.0..=1.0, rated).girth(4),
            text(format!("project {}/{} · {sources}", d.in_project, d.clips)).size(11).style(text::secondary),
        ]
        .spacing(4)
        .width(CARD);
        let open = button(column![poster, info].spacing(6)).padding(0).style(button::text).on_press(Message::OpenDay(trip.root.clone(), d.day.clone()));
        // The day straight into Omnix: its edit from the marking, or empty.
        let small = |label: &'static str, msg: Message, hint: &'static str| super::tip(button(text(label).size(12)).padding([2, 8]).style(button::secondary).on_press(msg), text(hint), tooltip::Position::Bottom);
        let actions = row![
            small("→ Omnix", Message::ToOmnix(trip.root.clone(), Some(d.day.clone())), "An edit of this day from its marking, opened in Omnix"),
            small("Empty day", Message::EmptyDay(trip.root.clone(), Some(d.day.clone())), "An empty edit of this day in Omnix, its clips in the bin"),
        ]
        .spacing(6);
        container(column![open, actions].spacing(6).width(CARD)).padding(6).style(container::rounded_box).into()
    }
}
