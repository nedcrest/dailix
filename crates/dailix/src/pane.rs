//! The picked clip at the right of the tiles, top to bottom (user's sketch,
//! 2026-11-05): Omnix's player (`viewer`) and a strip with the clip's marks
//! in their roles' colours; the marks as a list; an editing plane, the
//! bottom quarter, for the mark picked or just made; the clip's facts.
//! Every change to the marking goes through the sheet's core, by the
//! mark's id. The pane's width is dragged at its edge (to half the
//! window).
//!
//! Keys (the window's, read by their place so any layout works): Space
//! plays; I and O make a segment (or, inside the picked one, move its
//! ends); M sets a flag, a note to self that never goes into the edit
//! (M as Omnix's markers); N a moment, a segment a second either side (user, 2026-11-05: a flag
//! outside every segment adds nothing, so a quick way to add material);
//! Page Up / Down go to the mark before or after;
//! Delete removes the picked one. A mark just made (M, N, or O after I) asks
//! at once for its role (↑ ↓, Enter) and its note (Enter); Esc or Tab
//! before that takes it away again (user, 2026-11-05: roles had a key each,
//! which is hard to learn). On the strip a segment's edges and a flag are
//! dragged.

use super::{App, Message};
use dailix_sheet::{MarkKind, MarkOp, Op};
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::{button, column, container, pick_list, row, rule, scrollable, space, text, text_editor};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};
use rosmo360::viewer::{Loaded, Viewer};
use std::sync::{Arc, Mutex};

/// The pane's share of the window's width at first, and the least width
/// it keeps.
pub const SHARE: f32 = 0.4;
pub const MIN_WIDTH: f32 = 380.0;
/// How long the pick rests on a clip before the player opens it.
const REST: std::time::Duration = std::time::Duration::from_millis(250);

/// A clip loaded off the UI thread, handed over once.
#[derive(Clone)]
pub struct Slot(pub Arc<Mutex<Option<Result<Loaded, String>>>>);

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Slot")
    }
}

/// The pane's state: the clip in the player and its facts, the mark
/// picked, an in point (I) waiting for its O, a mark just made being
/// described, the note's field and the mark it was filled from, and
/// whether the keys go to the player (Tab) rather than the tiles.
#[derive(Default)]
pub struct Pane {
    pub key: Option<String>,
    pub viewer: Option<Viewer>,
    pub loading: bool,
    pub facts: Option<Result<Vec<(String, String)>, String>>,
    pub picked: Option<String>,
    pub mark_in: Option<f64>,
    pub pending: Option<Pending>,
    pub editor: text_editor::Content,
    editor_for: Option<String>,
    pub focused: bool,
}

/// A new mark's stage: its role (the highlighted choice), then its note.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Role(usize),
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub id: String,
    pub stage: Stage,
}

/// The note's field, focused when a new mark's role is chosen.
const NOTE: &str = "mark-note";

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

/// The pane's messages.
#[derive(Debug, Clone)]
pub enum PaneMsg {
    /// The pick rested on this clip: open it.
    Rested(String),
    Loaded(String, Slot),
    Facts(String, Result<Vec<(String, String)>, String>),
    Tick,
    Toggle,
    Scrub(f64),
    ScrubDone,
    Pick(Option<String>),
    /// A mark dragged on the strip to here.
    Moved(String, f64, Option<f64>),
    Role(Option<String>),
    /// The note's field typed in, and Enter in it.
    Edit(text_editor::Action),
    Enter,
    Remove,
    /// The new mark's role chosen by a click (its place in the choices).
    PendingRole(usize),
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

    /// Opens the clip `key` in the player, and reads its facts, off the UI
    /// thread, if it's still the one in the pane.
    fn load_clip(&mut self, key: String) -> iced::Task<Message> {
        if self.pane.key.as_deref() != Some(key.as_str()) || self.pane.viewer.is_some() {
            return iced::Task::none();
        }
        let Some(path) = self.project.as_ref().map(|p| p.footage.layout.clip(&key)) else { return iced::Task::none() };
        let (k, p) = (key.clone(), path.clone());
        let facts = iced::Task::perform(super::blocking(move || rosmo360::viewer::describe(&p).map_err(|e| format!("{e:#}"))), move |r| Message::Pane(PaneMsg::Facts(k.clone(), r)));
        let load = iced::Task::perform(super::blocking(move || rosmo360::viewer::load(&path).map_err(|e| format!("{e:#}"))), move |r| Message::Pane(PaneMsg::Loaded(key.clone(), Slot(Arc::new(Mutex::new(Some(r)))))));
        iced::Task::batch([load, facts])
    }

    pub(super) fn pane_busy(&self) -> bool {
        self.pane.viewer.as_ref().is_some_and(Viewer::busy)
    }

    /// The marks on the clip in the pane, as the sheet holds them.
    fn marks(&self) -> Vec<dailix_sheet::Mark> {
        let (Some(p), Some(key)) = (&self.project, &self.pane.key) else { return Vec::new() };
        p.sheet.clips.get(key).map(|c| c.marks.clone()).unwrap_or_default()
    }

    fn mark(&self, id: &str) -> Option<dailix_sheet::Mark> {
        self.marks().into_iter().find(|m| m.id.as_deref() == Some(id))
    }

    /// The note's field as typed, one line as the sheet keeps it.
    fn typed(&self) -> String {
        self.pane.editor.text().replace('\n', " ").trim().to_owned()
    }

    /// Saves the note typed for the mark the field was filled from, if it
    /// changed (also on leaving the contact sheet).
    pub(super) fn save_note(&mut self) {
        let Some(id) = self.pane.editor_for.clone() else { return };
        let typed = self.typed();
        if self.mark(&id).is_some_and(|m| m.note != typed) {
            self.change(Op::Mark(MarkOp::Note { mark: id, note: typed }));
        }
    }

    /// Fills the note's field from the mark being edited (the new one, or
    /// the one picked) when that mark changes.
    fn sync_editor(&mut self) {
        let now = self.pane.pending.as_ref().map(|p| p.id.clone()).or_else(|| self.pane.picked.clone());
        if now != self.pane.editor_for {
            let note = now.as_deref().and_then(|id| self.mark(id)).map(|m| m.note).unwrap_or_default();
            self.pane.editor = text_editor::Content::with_text(&note);
            self.pane.editor_for = now;
        }
    }

    pub(super) fn on_pane(&mut self, msg: PaneMsg) -> iced::Task<Message> {
        let task = self.pane_msg(msg);
        self.sync_editor();
        task
    }

    fn pane_msg(&mut self, msg: PaneMsg) -> iced::Task<Message> {
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
            PaneMsg::Facts(key, facts) => {
                if self.pane.key.as_deref() == Some(key.as_str()) {
                    self.pane.facts = Some(facts);
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
                if self.pane.pending.is_some() {
                    return iced::Task::none();
                }
                self.save_note();
                self.pane.picked = id.clone();
                self.pane.mark_in = None;
                if let Some(start) = id.and_then(|id| self.mark(&id)).map(|m| m.t)
                    && let Some(v) = &mut self.pane.viewer
                {
                    v.jump(start);
                }
            }
            PaneMsg::Moved(id, t, end) => {
                self.change(Op::Mark(MarkOp::Move { mark: id, t, end }));
            }
            PaneMsg::Role(role) => {
                if let Some(mark) = self.pane.picked.clone() {
                    self.change(Op::Mark(MarkOp::Role { mark, role }));
                }
            }
            PaneMsg::Edit(action) => self.pane.editor.perform(action),
            // Enter: the note saved; the mark is done with, so I, O and F
            // start the next.
            PaneMsg::Enter => {
                self.save_note();
                self.pane.pending = None;
                self.pane.picked = None;
            }
            PaneMsg::Remove => {
                if let Some(mark) = self.pane.picked.take() {
                    self.pane.pending = None;
                    self.change(Op::Mark(MarkOp::Remove { mark }));
                }
            }
            PaneMsg::PendingRole(i) => return self.pending_role(i),
        }
        iced::Task::none()
    }

    /// The roles to choose from: none first, the sheet's eight, the user's.
    fn role_choices(&self) -> Vec<RoleChoice> {
        let none = RoleChoice { key: String::new(), name: "— без ролі".to_owned() };
        let roles = self.project.iter().flat_map(|p| p.sheet.all_roles()).map(|r| RoleChoice { name: r.name(), key: r.key });
        std::iter::once(none).chain(roles).collect()
    }

    /// A mark's colour by its role, the user's own roles' included.
    fn role_colour(&self, role: Option<&str>) -> Color {
        let [r, g, b] = self.project.as_ref().map_or(dailix_sheet::NO_ROLE_RGB, |p| p.sheet.role_rgb(role));
        Color::from_rgb8(r, g, b)
    }

    /// A role as it reads ("розмова / sync"); one the sheet doesn't know,
    /// its key.
    fn role_name(&self, role: Option<&str>) -> String {
        role.map(|r| self.project.as_ref().and_then(|p| p.sheet.role(r)).map_or(r.to_owned(), |x| x.name())).unwrap_or_default()
    }

    /// Where `role` is among the choices (none: the first).
    fn role_choices_index(&self, role: &str) -> usize {
        self.role_choices().iter().position(|c| c.key == role).unwrap_or(0)
    }

    /// A mark just made: its role is asked for at once.
    fn begin_pending(&mut self, made: Option<String>) {
        self.save_note();
        self.pane.picked = made.clone();
        self.pane.pending = made.map(|id| Pending { id, stage: Stage::Role(0) });
        self.sync_editor();
    }

    /// The new mark's role given (none for the first choice); its note asked
    /// for next, the field focused.
    fn pending_role(&mut self, i: usize) -> iced::Task<Message> {
        let Some(p) = self.pane.pending.clone() else { return iced::Task::none() };
        let role = self.role_choices().get(i).map(|c| c.key.clone()).filter(|k| !k.is_empty());
        if role.is_some() {
            self.change(Op::Mark(MarkOp::Role { mark: p.id.clone(), role }));
        }
        self.pane.pending = Some(Pending { id: p.id, stage: Stage::Note });
        iced::widget::operation::focus(NOTE)
    }

    /// Esc or Tab before the last Enter: the new mark taken away.
    pub(super) fn cancel_pending(&mut self) {
        if let Some(p) = self.pane.pending.take() {
            self.change(Op::Mark(MarkOp::Remove { mark: p.id }));
            self.pane.picked = None;
            self.pane.editor_for = None;
            self.sync_editor();
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
        let picked = self.pane.picked.clone().and_then(|id| self.mark(&id));
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
                        self.save_note();
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
            // M: a flag here, a note to self (a meme to find, a sound); M
            // as Omnix's marker key, and the trade's (user, 2026-11-05).
            Code::KeyM => {
                let (Some(now), Some(clip)) = (now, self.pane.key.clone()) else { return true };
                self.pane.mark_in = None;
                let made = self.change(Op::Mark(MarkOp::Add { clip, t: now, end: None, flag: true, role: None, note: String::new() }));
                self.begin_pending(made);
            }
            // N: a moment, a segment a second either side of the playhead,
            // to stretch by its edges: material, where a flag is a note.
            // On a flag (picked, or at the playhead), the moment takes its
            // role, offered first (user, 2026-11-05).
            Code::KeyN => {
                let (Some(now), Some(clip)) = (now, self.pane.key.clone()) else { return true };
                let length = self.pane.viewer.as_ref().map_or(f64::INFINITY, Viewer::duration);
                let frame = 0.15;
                let flag = picked.filter(|m| m.kind == MarkKind::Flag).or_else(|| self.marks().into_iter().find(|m| m.kind == MarkKind::Flag && (m.t - now).abs() <= frame));
                let role = flag.and_then(|f| f.role);
                self.pane.mark_in = None;
                let made = self.change(Op::Mark(MarkOp::Add { clip, t: (now - 1.0).max(0.0), end: Some((now + 1.0).min(length)), flag: false, role: role.clone(), note: String::new() }));
                self.begin_pending(made);
                if let Some(role) = role {
                    let at = self.role_choices_index(&role);
                    if let Some(p) = &mut self.pane.pending {
                        p.stage = Stage::Role(at);
                    }
                }
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
        self.sync_editor();
        true
    }

    /// The pane, or nothing with no clip picked.
    pub(super) fn pane(&self) -> Option<Element<'_, Message>> {
        let key = self.pane.key.as_ref()?;
        let name = key.split_once('/').map_or(key.as_str(), |(_, f)| f);
        let to = |m: PaneMsg| Message::Pane(m);
        let room = self.pane_width() - 24.0;
        let tall = self.height / self.scale;
        let mut top = column![text(name.to_owned()).size(16)].spacing(8);
        match &self.pane.viewer {
            Some(v) => {
                let h = (room / v.aspect()).min(tall * 0.42);
                let w = h * v.aspect();
                let picture: Element<'_, Message> = match v.picture() {
                    Some(p) => iced::widget::shader(p).width(w).height(h).into(),
                    None => container(text("…")).width(w).height(h).center_x(w).center_y(h).into(),
                };
                top = top.push(container(picture).center_x(Length::Fill));
                let strip = Strip {
                    at: v.position(),
                    length: v.duration(),
                    marks: self.marks().iter().map(|m| (m.id.clone().unwrap_or_default(), m.t, if m.kind == MarkKind::Segment { m.end } else { None }, self.role_colour(m.role.as_deref()))).collect(),
                    picked: self.pane.picked.clone(),
                    mark_in: self.pane.mark_in,
                };
                top = top.push(canvas::Canvas::new(strip).width(Length::Fill).height(36));
                top = top.push(
                    row![
                        button(text(if v.playing() { "⏸" } else { "▶" }).size(14)).padding([2, 12]).on_press(to(PaneMsg::Toggle)),
                        text(format!("{} / {}", dailix_sheet::clock(v.position(), 1), dailix_sheet::clock(v.duration(), 1))).size(13).style(text::secondary),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                );
            }
            None => top = top.push(text(if self.pane.loading { "Opening…" } else { "" }).size(13).style(text::secondary)),
        }

        // The marks, by time.
        let mut list = column![].spacing(2);
        let mut marks = self.marks();
        marks.sort_by(|a, b| a.t.total_cmp(&b.t));
        for m in &marks {
            let id = m.id.clone().unwrap_or_default();
            let picked = self.pane.picked.as_deref() == Some(id.as_str());
            let c = self.role_colour(m.role.as_deref());
            let bar = container(space()).width(4).height(Length::Fill).style(move |_| container::background(c));
            // The note under the time and role: it reads at any width.
            let outside = m.kind != MarkKind::Segment && !dailix_sheet::in_segment(&marks, m.t);
            let when = text(span(m)).size(13).width(140);
            let when = if outside { when.style(text::secondary) } else { when };
            let mut body = column![row![when, text(self.role_name(m.role.as_deref())).size(13)].spacing(8)].spacing(2);
            if !m.note.is_empty() {
                body = body.push(text(m.note.clone()).size(13).style(text::secondary));
            }
            let line = button(row![bar, body].spacing(8).height(Length::Shrink))
                .width(Length::Fill)
                .padding([3, 6])
                .style(if picked { button::secondary } else { button::text })
                .on_press(to(PaneMsg::Pick(if picked { None } else { Some(id.clone()) })));
            list = list.push(line);
        }

        let column = column![
            container(top).padding(12),
            rule::horizontal(1),
            scrollable(list.padding([6, 12])).height(Length::Fill),
            container(self.plane()).height(tall * 0.25).width(Length::Fill).padding(10).style(container::bordered_box),
            container(self.facts()).width(Length::Fill).padding(10),
        ]
        .spacing(6);
        // Outlined while the keys go to it (Tab).
        let focused = self.pane.focused;
        Some(
            container(column)
                .width(self.pane_width())
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

    /// The editing plane: a new mark's role then note, or the picked
    /// mark's role, note and delete; empty with neither.
    fn plane(&self) -> Element<'_, Message> {
        let to = |m: PaneMsg| Message::Pane(m);
        let id = self.pane.pending.as_ref().map(|p| p.id.clone()).or_else(|| self.pane.picked.clone());
        let Some(m) = id.as_deref().and_then(|id| self.mark(id)) else { return space().into() };
        let title = match (self.pane.pending.is_some(), m.kind) {
            (true, MarkKind::Flag) => format!("New flag {}", span(&m)),
            (true, _) => format!("New segment {}", span(&m)),
            (false, MarkKind::Flag) => format!("Flag {}", span(&m)),
            (false, _) => format!("Segment {}", span(&m)),
        };
        let note = text_editor(&self.pane.editor)
            .id(NOTE)
            .placeholder("what happens here")
            .size(14)
            .wrapping(text::Wrapping::WordOrGlyph)
            .on_action(move |a| to(PaneMsg::Edit(a)))
            .key_binding(|press| match press.key.as_ref() {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => Some(text_editor::Binding::Custom(Message::Pane(PaneMsg::Enter))),
                _ => text_editor::Binding::from_key_press(press),
            });
        let mut b = column![text(title).size(14)].spacing(6);
        match self.pane.pending.as_ref().map(|p| &p.stage) {
            // A new mark: the roles unfolded, ↑ ↓ Enter or a click.
            Some(Stage::Role(at)) => {
                let mut roles = column![].spacing(1);
                for (i, c) in self.role_choices().into_iter().enumerate() {
                    let colour = self.role_colour(Some(c.key.as_str()).filter(|k| !k.is_empty()));
                    let bar = container(space()).width(4).height(18).style(move |_| container::background(colour));
                    roles = roles.push(
                        button(row![bar, text(c.name).size(13)].spacing(8).align_y(Alignment::Center))
                            .width(Length::Fill)
                            .padding([2, 6])
                            .style(if i == *at { button::primary } else { button::text })
                            .on_press(to(PaneMsg::PendingRole(i))),
                    );
                }
                b = b.push(scrollable(roles).height(Length::Fill));
            }
            Some(Stage::Note) => b = b.push(scrollable(note).height(Length::Fill)),
            // A mark picked: its role, its note, delete.
            None => {
                let choices = self.role_choices();
                let current = m.role.as_ref().and_then(|r| choices.iter().find(|c| &c.key == r).cloned()).unwrap_or_else(|| choices[0].clone());
                let role = pick_list(choices, Some(current), move |r: RoleChoice| to(PaneMsg::Role((!r.key.is_empty()).then_some(r.key)))).text_size(13);
                let remove = button(text("✕").size(13)).padding([2, 8]).style(button::danger).on_press(to(PaneMsg::Remove));
                b = b.push(row![role, space().width(Length::Fill), remove].spacing(8).align_y(Alignment::Center));
                b = b.push(scrollable(note).height(Length::Fill));
            }
        }
        b.into()
    }

    /// The clip's facts, as Omnix's Info shows them.
    fn facts(&self) -> Element<'_, Message> {
        match &self.pane.facts {
            None => text("…").size(12).style(text::secondary).into(),
            Some(Err(e)) => text(e.clone()).size(12).style(text::danger).into(),
            Some(Ok(rows)) => column(rows.iter().map(|(k, v)| row![text(k.clone()).size(12).style(text::secondary).width(90), text(v.clone()).size(12)].spacing(8).into())).spacing(2).into(),
        }
    }
}

/// A mark's place: a segment's span, a flag's time and ⚑.
fn span(m: &dailix_sheet::Mark) -> String {
    match (m.kind, m.end) {
        (MarkKind::Segment, Some(e)) => format!("{} – {}", dailix_sheet::clock(m.t, 1), dailix_sheet::clock(e, 1)),
        (MarkKind::Flag, _) => format!("{} ⚑", dailix_sheet::clock(m.t, 1)),
        _ => dailix_sheet::clock(m.t, 1),
    }
}

/// The strip under the player: the clip's length, its playhead, and its
/// marks: segments as bars in a band at the top, flags and points as
/// poles the strip's full height (seen over a segment). A press in the
/// band picks a mark; a segment's edge or a pole is dragged; below the
/// band a drag scrubs.
pub struct Strip {
    pub at: f64,
    pub length: f64,
    /// Id, start, end (none for a flag or a point), colour.
    pub marks: Vec<(String, f64, Option<f64>, Color)>,
    pub picked: Option<String>,
    /// An in point (I) waiting for its O.
    pub mark_in: Option<f64>,
}

/// The band's height; how near an edge or a pole a press takes it.
const BAND: f32 = 14.0;
const NEAR: f32 = 5.0;

/// What a drag on the strip is doing.
#[derive(Default)]
pub enum Drag {
    #[default]
    None,
    Scrub,
    /// A mark's edge or pole: the mark, which part, where it is now.
    Mark { id: String, part: Part, t: f64, moved: bool },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    Start,
    End,
    Pole,
}

impl Strip {
    fn px(&self, t: f64, width: f32) -> f32 {
        (t / self.length.max(1e-9)).clamp(0.0, 1.0) as f32 * width
    }

    /// The edge or pole under `x`, if any.
    fn handle_at(&self, x: f32, width: f32) -> Option<(String, Part)> {
        self.marks.iter().find_map(|(id, s, e, _)| match e {
            Some(_) if (x - self.px(*s, width)).abs() <= NEAR => Some((id.clone(), Part::Start)),
            Some(e) if (x - self.px(*e, width)).abs() <= NEAR => Some((id.clone(), Part::End)),
            None if (x - self.px(*s, width)).abs() <= NEAR => Some((id.clone(), Part::Pole)),
            _ => None,
        })
    }

    /// The segment under `x`: the shortest there, so one inside another
    /// can be picked.
    fn mark_at(&self, x: f32, width: f32) -> Option<String> {
        self.marks
            .iter()
            .filter(|(_, s, e, _)| e.is_some_and(|e| x >= self.px(*s, width) && x <= self.px(e, width)))
            .min_by(|a, b| a.2.map_or(0.0, |e| e - a.1).total_cmp(&b.2.map_or(0.0, |e| e - b.1)))
            .map(|m| m.0.clone())
    }

    /// A mark's start and end as drawn: as the sheet has it, or where the
    /// drag holds it.
    fn shown(&self, id: &str, s: f64, e: Option<f64>, drag: &Drag) -> (f64, Option<f64>) {
        match drag {
            Drag::Mark { id: d, part, t, .. } if d == id => match part {
                Part::Start => (t.min(e.unwrap_or(*t) - dailix_sheet::ops::MIN_SEGMENT), e),
                Part::End => (s, Some(t.max(s + dailix_sheet::ops::MIN_SEGMENT))),
                Part::Pole => (*t, None),
            },
            _ => (s, e),
        }
    }
}

impl canvas::Program<Message> for Strip {
    type State = Drag;

    fn update(&self, drag: &mut Drag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let time = |x: f32| (((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64) * self.length;
        let to = |m: PaneMsg| canvas::Action::publish(Message::Pane(m)).and_capture();
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_over(bounds)?;
                let x = p.x - bounds.x;
                // An edge or a pole: picked, and dragged.
                if let Some((id, part)) = self.handle_at(x, bounds.width) {
                    let t = self.marks.iter().find(|m| m.0 == id).map_or(0.0, |m| if part == Part::End { m.2.unwrap_or(m.1) } else { m.1 });
                    *drag = Drag::Mark { id: id.clone(), part, t, moved: false };
                    return Some(to(PaneMsg::Pick(Some(id))));
                }
                if p.y - bounds.y < BAND {
                    return Some(to(PaneMsg::Pick(self.mark_at(x, bounds.width))));
                }
                *drag = Drag::Scrub;
                Some(to(PaneMsg::Scrub(time(p.x))))
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => match drag {
                Drag::Scrub => Some(to(PaneMsg::Scrub(time(position.x)))),
                Drag::Mark { t, moved, .. } => {
                    *t = time(position.x);
                    *moved = true;
                    Some(canvas::Action::request_redraw().and_capture())
                }
                Drag::None => None,
            },
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let done = std::mem::take(drag);
                match &done {
                    Drag::Scrub => Some(to(PaneMsg::ScrubDone)),
                    // Moved: the mark where the drag left it, one change.
                    Drag::Mark { id, moved: true, .. } => {
                        let (s, e) = self.marks.iter().find(|m| &m.0 == id).map(|m| (m.1, m.2)).unwrap_or_default();
                        let (t, end) = self.shown(id, s, e, &done);
                        Some(to(PaneMsg::Moved(id.clone(), t, end)))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn draw(&self, drag: &Drag, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let (w, h) = (bounds.width, bounds.height);
        let px = |t: f64| self.px(t, w);
        // The bar and the playhead first: the marks are drawn over them.
        let bar = (h - BAND) * 0.4;
        let top = BAND + (h - BAND - bar) / 2.0;
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(w, bar), (bar / 2.0).into()), palette.background.strong.color);
        let x = px(self.at);
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(x.max(bar), bar), (bar / 2.0).into()), palette.primary.base.color);
        // Segments first, then the poles over them: a flag inside a segment
        // was hidden under its bar.
        let mut marks: Vec<_> = self.marks.iter().collect();
        marks.sort_by_key(|m| m.2.is_none());
        for (id, s, e, c) in marks {
            let picked = self.picked.as_deref() == Some(id.as_str());
            let (s, e) = self.shown(id, *s, *e, drag);
            match e {
                Some(e) => {
                    let (x0, x1) = (px(s), px(e));
                    let top = if picked { 0.0 } else { 3.0 };
                    frame.fill(&Path::rectangle(Point::new(x0, top), Size::new((x1 - x0).max(3.0), BAND - 2.0 - top)), *c);
                    if picked {
                        frame.fill(&Path::rectangle(Point::new(x0, BAND), Size::new((x1 - x0).max(3.0), h - BAND)), Color { a: 0.25, ..*c });
                    }
                }
                // A flag or a point: a pole the strip's height and a little
                // flag at its top, so it shows over a segment too.
                None => {
                    let x = px(s);
                    let edge = palette.background.base.color;
                    // Outside every segment it goes into no edit: dimmed.
                    let inside = self.marks.iter().any(|m| m.2.is_some_and(|e| s >= m.1 && s <= e));
                    let c = &if inside { *c } else { Color { a: 0.45, ..*c } };
                    frame.fill(&Path::rectangle(Point::new(x - 2.0, 0.0), Size::new(if picked { 5.0 } else { 4.0 }, h)), edge);
                    frame.fill(&Path::rectangle(Point::new(x - 1.0, 0.0), Size::new(if picked { 3.0 } else { 2.0 }, h)), *c);
                    let head = Path::new(|b| {
                        b.move_to(Point::new(x, 0.0));
                        b.line_to(Point::new(x + 11.0, 5.0));
                        b.line_to(Point::new(x, 10.0));
                        b.close();
                    });
                    frame.fill(&head, *c);
                    frame.stroke(&head, canvas::Stroke::default().with_color(edge).with_width(1.0));
                }
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
        frame.fill(&Path::rectangle(Point::new((x - 1.0).clamp(0.0, w - 2.0), BAND), Size::new(2.0, h - BAND)), Color::WHITE);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, drag: &Drag, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let on_handle = cursor.position_over(bounds).and_then(|p| self.handle_at(p.x - bounds.x, bounds.width)).is_some();
        match drag {
            Drag::Mark { .. } => mouse::Interaction::ResizingHorizontally,
            _ if on_handle => mouse::Interaction::ResizingHorizontally,
            Drag::Scrub => mouse::Interaction::Pointer,
            Drag::None if cursor.is_over(bounds) => mouse::Interaction::Pointer,
            Drag::None => mouse::Interaction::default(),
        }
    }
}
