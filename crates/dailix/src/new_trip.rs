//! A new trip (user, 2026-11-08): a project is made here, in the fixed
//! layout, never in Omnix. A form over the window: its name as people say
//! it, its folder (the month and the name in Latin until typed over), a
//! description and tags. Made, its page opens.

use crate::ui;
use super::{App, Message, modal};
use iced::widget::{column, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};

/// The form as typed.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub title: String,
    pub folder: String,
    /// The folder typed over: no longer follows the name.
    pub folder_typed: bool,
    pub description: String,
    pub tags: String,
}

/// The field the form opens in.
const TITLE: &str = "new-trip-title";

fn this_month() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    dailix_sheet::project::month_of(now)
}

impl App {
    /// The form's messages.
    pub(super) fn on_new_trip(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewTrip => {
                self.new_trip = Some(Draft { folder: dailix_sheet::project::trip_folder(&this_month(), ""), ..Draft::default() });
                return iced::widget::operation::focus(TITLE);
            }
            Message::NewTripCancel => self.new_trip = None,
            Message::NewTripTitle(t) => {
                if let Some(d) = &mut self.new_trip {
                    if !d.folder_typed {
                        d.folder = dailix_sheet::project::trip_folder(&this_month(), &t);
                    }
                    d.title = t;
                }
            }
            Message::NewTripFolder(f) => {
                if let Some(d) = &mut self.new_trip {
                    d.folder = f;
                    d.folder_typed = true;
                }
            }
            Message::NewTripDescription(t) => {
                if let Some(d) = &mut self.new_trip {
                    d.description = t;
                }
            }
            Message::NewTripTags(t) => {
                if let Some(d) = &mut self.new_trip {
                    d.tags = t;
                }
            }
            Message::NewTripMake => {
                let (Some(d), Some(place)) = (self.new_trip.clone(), self.settings.projects.clone()) else { return Task::none() };
                if d.title.trim().is_empty() || d.folder.trim().is_empty() {
                    return Task::none();
                }
                let about = dailix_sheet::project::About { description: d.description.trim().to_owned(), tags: dailix_sheet::project::parse_tags(&d.tags) };
                match dailix_sheet::project::new_trip(&place, d.folder.trim(), d.title.trim(), &about) {
                    Ok(root) => {
                        self.new_trip = None;
                        self.at_home = true;
                        self.refresh_home();
                        return self.update(Message::OpenTrip(root));
                    }
                    Err(e) => self.error = Some(format!("The trip couldn't be made: {e}")),
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// The form over the window while it's open.
    pub(super) fn new_trip_layer(&self) -> Element<'_, Message> {
        let Some(d) = &self.new_trip else { return space().into() };
        let place = self.settings.projects.as_ref().map(|p| p.display().to_string());
        let ready = place.is_some() && !d.title.trim().is_empty() && !d.folder.trim().is_empty();
        let label = |s: &'static str| text(s).size(13).style(text::secondary);
        let mut body = column![
            text("New trip").size(20),
            label("Its name"),
            text_input("Motorbike ride, November", &d.title).id(TITLE).size(16).on_input(Message::NewTripTitle).on_submit(Message::NewTripMake),
            label("Its folder"),
            text_input("2026-11_moto", &d.folder).size(14).on_input(Message::NewTripFolder).on_submit(Message::NewTripMake),
            label("A description"),
            text_input("What this trip was", &d.description).size(14).on_input(Message::NewTripDescription).on_submit(Message::NewTripMake),
            label("Tags, comma-separated"),
            text_input("moto, hills", &d.tags).size(14).on_input(Message::NewTripTags).on_submit(Message::NewTripMake),
        ]
        .spacing(8);
        body = body.push(match place {
            Some(p) => Element::from(column![label("In the Trips folder"), text(p).size(13)].spacing(2)),
            None => text("No Trips folder set: choose it in Settings first.").size(13).style(text::danger).into(),
        });
        let buttons = row![
            space().width(Length::Fill),
            ui::act("Cancel", ui::Kind::Secondary).on_press(Message::NewTripCancel),
            ui::act("Make the trip", ui::Kind::Primary).on_press_maybe(ready.then_some(Message::NewTripMake)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let card = ui::dialog(column![body, buttons].spacing(ui::DIALOG_GAP), ui::DIALOG_WIDTH);
        modal(card, Message::NewTripCancel)
    }
}
