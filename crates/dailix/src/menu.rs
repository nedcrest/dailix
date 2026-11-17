//! Pop-up menus (the UI review, 2026-11-14, `docs/UI-REVIEW.md`): the top
//! bar's ☰, a split button's ▾, a card's ⋯. A menu opens under the button
//! that asked for it: the button is found among the window's widgets by
//! its id (`anchor`), its place on the screen worked out past any scroll
//! (`on_screen`), and the menu pinned under it (`place`). A click outside
//! or Esc closes it; an entry closes it and does its message.

use super::{App, Message};
use crate::ui;
use iced::widget::{Id, button, column, container, row, rule, space, text};
use iced::{Element, Length, Point, Rectangle, Task, Vector};
use std::path::PathBuf;

/// Which menu.
#[derive(Clone, Debug, PartialEq)]
pub enum Menu {
    /// ☰: text size, open, keys, settings, about.
    Main,
    /// «→ Omnix ▾» of a trip (no day) or a day: the other ways in.
    Omnix(PathBuf, Option<String>),
    /// A trip's ⋯ (its page, its card).
    Trip(PathBuf),
    /// An edit's ⋯.
    Edit(PathBuf),
}

/// What a menu holds, a line each.
pub enum Entry {
    Act { label: String, key: &'static str, msg: Message, danger: bool },
    Rule,
}

fn act(label: impl Into<String>, key: &'static str, msg: Message) -> Entry {
    Entry::Act { label: label.into(), key, msg, danger: false }
}

/// A menu's width.
const WIDTH: f32 = 250.0;

/// The id of the button `menu` opens from.
pub fn anchor(menu: &Menu) -> Id {
    Id::from(format!("menu {menu:?}"))
}

/// A button opening `menu`, `label` on it.
pub fn opener<'a>(label: &'a str, menu: Menu, kind: ui::Kind, small: bool) -> Element<'a, Message> {
    let b = if small { ui::small(label, kind) } else { ui::act(label, kind) };
    container(b.on_press(Message::Menu(Some(menu.clone())))).id(anchor(&menu)).into()
}

/// Where `found` (a widget's bounds as laid out) is on the screen: moved
/// by how far each scrollable holding it is scrolled.
pub fn on_screen(found: Rectangle, scrolls: &[(Rectangle, Vector)]) -> Rectangle {
    let held = scrolls.iter().filter(|(content, _)| content.contains(found.position()));
    let by = held.fold(Vector::ZERO, |a, (_, t)| a + *t);
    Rectangle { x: found.x - by.x, y: found.y - by.y, ..found }
}

/// Where a menu `width` wide goes under `button` in a window `room` wide:
/// its left edge under the button's, or its right under the button's when
/// it wouldn't fit.
pub fn place(button: Rectangle, width: f32, room: f32) -> Point {
    let x = if button.x + width <= room { button.x } else { (button.x + button.width - width).max(0.0) };
    Point::new(x, button.y + button.height + 4.0)
}

/// Finds the button with id `id`, and every scrollable on the way.
struct Locate {
    id: Id,
    scrolls: Vec<(Rectangle, Vector)>,
    found: Option<Rectangle>,
}

impl iced::advanced::widget::Operation<Option<Rectangle>> for Locate {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<Option<Rectangle>>)) {
        operate(self);
    }
    fn scrollable(&mut self, _: Option<&Id>, _: Rectangle, content: Rectangle, translation: Vector, _: &mut dyn iced::advanced::widget::operation::Scrollable) {
        if self.found.is_none() {
            self.scrolls.push((content, translation));
        }
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if self.found.is_none() && id == Some(&self.id) {
            self.found = Some(on_screen(bounds, &self.scrolls));
        }
    }
    /// Always an answer, found or not: a menu whose button isn't found
    /// closes, rather than waiting unseen.
    fn finish(&self) -> iced::advanced::widget::operation::Outcome<Option<Rectangle>> {
        iced::advanced::widget::operation::Outcome::Some(self.found)
    }
}

impl App {
    /// A menu opened (again: closed), closed (none), placed, or one of its
    /// entries chosen.
    pub(super) fn on_menu(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(Some(m)) if self.menu.as_ref().is_some_and(|(open, _)| *open == m) => self.menu = None,
            Message::Menu(Some(m)) => {
                let id = anchor(&m);
                self.menu = Some((m, None));
                return iced::advanced::widget::operate(Locate { id, scrolls: Vec::new(), found: None }).map(Message::MenuAt);
            }
            Message::Menu(None) => self.menu = None,
            // Its button gone from the page (or not found): no menu.
            Message::MenuAt(None) => self.menu = None,
            Message::MenuAt(Some(at)) => {
                if let Some((_, place)) = &mut self.menu {
                    *place = Some(at);
                }
            }
            Message::MenuDo(m) => {
                self.menu = None;
                return self.update(*m);
            }
            _ => {}
        }
        Task::none()
    }

    /// What `menu` holds.
    fn entries(&self, menu: &Menu) -> Vec<Entry> {
        match menu {
            Menu::Main => vec![
                act("Smaller text", "Ctrl+−", Message::Scale(-1)),
                act("Bigger text", "Ctrl+=", Message::Scale(1)),
                act(format!("Text at 100% (now {:.0}%)", self.scale * 100.0), "Ctrl+0", Message::Scale(0)),
                Entry::Rule,
                act("Open a trip folder…", "", Message::Open),
                act("How Dailix works", "", Message::HowItWorks),
                act("Keyboard shortcuts", "F1", Message::Help),
                act("Settings", "Ctrl+,", Message::Settings),
                act("About Dailix", "", Message::About),
            ],
            Menu::Omnix(root, day) => {
                let mut e = vec![act(if day.is_some() { "Empty edit of this day" } else { "Empty edit, every day in the bin" }, "", Message::EmptyDay(root.clone(), day.clone()))];
                if day.is_none() {
                    let days = self.trips.iter().find(|t| &t.root == root).map(|t| t.days.iter().map(|d| d.day.clone()).collect()).unwrap_or_else(Vec::new);
                    if !days.is_empty() {
                        e.push(Entry::Rule);
                    }
                    e.extend(days.into_iter().map(|d| act(format!("Only {d}"), "", Message::ToOmnix(root.clone(), Some(d)))));
                }
                e
            }
            Menu::Trip(root) => {
                // «Open» on its card, not on its page.
                let open = (self.trip_open.as_ref() != Some(root) || !self.at_home).then(|| act("Open", "", Message::OpenTrip(root.clone())));
                open.into_iter().chain([
                act("Add videos…", "", Message::AddVideos(root.clone())),
                act("Rename", "", Message::Name(root.clone())),
                act("Show in the file manager", "", Message::ShowFolder(root.clone())),
                Entry::Rule,
                Entry::Act { label: "Delete trip…".into(), key: "", msg: Message::DeleteAsk(crate::delete::Doomed::Trip(root.clone())), danger: true },
                ]).collect()
            }
            Menu::Edit(edit) => vec![
                act("Open in Omnix", "", Message::OpenEdit(edit.clone())),
                act("Show in the file manager", "", Message::ShowFolder(edit.clone())),
                Entry::Rule,
                Entry::Act { label: "Move to trash…".into(), key: "", msg: Message::DeleteAsk(crate::delete::Doomed::Edit(edit.clone())), danger: true },
            ],
        }
    }

    /// The open menu over the page, under its button; nothing else.
    pub(super) fn menu_layer(&self) -> Element<'_, Message> {
        let Some((menu, Some(at))) = &self.menu else { return space().into() };
        let line = |e: Entry| -> Element<'_, Message> {
            match e {
                Entry::Rule => rule::horizontal(1).into(),
                Entry::Act { label, key, msg, danger } => {
                    let words = row![text(label).size(14).width(Length::Fill)].push((!key.is_empty()).then(|| text(key).size(12).style(text::secondary))).spacing(12);
                    let b = button(words).width(Length::Fill).padding([5, 10]).style(button::text).on_press(Message::MenuDo(Box::new(msg)));
                    // Red words, not a red block: last and apart is enough.
                    if danger {
                        b.style(|t: &iced::Theme, st| button::Style { text_color: t.extended_palette().danger.base.color, ..button::text(t, st) }).into()
                    } else {
                        b.into()
                    }
                }
            }
        };
        let card = container(column(self.entries(menu).into_iter().map(line)).spacing(2)).padding(6).width(WIDTH).style(container::bordered_box);
        let at = place(*at, WIDTH, self.width / self.scale);
        let pinned = iced::widget::pin(iced::widget::opaque(card)).position(at);
        iced::widget::opaque(iced::widget::mouse_area(pinned).on_press(Message::Menu(None)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_in_a_scrolled_list_is_where_it_shows() {
        let found = Rectangle { x: 40.0, y: 900.0, width: 30.0, height: 20.0 };
        let list = (Rectangle { x: 0.0, y: 60.0, width: 800.0, height: 2000.0 }, Vector::new(0.0, 700.0));
        let other = (Rectangle { x: 900.0, y: 60.0, width: 300.0, height: 400.0 }, Vector::new(0.0, 50.0));
        assert_eq!(on_screen(found, &[list, other]).y, 200.0);
        assert_eq!(on_screen(found, &[]), found, "not in a list: as laid out");
    }

    #[test]
    fn a_menu_opens_under_its_button_and_stays_in_the_window() {
        let b = Rectangle { x: 100.0, y: 10.0, width: 40.0, height: 30.0 };
        assert_eq!(place(b, 250.0, 1280.0), Point::new(100.0, 44.0));
        // The ☰ at the right: its right edge under the button's.
        let end = Rectangle { x: 1230.0, ..b };
        assert_eq!(place(end, 250.0, 1280.0), Point::new(1020.0, 44.0));
    }

    #[test]
    fn a_menu_is_known_by_its_button() {
        let p = PathBuf::from("trip");
        assert_eq!(anchor(&Menu::Trip(p.clone())), anchor(&Menu::Trip(p.clone())));
        assert_ne!(anchor(&Menu::Trip(p.clone())), anchor(&Menu::Edit(p)));
    }
}
