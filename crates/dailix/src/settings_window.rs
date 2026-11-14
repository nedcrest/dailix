//! The Settings window (user, 2026-11-07: a window of its own over the
//! program, in groups, as Omnix's): the places and how it looks. A
//! folder is chosen by a dialog; `dailix places` still sets any of them.

use crate::ui;
use super::{App, Message, modal, tip};
use crate::settings::PlaceKind;
use iced::widget::{button, column, container, row, rule, scrollable, space, text, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::path::PathBuf;

/// The window's groups, in the order they're listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Places,
    Look,
}

impl Group {
    const ALL: [Group; 2] = [Group::Places, Group::Look];

    fn name(self) -> &'static str {
        match self {
            Group::Places => "Places",
            Group::Look => "Look",
        }
    }
}

impl App {
    /// The Settings window's messages and choosing a place's folder (from
    /// the window or a place's chip on the home view).
    pub(super) fn on_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Settings => self.settings_open = if self.settings_open.is_some() { None } else { Some(Group::Places) },
            Message::SettingsGroup(g) => self.settings_open = Some(g),
            Message::CardMotion(on) => {
                self.settings.card_motion = Some(on);
                if let Err(e) = self.settings.save() {
                    self.error = Some(format!("The settings couldn't be saved: {e}"));
                }
            }
            Message::ChoosePlace(kind) => {
                let start = self.settings.place(kind);
                return Task::perform(
                    async move {
                        let mut d = rfd::AsyncFileDialog::new().set_title(format!("The {} folder", kind.name()));
                        if let Some(s) = start {
                            d = d.set_directory(s);
                        }
                        d.pick_folder().await.map(|f| f.path().to_owned())
                    },
                    move |folder: Option<PathBuf>| Message::PlaceChosen(kind, folder),
                );
            }
            Message::UseDefaultPlaces => {
                let Some(home) = std::env::home_dir() else { return Task::none() };
                for (kind, folder) in crate::library::defaults_for(&self.places, &home) {
                    match std::fs::create_dir_all(&folder) {
                        Ok(()) => self.settings.set_place(kind, folder),
                        Err(e) => self.error = Some(format!("{} couldn't be made: {e}", folder.display())),
                    }
                }
                if let Err(e) = self.settings.save() {
                    self.error = Some(format!("The settings couldn't be saved: {e}"));
                }
                self.refresh_home();
            }
            Message::PlaceChosen(kind, Some(folder)) => {
                self.settings.set_place(kind, folder);
                if let Err(e) = self.settings.save() {
                    self.error = Some(format!("The settings couldn't be saved: {e}"));
                }
                self.refresh_home();
            }
            _ => {}
        }
        Task::none()
    }

    /// The window over the program while it's open: the groups at the
    /// left, the chosen one's settings at the right. ✕, Esc or a click
    /// outside close it.
    pub(super) fn settings_layer(&self) -> Element<'_, Message> {
        let Some(open) = self.settings_open else { return space().into() };
        let close = tip(ui::small("✕", ui::Kind::Quiet).on_press(Message::Settings), text("Close (Esc)"), tooltip::Position::Bottom);
        let groups = column(Group::ALL.iter().map(|&g| button(text(g.name()).size(15)).width(Length::Fill).padding([6, 10]).style(if g == open { button::primary } else { button::text }).on_press(Message::SettingsGroup(g)).into())).spacing(4).width(140);
        let page = match open {
            Group::Places => self.places_page(),
            Group::Look => self.look_page(),
        };
        let card = ui::dialog(
            column![
                row![text("Settings").size(20).width(Length::Fill), close].align_y(Alignment::Center),
                row![groups, rule::vertical(1), scrollable(container(page).padding([0, 12])).width(Length::Fill).height(Length::Fill)].spacing(12),
            ]
            .spacing(ui::DIALOG_GAP),
            720.0,
        )
        .height(440)
        .style(container::rounded_box);
        modal(card, Message::Settings)
    }

    /// Each place: its folder and a button to choose another.
    fn places_page(&self) -> Element<'_, Message> {
        let place = |kind: PlaceKind, what: &'static str| -> Element<'_, Message> {
            let folder = self.settings.place(kind).map_or_else(|| "Not set".to_owned(), |p| p.display().to_string());
            column![
                text(kind.name()).size(16),
                text(what).size(13).style(text::secondary),
                row![text(folder).size(14).width(Length::Fill), ui::act("Choose…", ui::Kind::Secondary).on_press(Message::ChoosePlace(kind))].spacing(8).align_y(Alignment::Center),
            ]
            .spacing(6)
            .into()
        };
        column![
            place(PlaceKind::Projects, "The projects: each a folder of the fixed layout. Omnix starts from it too, until it has its own."),
            place(PlaceKind::Sources, "The originals, read only: a local, removable or network folder."),
            place(PlaceKind::Tmp, "Where new material lands first, before it's checked."),
        ]
        .spacing(18)
        .into()
    }

    /// The text's size.
    fn look_page(&self) -> Element<'_, Message> {
        let size = |label: &'static str, step: i32| ui::act(label, ui::Kind::Secondary).on_press(Message::Scale(step));
        column![
            text("Text size").size(16),
            row![size("A−", -1), text(format!("{:.0}%", self.scale * 100.0)).size(14), size("A+", 1), size("100%", 0)].spacing(8).align_y(Alignment::Center),
            text("Ctrl+− and Ctrl+= do it from anywhere.").size(13).style(text::secondary),
            text("Cards").size(16),
            iced::widget::toggler(self.settings.card_motion()).label("Animate the cards").on_toggle(Message::CardMotion),
            text("Off: a hovered card lights up, its pictures stay still.").size(13).style(text::secondary),
        ]
        .spacing(8)
        .into()
    }
}
