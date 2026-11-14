//! Dailix's look in one place (the UI review, 2026-11-14, `docs/UI-
//! REVIEW.md`): buttons in four kinds and two sizes, dialogs alike. Before,
//! a button left without a style drew as the accent by accident («Open…»
//! the loudest thing in the bar), secondary ones came in four sizes and
//! dialogs in four paddings.

use iced::widget::{Button, Container, button, container, text};
use iced::{Element, Padding};

/// What a button is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// The view's one main action (one a view, the guides say).
    Primary,
    /// An ordinary action.
    Secondary,
    /// One that reads as text until pointed at: in lists and over cards.
    Quiet,
    /// One that takes something away.
    Danger,
}

const TEXT: f32 = 14.0;
const PAD: Padding = Padding { top: 5.0, bottom: 5.0, left: 12.0, right: 12.0 };
const SMALL_TEXT: f32 = 12.0;
const SMALL_PAD: Padding = Padding { top: 2.0, bottom: 2.0, left: 8.0, right: 8.0 };

/// A dialog's padding, the room between its parts, its usual width.
pub const DIALOG_PAD: f32 = 18.0;
pub const DIALOG_GAP: f32 = 12.0;
pub const DIALOG_WIDTH: f32 = 520.0;

/// The style of a `kind` of button.
pub fn style(kind: Kind) -> fn(&iced::Theme, button::Status) -> button::Style {
    match kind {
        Kind::Primary => button::primary,
        Kind::Secondary => button::secondary,
        Kind::Quiet => button::text,
        Kind::Danger => button::danger,
    }
}

/// A button: `label`, the one size every ordinary button has.
pub fn act<'a, M>(label: impl text::IntoFragment<'a>, kind: Kind) -> Button<'a, M> {
    button(text(label).size(TEXT)).padding(PAD).style(style(kind))
}

/// A small one: in a card, a tile, the player's pane.
pub fn small<'a, M>(label: impl text::IntoFragment<'a>, kind: Kind) -> Button<'a, M> {
    button(text(label).size(SMALL_TEXT)).padding(SMALL_PAD).style(style(kind))
}

/// A dialog's card around `content`, `width` wide.
pub fn dialog<'a, M: 'a>(content: impl Into<Element<'a, M>>, width: f32) -> Container<'a, M> {
    container(content).padding(DIALOG_PAD).width(width).style(container::rounded_box)
}
