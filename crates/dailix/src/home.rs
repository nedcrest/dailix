//! The home view (UI-RESEARCH 3.1, 3.3, 3.4; user, 2026-11-07): the places
//! at the top (each there or not, its free space), then a card per trip
//! (a poster, its name, dates, days and clips, its description and tags).
//! A card opens the trip's page: its description and tags to write, its
//! edits, its days as cards (a poster, the clips and how many are rated,
//! ✎ when the day's story is written, where its files are). Folders stay
//! out of sight: a day's card opens it in the contact sheet.

use super::{App, Message, modal, pane};
use std::path::{Path, PathBuf};
use crate::library::{self, Place, Trip};
use iced::widget::{button, column, container, image, progress_bar, row, rule, scrollable, space, text, text_input, tooltip};
use iced::{Alignment, Element, Length, Task, Theme};

/// The field a project is named in, focused when it opens.
pub(super) const NAME: &str = "project-name";

/// A day card's width, and a trip card's; their posters are 16:9.
const CARD: f32 = 200.0;
const TRIP_CARD: f32 = 320.0;
/// An edit's card, two side by side in the question before a new one.
const EDIT_CARD: f32 = 236.0;

fn gb(bytes: u64) -> String {
    format!("{:.0} GB", bytes as f64 / 1e9)
}

/// A day's (or the trip's) videos offered before making another
/// (Omnix gap 104): the trip, the day (none: all days), whether an empty
/// one was asked for, and the videos there, newest first.
#[derive(Clone, Debug)]
pub struct Choosing {
    pub root: PathBuf,
    pub day: Option<String>,
    pub empty: bool,
    /// Each video's edit and when it was last changed (read once, as the
    /// question opens).
    pub edits: Vec<(PathBuf, String)>,
}

/// When `edit` was last changed, in local time.
fn changed(edit: &Path) -> String {
    std::fs::metadata(edit).and_then(|m| m.modified()).map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string()).unwrap_or_default()
}

impl App {
    /// «→ Omnix» (`empty`: «Empty day») on a day, or the trip (no day):
    /// its videos made so before offered first, else a new one made and
    /// opened (the user, 2026-11-11: each click made another copy).
    fn day_to_omnix(&mut self, root: PathBuf, day: Option<String>, empty: bool) {
        self.save_note();
        let edits: Vec<(PathBuf, String)> = dailix_sheet::project::videos_named(&root, day.as_deref().unwrap_or(dailix_sheet::project::ALL_DAYS)).into_iter().map(|e| {
            let when = changed(&e);
            (e, when)
        }).collect();
        if edits.is_empty() {
            self.make_video(&root, day.as_deref(), empty);
        } else {
            self.choosing = Some(Choosing { root, day, empty, edits });
        }
    }

    /// A new video of `day` (or all days) from the marking, or empty, opened in Omnix.
    fn make_video(&mut self, root: &Path, day: Option<&str>, empty: bool) {
        let made = if empty { omnix::from_sheet::write_empty(root, day) } else { omnix::from_sheet::write(root, day) };
        self.error = made.map_err(|e| format!("{e:#}")).and_then(|edit| crate::editor::open(&edit)).err();
        self.refresh_home();
    }

    /// The videos offered over the window while asked (gap 104).
    pub(super) fn choose_layer(&self) -> Element<'_, Message> {
        let Some(c) = &self.choosing else { return space().into() };
        let what = c.day.as_deref().unwrap_or("The trip");
        let list = iced::widget::row(c.edits.iter().map(|(e, when)| self.edit_card(e, when))).spacing(14).wrap().vertical_spacing(14);
        let new = if c.empty { "New empty video" } else { "New from the marking" };
        let buttons = row![
            space().width(Length::Fill),
            button(text("Cancel")).style(button::secondary).on_press(Message::ChooseCancel),
            button(text(new)).style(button::primary).on_press(Message::ChooseNew),
        ]
        .spacing(8);
        let card = column![text(format!("{what} has videos already")).size(20), text("Open one, or make a new one.").size(13).style(text::secondary), list, buttons].spacing(12);
        modal(container(card).padding(18).width(520).style(container::rounded_box), Message::ChooseCancel)
    }

    /// The home view's messages, and going to and from it: a project
    /// named, a day opened, an edit made for Omnix.
    pub(super) fn on_home(&mut self, message: Message) -> Task<Message> {
        self.let_go_of_cards(&message);
        match message {
            Message::ToOmnix(root, day) => self.day_to_omnix(root, day, false),
            Message::EmptyDay(root, day) => self.day_to_omnix(root, day, true),
            m @ (Message::TripCard(..) | Message::EditCard(..) | Message::EditFrames(..) | Message::CardTick) => return self.on_card(m),
            Message::ChooseNew => {
                if let Some(c) = self.choosing.take() {
                    self.make_video(&c.root, c.day.as_deref(), c.empty);
                }
            }
            Message::ChooseCancel => self.choosing = None,
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
                self.save_about();
                self.pane = pane::Pane::default();
                self.selected = None;
                self.at_home = true;
                self.trip_open = None;
                // Closed here, the next start opens here.
                self.settings.last = None;
                self.settings.last_clip = None;
                self.settings.last_trip = None;
                let _ = self.settings.save();
                self.refresh_home();
            }
            Message::OpenTrip(root) => {
                self.open_trip(root);
                self.settings.last_trip = self.trip_open.clone();
                let _ = self.settings.save();
            }
            Message::AboutDescription(typed) => self.about.0 = typed,
            Message::AboutTags(typed) => self.about.1 = typed,
            Message::AboutSave => {
                self.save_about();
                self.refresh_home();
            }
            Message::OpenEdit(edit) => {
                self.choosing = None;
                self.error = crate::editor::open(&edit).err();
            }
            Message::OpenDay(root, day) => {
                self.save_about();
                self.at_home = false;
                let open = if self.project.as_ref().map(|p| &p.footage.layout.root) != Some(&root) { self.load(root) } else { Task::none() };
                let first = self.project.as_ref().and_then(|p| p.footage.days.iter().find(|d| d.day == day)).and_then(|d| d.clips.first()).map(|c| c.key.clone());
                let pick = first.map_or_else(Task::none, |k| self.select(k));
                return Task::batch([open, pick]);
            }
            _ => {}
        }
        self.follow_edit_frames()
    }

    /// A page left or a question closed takes its cards with it, and a card
    /// gone sends no Exit (iced's mouse_area): the hover goes with them.
    fn let_go_of_cards(&mut self, message: &Message) {
        if leaves_cards(message) {
            self.card_hover = None;
        }
    }

    /// Whether the hovered card's tiles change: it's on the page, and has
    /// frames to change to (`omnix::cards::view::moves`).
    pub(super) fn card_moving(&self) -> bool {
        let Some((card, _)) = self.card_hover.as_ref().filter(|_| self.settings.card_motion()) else { return false };
        let frames = self.trips.iter().find(|t| &t.root == card).map_or_else(|| self.edit_frames.get(card), |t| &t.frames);
        omnix::cards::view::moves(frames.0.len())
    }

    /// The cards' messages: the pointer on a trip's or an edit's card, one
    /// opened, an edit's frames made.
    fn on_card(&mut self, message: Message) -> Task<Message> {
        use omnix::cards::view::Event;
        match message {
            Message::TripCard(root, Event::Open) => return self.on_home(Message::OpenTrip(root)),
            Message::EditCard(edit, Event::Open) => return self.on_home(Message::OpenEdit(edit)),
            Message::TripCard(card, Event::Enter) | Message::EditCard(card, Event::Enter) => self.card_hover = Some((card, std::time::Instant::now())),
            Message::TripCard(card, Event::Exit) | Message::EditCard(card, Event::Exit) => {
                if self.card_hover.as_ref().is_some_and(|(c, _)| *c == card) {
                    self.card_hover = None;
                }
            }
            Message::EditFrames(edit, done) => self.edit_frames.made(&edit, done),
            _ => {}
        }
        self.follow_edit_frames()
    }

    /// The edits shown as cards (the open trip's, the ones offered before
    /// a new one): their frames read, or the next made off the window's
    /// thread.
    pub(super) fn follow_edit_frames(&mut self) -> Task<Message> {
        let open = self.trip_open.as_ref().and_then(|r| self.trips.iter().find(|t| &t.root == r)).map(|t| t.edits.clone()).unwrap_or_default();
        let offered = self.choosing.iter().flat_map(|c| c.edits.iter().map(|(e, _)| e.clone()));
        let edits: Vec<PathBuf> = open.into_iter().chain(offered).collect();
        match self.edit_frames.want(&edits) {
            Some(edit) => Task::perform(super::blocking({
                let edit = edit.clone();
                move || omnix::cards::make(&edit).map(|_| ()).map_err(|e| format!("{e:#}"))
            }), move |done| Message::EditFrames(edit.clone(), done)),
            None => Task::none(),
        }
    }

    /// Edit `edit` as a small card: its frames, its name and `note`.
    fn edit_card<'a>(&'a self, edit: &'a Path, note: &str) -> Element<'a, Message> {
        let hovered = self.card_hover.as_ref().filter(|(e, _)| e == edit).map(|(_, since)| self.settings.card_motion().then(|| since.elapsed()));
        let note = if self.edit_frames.making(edit) { "making its frames…" } else { note };
        let info = column![text(dailix_sheet::project::edit_name(edit)).size(14)].push((!note.is_empty()).then(|| text(note.to_owned()).size(12).style(text::secondary))).spacing(2);
        let to = edit.to_owned();
        omnix::cards::view::card(self.edit_frames.get(edit), hovered, EDIT_CARD, false, info.into(), 38.0, move |e| Message::EditCard(to.clone(), e))
    }

    /// The trip's page shown, its description and tags ready to edit.
    pub(super) fn open_trip(&mut self, root: std::path::PathBuf) {
        let about = dailix_sheet::project::about(&root);
        self.about = (about.description, about.tags.join(", "));
        self.trip_open = Some(root);
    }

    /// The open trip's description and tags written, if they changed.
    fn save_about(&mut self) {
        let Some(root) = &self.trip_open else { return };
        let typed = dailix_sheet::project::About { description: self.about.0.trim().to_owned(), tags: dailix_sheet::project::parse_tags(&self.about.1) };
        if typed != dailix_sheet::project::about(root)
            && let Err(e) = dailix_sheet::project::set_about(root, &typed)
        {
            self.error = Some(format!("{}: {e}", root.display()));
        }
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
        // Before any trip: what to set, and a button to set it.
        if let Some((said, choose)) = library::first_run(&self.places, self.trips.len()) {
            let choose = choose.map(|kind| button(text(format!("Choose the {} folder…", kind.name())).size(14)).padding([5, 12]).on_press(Message::ChoosePlace(kind)));
            page = page.push(column![text(said).size(15)].push(choose).spacing(10).max_width(640));
        }
        match self.trip_open.as_ref().and_then(|r| self.trips.iter().find(|t| &t.root == r)) {
            Some(trip) => page = page.push(rule::horizontal(1)).push(self.trip_page(trip)),
            None => {
                // A trip needs the projects place: none yet, the first run's
                // button above chooses it.
                let place = self.places.iter().any(|p| p.kind == crate::settings::PlaceKind::Projects && p.online);
                let new = place.then(|| super::tip(button(text("+ New trip").size(14)).padding([5, 12]).on_press(Message::NewTrip), text("A new trip in the projects place, in the fixed layout (Ctrl+N)"), tooltip::Position::Bottom));
                page = page.push(new).push(iced::widget::row(self.trips.iter().map(|t| self.trip_card(t))).spacing(14).wrap().vertical_spacing(14));
            }
        }
        scrollable(page).height(Length::Fill).into()
    }

    /// The places, a chip each: there (●) or not (○), and the free space.
    fn places_strip(&self) -> Element<'_, Message> {
        let chip = |p: &Place| -> Element<'_, Message> {
            let state = match (p.online, p.free) {
                (true, Some(f)) => format!("● {} · {} free", p.kind.name(), gb(f)),
                (true, None) => format!("● {}", p.kind.name()),
                (false, _) if p.path.is_none() => format!("○ {} · not set", p.kind.name()),
                (false, _) => format!("○ {} · offline", p.kind.name()),
            };
            let path = p.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let online = p.online;
            let chip = container(text(state).size(13)).padding([4, 10]).style(move |t: &Theme| {
                let mut s = container::rounded_box(t);
                if !online {
                    s.text_color = Some(t.extended_palette().background.strong.text);
                }
                s
            });
            // A click chooses its folder (user, 2026-11-07), as Settings does.
            let hint = if path.is_empty() { format!("Choose the {} folder", p.kind.name()) } else { format!("{path}\nClick to choose another folder") };
            super::tip(button(chip).padding(0).style(button::text).on_press(Message::ChoosePlace(p.kind)), text(hint), tooltip::Position::Bottom)
        };
        row(self.places.iter().map(chip)).spacing(8).align_y(Alignment::Center).into()
    }

    /// A trip as a card on the home view; a click opens its page.
    fn trip_card<'a>(&'a self, trip: &'a Trip) -> Element<'a, Message> {
        let mut info = column![
            text(trip.title.clone()).size(17),
            text(format!("{} · {} days · {} clips · {} videos", trip.dates(), trip.days.len(), trip.clips(), trip.edits.len())).size(12).style(text::secondary),
        ]
        .spacing(4);
        if let Some(first) = trip.about.description.lines().next().filter(|l| !l.is_empty()) {
            info = info.push(text(first.to_owned()).size(13));
        }
        if !trip.about.tags.is_empty() {
            info = info.push(text(trip.about.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join("  ")).size(12).style(text::secondary));
        }
        // Omnix's project card (gap 110): its posters as a 2×2 mosaic,
        // changing while hovered; sheets behind, a trip being a collection.
        let hovered = self.card_hover.as_ref().filter(|(r, _)| *r == trip.root).map(|(_, since)| self.settings.card_motion().then(|| since.elapsed()));
        let root = trip.root.clone();
        omnix::cards::view::card(&trip.frames, hovered, TRIP_CARD, true, info.into(), 84.0, move |e| Message::TripCard(root.clone(), e))
    }

    /// A trip's page: its name, dates and whole-trip edits for Omnix; its
    /// description and tags to write; its edits to open; its days.
    fn trip_page<'a>(&'a self, trip: &'a Trip) -> Element<'a, Message> {
        let dates = trip.dates();
        // The whole project into Omnix (user, 2026-11-05: an empty edit
        // with every day in the bin, not one day's).
        let all = |label: &'static str, msg: Message, hint: &'static str| super::tip(button(text(label).size(13)).padding([3, 10]).style(button::secondary).on_press(msg), text(hint), tooltip::Position::Bottom);
        let name: Element<'a, Message> = match &self.naming {
            Some((root, typed)) if *root == trip.root => text_input("the folder's name", typed).id(NAME).size(18).width(320).on_input(Message::NameEdit).on_submit(Message::NameSave).into(),
            _ => super::tip(button(text(trip.title.clone()).size(18)).padding(0).style(button::text).on_press(Message::Name(trip.root.clone())), text("Rename (the folder keeps its name)"), tooltip::Position::Bottom),
        };
        let head = row![
            name,
            text(format!("{dates} · {} days · {} clips", trip.days.len(), trip.clips())).size(13).style(text::secondary),
            all("→ Omnix (all days)", Message::ToOmnix(trip.root.clone(), None), "An edit of every day from its marking, opened in Omnix"),
            all("Empty edit", Message::EmptyDay(trip.root.clone(), None), "An empty edit in Omnix, every day's clips in its bin"),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let about = column![
            text_input("A description: what this trip was", &self.about.0).size(14).on_input(Message::AboutDescription).on_submit(Message::AboutSave),
            text_input("Tags, comma-separated", &self.about.1).size(13).on_input(Message::AboutTags).on_submit(Message::AboutSave),
        ]
        .spacing(6)
        .max_width(760);
        let mut edits = column![text("Videos").size(15)].spacing(4);
        if trip.edits.is_empty() {
            edits = edits.push(text("None yet: → Omnix or Empty edit make one.").size(12).style(text::secondary));
        }
        edits = edits.push(iced::widget::row(trip.edits.iter().map(|e| self.edit_card(e, ""))).spacing(14).wrap().vertical_spacing(14));
        let cards = iced::widget::row(trip.days.iter().map(|d| self.day_card(trip, d))).spacing(10).wrap().vertical_spacing(10);
        column![head, about, edits, text("Days").size(15), cards].spacing(12).into()
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

/// Whether `message` leaves the page or closes the question its cards are
/// on.
fn leaves_cards(message: &Message) -> bool {
    matches!(message, Message::Home | Message::OpenTrip(_) | Message::OpenDay(..) | Message::OpenEdit(_) | Message::ChooseNew | Message::ChooseCancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_a_page_or_a_question_lets_go_of_its_cards() {
        let p = PathBuf::from("trip");
        assert!(leaves_cards(&Message::OpenTrip(p.clone())) && leaves_cards(&Message::Home) && leaves_cards(&Message::ChooseCancel));
        assert!(!leaves_cards(&Message::TripCard(p, omnix::cards::view::Event::Enter)) && !leaves_cards(&Message::CardTick));
    }
}
