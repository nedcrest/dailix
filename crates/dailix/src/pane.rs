//! The picked clip at the right of the tiles: Omnix's player (`viewer`),
//! a strip with the clip's segments in their roles' colours, and the
//! segments as a list, the picked one's role and note editable. Every
//! change to the marking goes through the sheet's core, by the mark's id.
//!
//! Keys (the window's, read by their place so any layout works): Space
//! plays; I and O set the picked segment's ends, or make a new one from
//! an in point; a role's letter gives the picked segment that role; Delete
//! removes it.

use super::{App, Message};
use dailix_sheet::{MarkKind, MarkOp, Op};
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::{button, column, container, pick_list, row, rule, scrollable, space, text, text_input};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};
use rosmo360::viewer::{Loaded, Viewer};
use std::sync::{Arc, Mutex};

/// The pane's width.
pub const WIDTH: f32 = 600.0;
/// How long the pick rests on a clip before the player opens it.
const REST: std::time::Duration = std::time::Duration::from_millis(250);

/// The roles: key, the sheet's name, its colour, the letter that sets it.
pub const ROLES: [(&str, &str, Color, char); 8] = [
    ("talk", "розмова / sync", Color::from_rgb8(0xc0, 0x56, 0x3a), 't'),
    ("establish", "загальний / establishing", Color::from_rgb8(0x4f, 0x8a, 0x6a), 'e'),
    ("broll", "фон / B-roll", Color::from_rgb8(0x6b, 0x7f, 0x9e), 'b'),
    ("detail", "деталь / insert", Color::from_rgb8(0xc9, 0x8a, 0x5e), 'd'),
    ("cut", "перебивка / cutaway", Color::from_rgb8(0xb0, 0x7a, 0xa8), 'c'),
    ("action", "дія / action", Color::from_rgb8(0xd2, 0x9a, 0x5e), 'a'),
    ("reveal", "ревіл / reveal", Color::from_rgb8(0x7a, 0x6b, 0xab), 'v'),
    ("reaction", "реакція / reaction", Color::from_rgb8(0xd1, 0x8f, 0xa5), 'r'),
];

/// A role's colour; grey for none or one of the user's own.
pub fn colour(role: Option<&str>) -> Color {
    role.and_then(|r| ROLES.iter().find(|x| x.0 == r)).map_or(Color::from_rgb8(0x9a, 0x9a, 0x9a), |x| x.2)
}

/// A clip loaded off the UI thread, handed over once.
#[derive(Clone)]
pub struct Slot(pub Arc<Mutex<Option<Result<Loaded, String>>>>);

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Slot")
    }
}

/// The pane's state: the clip in the player, the segment picked, the flag
/// I left waiting for its O, the picked segment's note as typed, and
/// whether the keys go to the player (Tab) rather than the tiles.
#[derive(Default)]
pub struct Pane {
    pub key: Option<String>,
    pub viewer: Option<Viewer>,
    pub loading: bool,
    pub picked: Option<String>,
    pub open_flag: Option<String>,
    pub note: Option<String>,
    pub focused: bool,
}

/// A role in the pick list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleChoice {
    pub key: String,
    pub name: String,
}

impl std::fmt::Display for RoleChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

fn clock(t: f64) -> String {
    let tenths = (t.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// The pane's messages.
#[derive(Debug, Clone)]
pub enum PaneMsg {
    /// The pick rested on this clip: open it.
    Rested(String),
    Loaded(String, Slot),
    Tick,
    Toggle,
    Scrub(f64),
    ScrubDone,
    Pick(Option<String>),
    Role(Option<String>),
    Note(String),
    NoteSave,
    Remove,
}

impl App {
    /// Shows the clip `key` in the pane and opens it in the player once
    /// the pick has rested on it a moment: rating with the keys runs past
    /// many clips, and each opened would start a decoder (on the GPU).
    pub(super) fn open_clip(&mut self, key: &str) -> iced::Task<Message> {
        if self.pane.key.as_deref() == Some(key) {
            return iced::Task::none();
        }
        self.save_note();
        // The keys stay with the player from clip to clip.
        self.pane = Pane { key: Some(key.to_owned()), loading: true, focused: self.pane.focused, ..Default::default() };
        let key = key.to_owned();
        iced::Task::perform(super::blocking(|| std::thread::sleep(REST)), move |_| Message::Pane(PaneMsg::Rested(key.clone())))
    }

    /// Opens the clip `key` in the player, off the UI thread, if it's
    /// still the one in the pane.
    fn load_clip(&mut self, key: String) -> iced::Task<Message> {
        if self.pane.key.as_deref() != Some(key.as_str()) || self.pane.viewer.is_some() {
            return iced::Task::none();
        }
        let Some(path) = self.project.as_ref().map(|p| p.footage.layout.clip(&key)) else { return iced::Task::none() };
        iced::Task::perform(super::blocking(move || rosmo360::viewer::load(&path).map_err(|e| format!("{e:#}"))), move |r| Message::Pane(PaneMsg::Loaded(key.clone(), Slot(Arc::new(Mutex::new(Some(r)))))))
    }

    pub(super) fn pane_busy(&self) -> bool {
        self.pane.viewer.as_ref().is_some_and(Viewer::busy)
    }

    /// The marks on the clip in the pane, as the sheet holds them.
    fn marks(&self) -> Vec<dailix_sheet::Mark> {
        let (Some(p), Some(key)) = (&self.project, &self.pane.key) else { return Vec::new() };
        p.sheet.clips.get(key).map(|c| c.marks.clone()).unwrap_or_default()
    }

    /// Saves the picked segment's note if it was typed.
    fn save_note(&mut self) {
        if let (Some(note), Some(mark)) = (self.pane.note.take(), self.pane.picked.clone()) {
            self.change(Op::Mark(MarkOp::Note { mark, note }));
        }
    }

    pub(super) fn on_pane(&mut self, msg: PaneMsg) -> iced::Task<Message> {
        match msg {
            PaneMsg::Rested(key) => return self.load_clip(key),
            PaneMsg::Loaded(key, slot) => {
                if self.pane.key.as_deref() != Some(key.as_str()) {
                    return iced::Task::none();
                }
                self.pane.loading = false;
                match slot.0.lock().ok().and_then(|mut s| s.take()) {
                    Some(Ok(loaded)) => self.pane.viewer = Some(Viewer::new(loaded)),
                    Some(Err(e)) => self.error = Some(e),
                    None => {}
                }
            }
            PaneMsg::Tick => {
                if let Some(v) = &mut self.pane.viewer {
                    v.tick();
                }
            }
            PaneMsg::Toggle => {
                if let Some(v) = &mut self.pane.viewer {
                    v.toggle();
                }
            }
            PaneMsg::Scrub(t) => {
                if let Some(v) = &mut self.pane.viewer {
                    v.scrub(t);
                }
            }
            PaneMsg::ScrubDone => {
                if let Some(v) = &mut self.pane.viewer {
                    v.scrub_end();
                }
            }
            PaneMsg::Pick(id) => {
                self.save_note();
                self.pane.picked = id.clone();
                self.pane.open_flag = None;
                if let Some(start) = id.and_then(|id| self.marks().into_iter().find(|m| m.id.as_deref() == Some(id.as_str()))).map(|m| m.t)
                    && let Some(v) = &mut self.pane.viewer
                {
                    v.jump(start);
                }
            }
            PaneMsg::Role(role) => {
                if let Some(mark) = self.pane.picked.clone() {
                    self.change(Op::Mark(MarkOp::Role { mark, role }));
                }
            }
            PaneMsg::Note(text) => self.pane.note = Some(text),
            // Saved, the segment is done with: I and O start the next.
            PaneMsg::NoteSave => {
                self.save_note();
                self.pane.picked = None;
            }
            PaneMsg::Remove => {
                if let Some(mark) = self.pane.picked.take() {
                    self.pane.note = None;
                    self.change(Op::Mark(MarkOp::Remove { mark }));
                }
            }
        }
        iced::Task::none()
    }

    /// The pane's keys: whether `code` was one of them.
    pub(super) fn pane_key(&mut self, code: iced::keyboard::key::Code) -> bool {
        use iced::keyboard::key::Code;
        let now = self.pane.viewer.as_ref().map(Viewer::position);
        let picked = self.pane.picked.clone().and_then(|id| self.marks().into_iter().find(|m| m.id.as_deref() == Some(id.as_str())));
        match code {
            Code::Space => {
                if let Some(v) = &mut self.pane.viewer {
                    v.toggle();
                }
            }
            // I inside the picked segment moves its start. Anywhere else it
            // sets a flag here, which O then makes a segment; one with no
            // O stays a flag. Never stretches a segment the playhead isn't
            // in (it did: one finished further on grew back to an I set
            // earlier in the clip).
            Code::KeyI => {
                let Some(now) = now else { return true };
                match picked {
                    Some(m) if m.kind == MarkKind::Segment && m.end.is_some_and(|e| now >= m.t && now < e) => {
                        self.change(Op::Mark(MarkOp::Move { mark: m.id.clone().unwrap_or_default(), t: now, end: m.end }));
                    }
                    _ => {
                        let Some(clip) = self.pane.key.clone() else { return true };
                        let made = self.change(Op::Mark(MarkOp::Add { clip, t: now, end: None, flag: true, role: None, note: String::new() }));
                        self.pane.picked = made.clone();
                        self.pane.open_flag = made;
                    }
                }
            }
            // O inside the picked segment moves its end; after an I (or on
            // a flag picked), the flag becomes a segment to here, its role
            // and note kept. Without either, nothing: there's no start.
            Code::KeyO => {
                let Some(now) = now else { return true };
                let flag = self.pane.open_flag.clone().or_else(|| picked.as_ref().filter(|m| m.kind == MarkKind::Flag).and_then(|m| m.id.clone()));
                let flag = flag.and_then(|id| self.marks().into_iter().find(|m| m.id.as_deref() == Some(id.as_str())));
                match (picked, flag) {
                    (Some(m), _) if m.kind == MarkKind::Segment && m.end.is_some_and(|e| now > m.t && now <= e) => {
                        self.change(Op::Mark(MarkOp::Move { mark: m.id.clone().unwrap_or_default(), t: m.t, end: Some(now) }));
                    }
                    (_, Some(f)) if now > f.t => {
                        let Some(clip) = self.pane.key.clone() else { return true };
                        let add = Op::Mark(MarkOp::Add { clip, t: f.t, end: Some(now), flag: false, role: f.role.clone(), note: f.note.clone() });
                        let made = self.change_all(vec![Op::Mark(MarkOp::Remove { mark: f.id.clone().unwrap_or_default() }), add]);
                        self.pane.picked = made;
                        self.pane.open_flag = None;
                    }
                    _ => self.error = Some("No start for a segment: set one with I first".to_owned()),
                }
            }
            Code::Delete | Code::Backspace => {
                if let Some(mark) = self.pane.picked.take() {
                    self.change(Op::Mark(MarkOp::Remove { mark }));
                }
            }
            _ => {
                // A role's letter, on the picked segment.
                let letter = match code {
                    Code::KeyT => 't',
                    Code::KeyE => 'e',
                    Code::KeyB => 'b',
                    Code::KeyD => 'd',
                    Code::KeyC => 'c',
                    Code::KeyA => 'a',
                    Code::KeyV => 'v',
                    Code::KeyR => 'r',
                    _ => return false,
                };
                let Some(mark) = self.pane.picked.clone() else { return true };
                let role = ROLES.iter().find(|r| r.3 == letter).map(|r| r.0.to_owned());
                self.change(Op::Mark(MarkOp::Role { mark, role }));
            }
        }
        true
    }

    /// The pane, or nothing with no clip picked.
    pub(super) fn pane(&self) -> Option<Element<'_, Message>> {
        let key = self.pane.key.as_ref()?;
        let name = key.split_once('/').map_or(key.as_str(), |(_, f)| f);
        let to = |m: PaneMsg| Message::Pane(m);
        let mut col = column![text(name.to_owned()).size(16)].spacing(10);
        let room = WIDTH - 24.0;
        match &self.pane.viewer {
            Some(v) => {
                let h = (room / v.aspect()).min(self.height * 0.45);
                let w = h * v.aspect();
                let picture: Element<'_, Message> = match v.picture() {
                    Some(p) => iced::widget::shader(p).width(w).height(h).into(),
                    None => container(text("…")).width(w).height(h).center_x(w).center_y(h).into(),
                };
                col = col.push(container(picture).center_x(Length::Fill));
                let marks = self.marks();
                let strip = Strip {
                    at: v.position(),
                    length: v.duration(),
                    marks: marks.iter().map(|m| (m.id.clone().unwrap_or_default(), m.t, if m.kind == MarkKind::Segment { m.end } else { None }, colour(m.role.as_deref()))).collect(),
                    picked: self.pane.picked.clone(),
                };
                col = col.push(canvas::Canvas::new(strip).width(Length::Fill).height(30));
                col = col.push(
                    row![
                        button(text(if v.playing() { "⏸" } else { "▶" }).size(14)).padding([2, 12]).on_press(to(PaneMsg::Toggle)),
                        text(format!("{} / {}", clock(v.position()), clock(v.duration()))).size(13).style(text::secondary),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                );
            }
            None => col = col.push(text(if self.pane.loading { "Opening…" } else { "" }).size(13).style(text::secondary)),
        }
        col = col.push(rule::horizontal(1));
        let mut marks = self.marks();
        marks.sort_by(|a, b| a.t.total_cmp(&b.t));
        for m in &marks {
            let id = m.id.clone().unwrap_or_default();
            let picked = self.pane.picked.as_deref() == Some(id.as_str());
            let span = match (m.kind, m.end) {
                (MarkKind::Segment, Some(e)) => format!("{} – {}", clock(m.t), clock(e)),
                (MarkKind::Flag, _) => format!("{} ⚑", clock(m.t)),
                _ => clock(m.t),
            };
            let c = colour(m.role.as_deref());
            let bar = container(space()).width(4).height(22).style(move |_| container::background(c));
            let role_name = m.role.as_deref().map(|r| ROLES.iter().find(|x| x.0 == r).map_or(r.to_owned(), |x| x.1.to_owned())).unwrap_or_default();
            let line = button(row![bar, text(span).size(13).width(150), text(role_name).size(13).width(170), text(m.note.clone()).size(13)].spacing(8).align_y(Alignment::Center))
                .width(Length::Fill)
                .padding([3, 6])
                .style(if picked { button::secondary } else { button::text })
                .on_press(to(PaneMsg::Pick(if picked { None } else { Some(id.clone()) })));
            col = col.push(line);
            if picked {
                let none = RoleChoice { key: String::new(), name: "—".to_owned() };
                let own = self.project.iter().flat_map(|p| p.sheet.roles.iter()).map(|r| RoleChoice { key: r.key.clone(), name: if r.en.is_empty() { r.title.clone() } else { format!("{} / {}", r.title, r.en) } });
                let choices: Vec<RoleChoice> = std::iter::once(none.clone()).chain(ROLES.iter().map(|r| RoleChoice { key: r.0.to_owned(), name: r.1.to_owned() })).chain(own).collect();
                let current = m.role.as_ref().and_then(|r| choices.iter().find(|c| &c.key == r).cloned()).unwrap_or(none);
                let role = pick_list(choices, Some(current), move |r: RoleChoice| to(PaneMsg::Role((!r.key.is_empty()).then_some(r.key)))).text_size(13);
                let note = text_input("what happens here", self.pane.note.as_deref().unwrap_or(&m.note)).size(13).on_input(move |t| to(PaneMsg::Note(t))).on_submit(to(PaneMsg::NoteSave));
                let remove = button(text("✕").size(13)).padding([2, 8]).style(button::danger).on_press(to(PaneMsg::Remove));
                col = col.push(row![role, note, remove].spacing(8).align_y(Alignment::Center));
            }
        }
        // Outlined while the keys go to it (Tab).
        let focused = self.pane.focused;
        Some(
            container(scrollable(col.padding(12)))
                .width(WIDTH)
                .height(Length::Fill)
                .style(move |t: &Theme| {
                    let mut s = container::rounded_box(t);
                    if focused {
                        s.border = iced::Border { color: t.extended_palette().primary.strong.color, width: 2.0, radius: s.border.radius };
                    }
                    s
                })
                .into(),
        )
    }
}

/// The strip under the player: the clip's length, its playhead, and its
/// segments and flags in a band at the top (a press there picks one).
pub struct Strip {
    pub at: f64,
    pub length: f64,
    /// Id, start, end (none for a point or a flag), colour.
    pub marks: Vec<(String, f64, Option<f64>, Color)>,
    pub picked: Option<String>,
}

/// The band's height, and how near a point a press picks it.
const BAND: f32 = 12.0;
const NEAR: f32 = 5.0;

impl Strip {
    /// The segment under `x`: the shortest there, so one inside another
    /// can be picked; a point within `NEAR`.
    fn mark_at(&self, x: f32, width: f32) -> Option<String> {
        let px = |t: f64| (t / self.length.max(1e-9)) as f32 * width;
        self.marks
            .iter()
            .filter(|(_, s, e, _)| match e {
                Some(e) => x >= px(*s) - 2.0 && x <= px(*e) + 2.0,
                None => (x - px(*s)).abs() <= NEAR,
            })
            .min_by(|a, b| a.2.map_or(0.0, |e| e - a.1).total_cmp(&b.2.map_or(0.0, |e| e - b.1)))
            .map(|m| m.0.clone())
    }
}

impl canvas::Program<Message> for Strip {
    type State = bool;

    fn update(&self, dragging: &mut bool, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let time = |x: f32| (((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64) * self.length;
        let to = |m: PaneMsg| canvas::Action::publish(Message::Pane(m)).and_capture();
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_over(bounds)?;
                if p.y - bounds.y < BAND {
                    return Some(to(PaneMsg::Pick(self.mark_at(p.x - bounds.x, bounds.width))));
                }
                *dragging = true;
                Some(to(PaneMsg::Scrub(time(p.x))))
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) if *dragging => Some(to(PaneMsg::Scrub(time(position.x)))),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if *dragging => {
                *dragging = false;
                Some(to(PaneMsg::ScrubDone))
            }
            _ => None,
        }
    }

    fn draw(&self, _: &bool, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let (w, h) = (bounds.width, bounds.height);
        let px = |t: f64| if self.length > 0.0 { (t / self.length).clamp(0.0, 1.0) as f32 * w } else { 0.0 };
        for (id, s, e, c) in &self.marks {
            let picked = self.picked.as_deref() == Some(id.as_str());
            let (x0, x1) = (px(*s), e.map_or(px(*s) + 3.0, px));
            let top = if picked { 0.0 } else { 3.0 };
            frame.fill(&Path::rectangle(Point::new(x0, top), Size::new((x1 - x0).max(3.0), BAND - 2.0 - top)), *c);
            if picked {
                frame.fill(&Path::rectangle(Point::new(x0, BAND), Size::new((x1 - x0).max(3.0), h - BAND)), Color { a: 0.25, ..*c });
            }
        }
        let bar = (h - BAND) * 0.4;
        let top = BAND + (h - BAND - bar) / 2.0;
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(w, bar), (bar / 2.0).into()), palette.background.strong.color);
        let x = px(self.at);
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(x.max(bar), bar), (bar / 2.0).into()), palette.primary.base.color);
        frame.fill(&Path::rectangle(Point::new((x - 1.0).clamp(0.0, w - 2.0), BAND), Size::new(2.0, h - BAND)), Color::WHITE);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, dragging: &bool, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if *dragging || cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}
