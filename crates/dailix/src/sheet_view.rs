//! The contact sheet's page: the filter bar, the days and their tiles
//! (the UI review, 2026-11-14, step 4: Lightroom's and Final Cut's way).
//! A tile shows its rating as a colour (a line along its top; a junked one
//! dimmed), the ✓ │ ? │ ✕ buttons only on the tile picked or under the
//! pointer; the bar over the tiles shows all, the unrated, or one rating,
//! with their counts.

use super::{App, Message, Project, TILE, home, rating_value, tip};
use crate::ui;
use dailix_sheet::footage;
use dailix_sheet::MarkKind;
use iced::widget::{column, container, image, row, space, text, text_editor, tooltip};
use iced::{Alignment, Color, Element, Length, Task, Theme};
use dailix_sheet::{ClipOp, Op};

/// Which tiles the sheet shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Unrated,
    /// One rating: 2 take, 1 think about it, 0 junk.
    Rated(u8),
}

impl Filter {
    pub const ALL: [Filter; 5] = [Filter::All, Filter::Unrated, Filter::Rated(2), Filter::Rated(1), Filter::Rated(0)];

    /// Whether a clip rated `rating` shows.
    pub fn shows(self, rating: Option<u8>) -> bool {
        match self {
            Filter::All => true,
            Filter::Unrated => rating.is_none(),
            Filter::Rated(r) => rating == Some(r),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Unrated => "Unrated",
            Filter::Rated(2) => "✓",
            Filter::Rated(1) => "?",
            Filter::Rated(_) => "✕",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Filter::All => "Every clip",
            Filter::Unrated => "The clips not rated yet: what's left to look at",
            Filter::Rated(2) => "The takes",
            Filter::Rated(1) => "The ones to think about",
            Filter::Rated(_) => "The junk",
        }
    }
}

/// How many of `ratings` each filter shows, in `Filter::ALL`'s order.
pub fn counts(ratings: impl IntoIterator<Item = Option<u8>>) -> [usize; 5] {
    let mut n = [0; 5];
    for r in ratings {
        for (i, f) in Filter::ALL.iter().enumerate() {
            n[i] += usize::from(f.shows(r));
        }
    }
    n
}

/// The clip `by` places from `at` among those `shown`, in `order`: from
/// where `at` stands even when it no longer shows (rated out of the
/// filter, the next unrated after it); the first shown without one.
pub fn step_from(order: &[String], shown: impl Fn(&str) -> bool, at: Option<&str>, by: isize) -> Option<String> {
    let Some(i) = at.and_then(|a| order.iter().position(|k| k == a)) else { return order.iter().find(|k| shown(k)).cloned() };
    let visible = |k: &&String| shown(k);
    match by {
        0 if shown(&order[i]) => Some(order[i].clone()),
        b if b >= 0 => order[i + 1..].iter().filter(visible).nth((b.max(1) - 1) as usize).or_else(|| order[..=i].iter().rev().find(visible)).cloned(),
        b => order[..i].iter().rev().filter(visible).nth((-b - 1) as usize).or_else(|| order[i..].iter().find(visible)).cloned(),
    }
}

/// A day's heading's id, to jump to it.
pub fn day_id(day: &str) -> String {
    format!("day {day}")
}

/// A rating's colour: green, amber, red.
pub fn rating_color(r: u8) -> Color {
    match r {
        2 => Color::from_rgb8(0x3f, 0xb9, 0x50),
        1 => Color::from_rgb8(0xd2, 0x99, 0x22),
        _ => Color::from_rgb8(0xf8, 0x51, 0x49),
    }
}

impl App {
    /// The sheet's own messages: the filter, the pointer on a tile, a
    /// rating by a click, a story typed and resting, the days list.
    pub(super) fn on_sheet(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Filter(f) => {
                self.filter = f;
                // The picked clip hidden: the first one shown instead.
                return self.step(0);
            }
            Message::TileHover(key, true) => self.tile_hover = Some(key),
            Message::TileHover(key, false) => {
                if self.tile_hover.as_ref() == Some(&key) {
                    self.tile_hover = None;
                }
            }
            Message::Rate(clip, rating) => {
                // The same rating again takes it off, as the sheet does.
                let now = self.project.as_ref().and_then(|p| p.sheet.clips.get(&clip)).and_then(|c| c.rating).map(rating_value);
                let rating = if now == rating { None } else { rating };
                self.change(Op::Clip(ClipOp::Rate { clip, rating }));
            }
            Message::DayEdit(day, action) => {
                if let Some(c) = self.drafts.get_mut(&day) {
                    let edit = action.is_edit();
                    c.perform(action);
                    if edit {
                        self.dirty.insert(day);
                        self.story_edits += 1;
                        let n = self.story_edits;
                        return Task::perform(super::blocking(|| std::thread::sleep(std::time::Duration::from_millis(1200))), move |()| Message::StoryRest(n));
                    }
                }
            }
            Message::StoryRest(n) => {
                if n == self.story_edits {
                    let days: Vec<String> = self.dirty.iter().cloned().collect();
                    for day in days {
                        self.save_story(&day);
                    }
                }
            }
            Message::DaysList => self.days_list = !self.days_list,
            Message::JumpDay(day) => return iced::advanced::widget::operate(super::Find { tile: iced::widget::Id::from(day_id(&day)), list: None, found: None }).map(Message::DayAt),
            Message::DayAt(p) => return iced::widget::operation::scroll_to(super::LIST, iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: (p.tile.y - p.content.y - 4.0).max(0.0) }),
            _ => {}
        }
        Task::none()
    }

    /// A clip's rating, if it has one.
    pub(super) fn rating_of(p: &Project, key: &str) -> Option<u8> {
        p.sheet.clips.get(key).and_then(|m| m.rating).map(rating_value)
    }

    /// The bar over the tiles: the filters, linked, each with its count.
    pub(super) fn filter_bar<'a>(&'a self, p: &'a Project) -> Element<'a, Message> {
        let n = counts(p.footage.days.iter().flat_map(|d| &d.clips).map(|c| Self::rating_of(p, &c.key)));
        let one = |(i, f): (usize, &Filter)| -> Element<'a, Message> {
            let kind = if *f == self.filter { ui::Kind::Primary } else { ui::Kind::Secondary };
            tip(ui::small(format!("{} {}", f.label(), n[i]), kind).on_press(Message::Filter(*f)), text(f.hint()), tooltip::Position::Bottom)
        };
        let linked = iced::widget::Row::with_children(Filter::ALL.iter().enumerate().map(one)).spacing(1);
        row![text("Show").size(13).style(text::secondary), linked].spacing(8).align_y(Alignment::Center).into()
    }

    /// A day: its date, how far it's rated, its story, its clips shown.
    pub(super) fn day<'a>(&'a self, p: &'a Project, day: &'a footage::Day) -> Element<'a, Message> {
        let rated = day.clips.iter().filter(|c| Self::rating_of(p, &c.key).is_some()).count();
        let head = row![text(day.day.clone()).size(18), text(format!("{} · {rated} rated", crate::library::count(day.clips.len(), "clip"))).size(13).style(text::secondary), home::to_omnix(&p.footage.layout.root, Some(&day.day), false)].spacing(12).align_y(Alignment::Center);
        let story: Element<'a, Message> = match self.drafts.get(&day.day) {
            Some(content) => {
                let d = day.day.clone();
                let editor = text_editor(content).placeholder("the day's story").size(14).height(Length::Shrink).on_action(move |a| Message::DayEdit(d.clone(), a));
                // Saved by itself a moment after the typing stops.
                editor.into()
            }
            None => space().into(),
        };
        let shown: Vec<&footage::Clip> = day.clips.iter().filter(|c| self.filter.shows(Self::rating_of(p, &c.key))).collect();
        let tiles: Element<'a, Message> = if shown.is_empty() {
            text("None of this day's clips shows: see the filter above.").size(12).style(text::secondary).into()
        } else {
            iced::widget::row(shown.into_iter().map(|c| self.tile(p, c))).spacing(10).wrap().vertical_spacing(10).into()
        };
        container(column![head, story, tiles].spacing(10)).id(day_id(&day.day)).into()
    }

    /// The days at the sheet's left, a line each: its date, its clips, how
    /// much of it is rated; the picked clip's day lit; a click brings its
    /// heading to the top. Folds away to a narrow «Days» button.
    pub(super) fn days_side<'a>(&'a self, p: &'a Project) -> Element<'a, Message> {
        let fold = tip(ui::small(if self.days_list { "◂" } else { "▸ Days" }, ui::Kind::Quiet).on_press(Message::DaysList), text(if self.days_list { "Fold the days away" } else { "The days, to jump to" }), tooltip::Position::Right);
        if !self.days_list {
            return container(fold).padding([6, 4]).into();
        }
        let here = self.selected.as_ref().and_then(|k| p.footage.days.iter().find(|d| d.clips.iter().any(|c| &c.key == k))).map(|d| d.day.as_str());
        let line = |d: &'a footage::Day| -> Element<'a, Message> {
            let rated = d.clips.iter().filter(|c| Self::rating_of(p, &c.key).is_some()).count();
            let share = if d.clips.is_empty() { 0.0 } else { rated as f32 / d.clips.len() as f32 };
            let words = column![
                row![text(d.day.clone()).size(13), space().width(Length::Fill), text(d.clips.len().to_string()).size(12).style(text::secondary)],
                iced::widget::progress_bar(0.0..=1.0, share).girth(3),
            ]
            .spacing(3);
            let kind = if here == Some(d.day.as_str()) { iced::widget::button::secondary } else { iced::widget::button::text };
            iced::widget::button(words).width(Length::Fill).padding([4, 8]).style(kind).on_press(Message::JumpDay(d.day.clone())).into()
        };
        let head = row![text("Days").size(13).style(text::secondary), space().width(Length::Fill), fold].align_y(Alignment::Center);
        let list = iced::widget::scrollable(column(p.footage.days.iter().map(line)).spacing(2)).height(Length::Fill);
        container(column![head, list].spacing(6)).width(150).padding([6, 6]).into()
    }

    /// A clip as a tile: its rating a line of colour along the top.
    fn tile<'a>(&'a self, p: &'a Project, c: &'a footage::Clip) -> Element<'a, Message> {
        let h = (TILE * 9.0 / 16.0).round();
        let mark = p.sheet.clips.get(&c.key);
        let rating = Self::rating_of(p, &c.key);
        let poster: Element<'a, Message> = if c.poster.is_file() && !self.making.contains(&c.poster) {
            // Junk dimmed, as Lightroom's rejects.
            image(image::Handle::from_path(&c.poster)).width(TILE).height(h).opacity(if rating == Some(0) { 0.4_f32 } else { 1.0 }).into()
        } else {
            container(text(if c.path.is_file() { "…" } else { "missing" }).size(14)).width(TILE).height(h).center_x(TILE).center_y(h).style(container::rounded_box).into()
        };
        let picked = self.selected.as_deref() == Some(c.key.as_str());
        let near = picked || self.tile_hover.as_deref() == Some(c.key.as_str());
        let rate = |r: u8, label: &'static str, hint: &'static str| -> Element<'a, Message> {
            let kind = if rating == Some(r) { ui::Kind::Primary } else { ui::Kind::Secondary };
            tip(ui::small(label, kind).on_press(Message::Rate(c.key.clone(), Some(r))), text(hint), tooltip::Position::Top)
        };
        // The three linked, on the tile in hand; else the rating as a word.
        let rating_line: Element<'a, Message> = if near {
            row![rate(2, "✓", "Take (P); again: unrated"), rate(1, "?", "Think about it (?)"), rate(0, "✕", "Junk (X)")].spacing(1).into()
        } else {
            let (word, r) = match rating {
                Some(2) => ("✓ take", 2),
                Some(1) => ("? think", 1),
                Some(r) => ("✕ junk", r),
                None => ("", 0),
            };
            text(word).size(12).color(rating_color(r)).into()
        };
        let segments = mark.map_or(0, |m| m.marks.iter().filter(|x| x.kind != MarkKind::Flag).count());
        let flags = mark.map_or(0, |m| m.marks.iter().filter(|x| x.kind == MarkKind::Flag).count());
        let count = |n: usize, one: &str| crate::library::count(n, one);
        let counts = match (segments, flags) {
            (0, 0) => String::new(),
            (s, 0) => count(s, "segment"),
            (0, f) => count(f, "flag"),
            (s, f) => format!("{} · {}", count(s, "segment"), count(f, "flag")),
        };
        let note = mark.map(|m| m.note.clone()).unwrap_or_default();
        let info = column![
            text(c.file.clone()).size(12).wrapping(text::Wrapping::None),
            row![text(dailix_sheet::clock(c.duration, 0)).size(12).style(text::secondary), space().width(Length::Fill), rating_line].spacing(4).align_y(Alignment::Center).height(22),
            text(counts).size(12).style(text::secondary),
            text(note).size(12),
        ]
        .spacing(4)
        .width(TILE)
        .clip(true);
        let band = container(space().height(3).width(TILE)).style(move |_: &Theme| container::Style { background: rating.map(|r| rating_color(r).into()), ..Default::default() });
        let tile = container(column![band, poster, info].spacing(5)).padding(6).id(c.key.clone()).style(move |t: &Theme| {
            let mut s = container::rounded_box(t);
            if picked {
                s.border = iced::Border { color: t.extended_palette().primary.strong.color, width: 2.0, radius: s.border.radius };
            }
            s
        });
        let (on, off) = (c.key.clone(), c.key.clone());
        iced::widget::mouse_area(tile).on_press(Message::Select(c.key.clone())).on_enter(Message::TileHover(on, true)).on_exit(Message::TileHover(off, false)).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dailix_sheet::Rating;

    /// A rating as the sheet keeps it.
    fn r(x: Option<Rating>) -> Option<u8> {
        x.map(rating_value)
    }

    #[test]
    fn a_filter_shows_what_it_says_and_counts_it() {
        let ratings = [r(Some(Rating::Take)), None, r(Some(Rating::Junk)), None, r(Some(Rating::Take))];
        assert_eq!(counts(ratings), [5, 2, 2, 0, 1]);
        assert!(Filter::Unrated.shows(None) && !Filter::Unrated.shows(Some(2)));
        assert!(Filter::Rated(0).shows(Some(0)) && !Filter::Rated(0).shows(None));
    }

    #[test]
    fn the_next_clip_is_the_next_one_shown_even_from_one_rated_away() {
        let order: Vec<String> = ["a", "b", "c", "d"].map(String::from).into();
        // Only b and d show (the unrated, say); a was just rated.
        let shown = |k: &str| k == "b" || k == "d";
        assert_eq!(step_from(&order, shown, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(step_from(&order, shown, Some("c"), 1).as_deref(), Some("d"));
        assert_eq!(step_from(&order, shown, Some("c"), -1).as_deref(), Some("b"));
        // At the end: stays on the last shown, not off the list.
        assert_eq!(step_from(&order, shown, Some("d"), 1).as_deref(), Some("d"));
        assert_eq!(step_from(&order, shown, None, 1).as_deref(), Some("b"));
        assert_eq!(step_from(&order, |_| false, Some("a"), 1), None);
    }
}
