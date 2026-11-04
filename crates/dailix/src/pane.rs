//! The picked clip at the right of the tiles: Omnix's player (`viewer`),
//! a strip with the clip's segments in their roles' colours, and the
//! segments as a list, the picked one's role and note editable. Every
//! change to the marking goes through the sheet's core, by the mark's id.
//!
//! Keys (the window's, read by their place so any layout works): Space
//! plays; I and O make a segment (or, inside the picked one, move its
//! ends); F sets a flag; Page Up / Down go to the mark before or after;
//! Delete removes the picked one. A mark just made (F, or O after I) asks
//! at once for its role (↑ ↓, Enter) and its note (Enter); Esc or Tab
//! before that takes it away again (user, 2026-11-05: roles had a key each,
//! which is hard to learn).

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

/// The roles: key, the sheet's name, its colour.
pub const ROLES: [(&str, &str, Color); 8] = [
    ("talk", "розмова / sync", Color::from_rgb8(0xc0, 0x56, 0x3a)),
    ("establish", "загальний / establishing", Color::from_rgb8(0x4f, 0x8a, 0x6a)),
    ("broll", "фон / B-roll", Color::from_rgb8(0x6b, 0x7f, 0x9e)),
    ("detail", "деталь / insert", Color::from_rgb8(0xc9, 0x8a, 0x5e)),
    ("cut", "перебивка / cutaway", Color::from_rgb8(0xb0, 0x7a, 0xa8)),
    ("action", "дія / action", Color::from_rgb8(0xd2, 0x9a, 0x5e)),
    ("reveal", "ревіл / reveal", Color::from_rgb8(0x7a, 0x6b, 0xab)),
    ("reaction", "реакція / reaction", Color::from_rgb8(0xd1, 0x8f, 0xa5)),
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

/// The pane's state: the clip in the player, the segment picked, an in
/// point (I) waiting for its O, the picked segment's note as typed, and
/// whether the keys go to the player (Tab) rather than the tiles.
#[derive(Default)]
pub struct Pane {
    pub key: Option<String>,
    pub viewer: Option<Viewer>,
    pub loading: bool,
    pub picked: Option<String>,
    pub mark_in: Option<f64>,
    pub note: Option<String>,
    pub focused: bool,
    /// A mark just made, being given its role, then its note; Esc or Tab
    /// before the last Enter takes it away again.
    pub pending: Option<Pending>,
}

/// A new mark's role (the highlighted choice) or note (as typed).
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Role(usize),
    Note(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub id: String,
    pub stage: Stage,
}

/// The new mark's note field, focused when its role is chosen.
const NOTE: &str = "new-mark-note";

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
    /// The new mark's role chosen by a click (its place in the choices).
    PendingRole(usize),
    PendingNote(String),
    PendingDone,
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
            PaneMsg::PendingRole(i) => return self.pending_role(i),
            PaneMsg::PendingNote(text) => {
                if let Some(p) = &mut self.pane.pending {
                    p.stage = Stage::Note(text);
                }
            }
            PaneMsg::PendingDone => {
                if let Some(Pending { id, stage: Stage::Note(note) }) = self.pane.pending.take() {
                    if !note.trim().is_empty() {
                        self.change(Op::Mark(MarkOp::Note { mark: id, note: note.trim().to_owned() }));
                    }
                    // Done with: I, O and F start the next.
                    self.pane.picked = None;
                }
            }
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
                self.pane.mark_in = None;
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

    /// The roles to choose from: none first, the sheet's eight, the user's.
    fn role_choices(&self) -> Vec<RoleChoice> {
        let none = RoleChoice { key: String::new(), name: "— без ролі".to_owned() };
        let own = self.project.iter().flat_map(|p| p.sheet.roles.iter()).map(|r| RoleChoice { key: r.key.clone(), name: if r.en.is_empty() { r.title.clone() } else { format!("{} / {}", r.title, r.en) } });
        std::iter::once(none).chain(ROLES.iter().map(|r| RoleChoice { key: r.0.to_owned(), name: r.1.to_owned() })).chain(own).collect()
    }

    /// A mark just made: its role is asked for at once.
    fn begin_pending(&mut self, made: Option<String>) {
        self.pane.picked = made.clone();
        self.pane.pending = made.map(|id| Pending { id, stage: Stage::Role(0) });
    }

    /// The new mark's role given (none for the first choice); its note asked
    /// for next, the field focused.
    fn pending_role(&mut self, i: usize) -> iced::Task<Message> {
        let Some(p) = self.pane.pending.clone() else { return iced::Task::none() };
        let role = self.role_choices().get(i).map(|c| c.key.clone()).filter(|k| !k.is_empty());
        if role.is_some() {
            self.change(Op::Mark(MarkOp::Role { mark: p.id.clone(), role }));
        }
        self.pane.pending = Some(Pending { id: p.id, stage: Stage::Note(String::new()) });
        iced::widget::operation::focus(NOTE)
    }

    /// Esc or Tab before the last Enter: the new mark taken away.
    pub(super) fn cancel_pending(&mut self) {
        if let Some(p) = self.pane.pending.take() {
            self.change(Op::Mark(MarkOp::Remove { mark: p.id }));
            self.pane.picked = None;
        }
    }

    /// Keys while a new mark is being described: ↑ ↓ and Enter choose its
    /// role (the note's field takes its own keys).
    pub(super) fn pending_key(&mut self, key: &iced::keyboard::Key) -> iced::Task<Message> {
        use iced::keyboard::{Key, key::Named};
        let Some(Pending { stage: Stage::Role(i), id }) = self.pane.pending.clone() else { return iced::Task::none() };
        let n = self.role_choices().len();
        match key.as_ref() {
            Key::Named(Named::ArrowDown) => self.pane.pending = Some(Pending { id, stage: Stage::Role((i + 1).min(n - 1)) }),
            Key::Named(Named::ArrowUp) => self.pane.pending = Some(Pending { id, stage: Stage::Role(i.saturating_sub(1)) }),
            Key::Named(Named::Enter) => return self.pending_role(i),
            _ => {}
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
            // I inside the picked segment moves its start; anywhere else it
            // sets an in point for O. Never stretches a segment the
            // playhead isn't in (it did: one finished further on grew back
            // to an I set earlier in the clip).
            Code::KeyI => {
                let Some(now) = now else { return true };
                match picked {
                    Some(m) if m.kind == MarkKind::Segment && m.end.is_some_and(|e| now >= m.t && now < e) => {
                        self.change(Op::Mark(MarkOp::Move { mark: m.id.clone().unwrap_or_default(), t: now, end: m.end }));
                    }
                    _ => {
                        self.pane.picked = None;
                        self.pane.mark_in = Some(now);
                    }
                }
            }
            // O inside the picked segment moves its end; after an I, a
            // segment from there; on a flag picked, the flag becomes a
            // segment to here (its role and note kept). Else nothing: no
            // start.
            Code::KeyO => {
                let Some(now) = now else { return true };
                let Some(clip) = self.pane.key.clone() else { return true };
                match (picked, self.pane.mark_in) {
                    (Some(m), _) if m.kind == MarkKind::Segment && m.end.is_some_and(|e| now > m.t && now <= e) => {
                        self.change(Op::Mark(MarkOp::Move { mark: m.id.clone().unwrap_or_default(), t: m.t, end: Some(now) }));
                    }
                    (_, Some(start)) => {
                        self.pane.mark_in = None;
                        let made = self.change(Op::Mark(MarkOp::Add { clip, t: start.min(now), end: Some(start.max(now)), flag: false, role: None, note: String::new() }));
                        self.begin_pending(made);
                    }
                    (Some(f), None) if f.kind == MarkKind::Flag && now > f.t => {
                        let add = Op::Mark(MarkOp::Add { clip, t: f.t, end: Some(now), flag: false, role: f.role.clone(), note: f.note.clone() });
                        self.pane.picked = self.change_all(vec![Op::Mark(MarkOp::Remove { mark: f.id.clone().unwrap_or_default() }), add]);
                    }
                    _ => self.error = Some("No start for a segment: set one with I first".to_owned()),
                }
            }
            // F: a flag here, a note to self (a meme to find, a sound).
            Code::KeyF => {
                let (Some(now), Some(clip)) = (now, self.pane.key.clone()) else { return true };
                self.pane.mark_in = None;
                let made = self.change(Op::Mark(MarkOp::Add { clip, t: now, end: None, flag: true, role: None, note: String::new() }));
                self.begin_pending(made);
            }
            // Page Up / Down: to the mark before or after, picked.
            Code::PageUp | Code::PageDown => {
                let mut marks = self.marks();
                marks.sort_by(|a, b| a.t.total_cmp(&b.t));
                let from = picked.map_or(now.unwrap_or(0.0), |m| m.t);
                let next = if code == Code::PageDown { marks.into_iter().find(|m| m.t > from + 1e-3) } else { marks.into_iter().rev().find(|m| m.t < from - 1e-3) };
                if let Some(m) = next {
                    self.save_note();
                    self.pane.picked = m.id.clone();
                    self.pane.mark_in = None;
                    if let Some(v) = &mut self.pane.viewer {
                        v.jump(m.t);
                    }
                }
            }
            Code::Delete | Code::Backspace => {
                if let Some(mark) = self.pane.picked.take() {
                    self.change(Op::Mark(MarkOp::Remove { mark }));
                }
            }
            _ => return false,
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
                let h = (room / v.aspect()).min(self.height / self.scale * 0.45);
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
                    mark_in: self.pane.mark_in,
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
        // A new mark being described, below the list, out of its scroll.
        let pending: Option<Element<'_, Message>> = self.pane.pending.as_ref().map(|p| {
            let what = self.marks().into_iter().find(|m| m.id.as_deref() == Some(p.id.as_str())).map(|m| match (m.kind, m.end) {
                (MarkKind::Segment, Some(e)) => format!("New segment {} – {}", clock(m.t), clock(e)),
                _ => format!("New flag {}", clock(m.t)),
            });
            let mut b = column![text(what.unwrap_or_default()).size(14)].spacing(4);
            match &p.stage {
                Stage::Role(at) => {
                    for (i, c) in self.role_choices().into_iter().enumerate() {
                        let colour = colour(Some(c.key.as_str()).filter(|k| !k.is_empty()));
                        let bar = container(space()).width(4).height(18).style(move |_| container::background(colour));
                        b = b.push(
                            button(row![bar, text(c.name).size(13)].spacing(8).align_y(Alignment::Center))
                                .width(Length::Fill)
                                .padding([2, 6])
                                .style(if i == *at { button::primary } else { button::text })
                                .on_press(to(PaneMsg::PendingRole(i))),
                        );
                    }
                }
                Stage::Note(note) => {
                    b = b.push(text_input("what happens here", note).id(NOTE).size(14).on_input(move |t| to(PaneMsg::PendingNote(t))).on_submit(to(PaneMsg::PendingDone)));
                }
            }
            container(b.padding(10)).style(container::bordered_box).into()
        });
        // Outlined while the keys go to it (Tab).
        let focused = self.pane.focused;
        let body = column![scrollable(col.padding(12)).height(Length::Fill)].push(pending).spacing(6);
        Some(
            container(body)
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
    /// An in point (I) waiting for its O, drawn as a little flag.
    pub mark_in: Option<f64>,
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
        if let Some(t) = self.mark_in {
            let x = px(t);
            frame.fill(
                &Path::new(|b| {
                    b.move_to(Point::new(x, 0.0));
                    b.line_to(Point::new(x + 7.0, 0.0));
                    b.line_to(Point::new(x, BAND));
                    b.close();
                }),
                palette.primary.strong.color,
            );
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
