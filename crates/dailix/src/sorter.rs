//! The TMP sorter (Omnix gap 117; the sketch the user approved,
//! 2026-11-16): TMP's videos as tiles by the day they were filmed, the
//! picked one playing above them; at the right, what the picked ones
//! become: a trip (or a new one), the day (theirs, or one given to those
//! with none), the trip's parts to put them into (or a new part), and
//! «File». Filing copies (`tmp::file`): TMP keeps them, each tile then
//! saying where it went. The rules (a click, the arrows, the parts' ops,
//! the ledger) are in `tmp.rs`, tested; this only shows and calls them.

use super::pane::Slot;
use super::{App, Message, blocking, tip};
use crate::tmp::{self, Raw};
use crate::ui;
use iced::widget::{checkbox, column, container, image, pick_list, row, rule, scrollable, slider, space, text, text_input, tooltip};
use iced::{Alignment, Element, Length, Task};
use omnix::viewer::Viewer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A tile's width; its poster 16:9.
const TILE: f32 = 176.0;
/// The panel at the right.
const SIDE: f32 = 340.0;
/// The tiles' list: its id, for scrolling a tile into view.
const TILES: &str = "tmp-tiles";

/// A tile's id: its place in TMP.
fn tile_id(rel: &str) -> String {
    format!("tmp:{rel}")
}

/// The sorter's state while it's open.
#[derive(Default)]
pub struct Sorter {
    pub raws: Vec<Raw>,
    pub scanning: bool,
    pub ledger: tmp::Ledger,
    /// The tiles picked (indices into `raws`), in the order shown; the
    /// last one clicked, where Shift runs from and what plays.
    pub picked: Vec<usize>,
    pub anchor: Option<usize>,
    pub trip: Option<PathBuf>,
    /// The trip's parts ticked; a new one's name being typed.
    pub parts: Vec<String>,
    pub new_part: String,
    /// The day given to the picked with none of their own.
    pub day: String,
    /// The video in the player: the last one clicked or stepped to.
    pub shown: Option<usize>,
    pub viewer: Option<Viewer>,
    pub showing: Option<PathBuf>,
    /// The picked being filed, one at a time (each said as it goes).
    pub filing: Option<Filing>,
    /// The player's share of the height; the edge under it being dragged
    /// (where the pointer was first seen, and the share then).
    pub share: f32,
    pub grip: Option<Option<(f32, f32)>>,
    /// Videos going into the archive or out of TMP (gap 119); the ones
    /// TMP is asked to let go.
    pub moving: Option<super::sorter_archive::Moving>,
    pub asking_free: Option<Vec<usize>>,
    /// «Archive all and free TMP» asked about.
    pub asking_all: bool,
    /// «Remove picked from TMP» asked about (the picked then), and the
    /// pointer on its button.
    pub asking_remove: Option<Vec<usize>>,
    pub remove_hover: bool,
    /// Shift and Ctrl as they are now, for a click on a tile.
    pub mods: iced::keyboard::Modifiers,
}

#[derive(Debug, Clone)]
pub enum SortMsg {
    Open,
    Scanned(Vec<Raw>),
    Posters,
    Click(usize),
    Mods(iced::keyboard::Modifiers),
    Trip(TripChoice),
    Part(String, bool),
    NewPart(String),
    AddPart,
    Day(String),
    File,
    /// One file filed (what it said, or why not); the next started.
    FiledOne(Result<Vec<crate::add::Said>, String>),
    /// The edge between the player and the tiles: pressed, the pointer
    /// moved (in the window), let go.
    Grip,
    Drag(iced::Point),
    Release,
    /// After filing (gap 119): videos into the archive; TMP asked to let
    /// them go, said no or yes to; one done (its archive copy, if made).
    Archive(Vec<usize>),
    FreeAsk(Vec<usize>),
    FreeCancel,
    FreeConfirm,
    MovedOne(String, crate::archive::Moved),
    /// Every video into the archive and out of TMP: asked, no, yes.
    AllAsk,
    AllCancel,
    AllConfirm,
    /// The picked out of TMP, to the trash: asked, no, yes; the pointer
    /// on its button (it grows and reddens).
    RemoveAsk,
    RemoveCancel,
    RemoveConfirm,
    RemoveHover(bool),
    Loaded(PathBuf, Slot),
    Toggle,
    Scrub(f64),
    ScrubDone,
    Tick,
}

/// Filing under way: into which trip and parts, what's left (each file
/// and the day it's given), how many there were and are done, what each
/// said, the last of the picked (the next new one after it is picked).
#[derive(Debug, Clone)]
pub struct Filing {
    pub root: PathBuf,
    pub parts: Vec<String>,
    pub queue: std::collections::VecDeque<(PathBuf, Option<String>)>,
    pub total: usize,
    pub done: usize,
    pub said: Vec<crate::add::Said>,
    pub last: Option<usize>,
}

/// A trip as the list offers it: its title, its folder.
#[derive(Debug, Clone, PartialEq)]
pub struct TripChoice {
    pub root: PathBuf,
    pub title: String,
}

impl std::fmt::Display for TripChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)
    }
}

impl Sorter {
    /// The tiles as shown: day by day, the undated last.
    pub fn order(&self) -> Vec<usize> {
        tmp::by_day(&self.raws).into_iter().flat_map(|(_, list)| list).collect()
    }

    /// How many of TMP's videos aren't filed anywhere yet.
    pub fn new_ones(&self) -> usize {
        tmp::new_count(&self.ledger, self.raws.iter().map(|r| r.rel.as_str()))
    }
}

impl App {
    /// The TMP place, if it's set and there.
    fn tmp_place(&self) -> Option<PathBuf> {
        self.settings.tmp.clone().filter(|d| d.is_dir())
    }

    pub(super) fn on_sort(&mut self, msg: SortMsg) -> Task<Message> {
        match msg {
            SortMsg::Open => return self.open_sorter(),
            SortMsg::Scanned(raws) => return self.scanned(raws),
            SortMsg::Posters => {}
            SortMsg::Click(i) => return self.sort_click(i),
            SortMsg::Mods(m) => {
                self.mods = m;
                if let Some(s) = &mut self.sorter {
                    s.mods = m;
                }
            }
            m @ (SortMsg::Trip(_) | SortMsg::Part(..) | SortMsg::NewPart(_) | SortMsg::AddPart | SortMsg::Day(_)) => self.sort_choice(m),
            SortMsg::File => return self.file_picked(),
            SortMsg::FiledOne(said) => return self.filed_one(said),
            m @ (SortMsg::Grip | SortMsg::Drag(_) | SortMsg::Release) => self.sort_grip(m),
            m @ (SortMsg::Archive(_) | SortMsg::FreeAsk(_) | SortMsg::FreeCancel | SortMsg::FreeConfirm | SortMsg::MovedOne(..) | SortMsg::AllAsk | SortMsg::AllCancel | SortMsg::AllConfirm | SortMsg::RemoveAsk | SortMsg::RemoveCancel | SortMsg::RemoveConfirm | SortMsg::RemoveHover(_)) => return self.on_archive(m),
            m => self.sort_player(m),
        }
        Task::none()
    }

    /// The sorter opened on TMP, its videos read off the window's thread.
    fn open_sorter(&mut self) -> Task<Message> {
        let Some(place) = self.tmp_place() else {
            self.error = Some("TMP isn't set, or isn't there: choose its folder in Settings.".into());
            return Task::none();
        };
        // A job under way: the sorter stays as it is.
        if self.sorter.as_ref().is_some_and(Sorter::busy) {
            return Task::none();
        }
        let _ = self.save_typed();
        self.at_home = true;
        self.trip_open = None;
        let trip = self.sorter.as_ref().and_then(|s| s.trip.clone());
        let share = tmp::player_share(self.settings.sorter_player.unwrap_or(0.32));
        self.sorter = Some(Sorter { scanning: true, ledger: tmp::Ledger::now(&place, self.settings.projects.as_deref()), trip, share, ..Default::default() });
        Task::perform(blocking(move || tmp::scan(&place)), |raws| Message::Sort(SortMsg::Scanned(raws)))
    }

    /// TMP read: its posters made, a few at a time; the first new one picked.
    fn scanned(&mut self, raws: Vec<Raw>) -> Task<Message> {
        let (Some(place), Some(s)) = (self.tmp_place(), &mut self.sorter) else { return Task::none() };
        s.scanning = false;
        s.raws = raws;
        let todo: Vec<(PathBuf, f64, PathBuf)> = s.raws.iter().map(|r| (r.path.clone(), r.duration, tmp::poster(&place, &r.rel))).filter(|(_, _, p)| !p.is_file()).collect();
        let first = s.order().into_iter().find(|i| crate::archive::stage(&s.ledger, &s.raws[*i].rel) == crate::archive::Stage::New);
        let pick = first.map_or_else(Task::none, |i| {
            s.mods = iced::keyboard::Modifiers::default();
            Task::done(Message::Sort(SortMsg::Click(i)))
        });
        let per = todo.len().div_ceil(super::POSTER_THREADS).max(1);
        let posters = todo.chunks(per).map(|chunk| {
            let chunk = chunk.to_vec();
            Task::perform(blocking(move || chunk.into_iter().for_each(|(c, d, p)| {
                let _ = super::make_poster(&c, d, &p);
            })), |()| Message::Sort(SortMsg::Posters))
        });
        Task::batch(posters.chain([pick]))
    }

    /// A tile clicked (Shift and Ctrl as `tmp::click` says); the one
    /// clicked opens in the player.
    fn sort_click(&mut self, at: usize) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        let order = s.order();
        (s.picked, s.anchor) = tmp::click(&s.picked, s.anchor, at, &order, s.mods.shift(), s.mods.command());
        self.show_raw(at)
    }

    /// The video `i` loaded into the player, off the window's thread.
    fn show_raw(&mut self, i: usize) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(path) = s.raws.get(i).map(|r| r.path.clone()) else { return Task::none() };
        s.shown = Some(i);
        // Its tile brought into view, as the sheet's are.
        let place = iced::advanced::widget::operate(super::Find { list_id: TILES, tile: iced::widget::Id::from(tile_id(&s.raws[i].rel)), list: None, found: None }).map(Message::Placed);
        if s.showing.as_ref() == Some(&path) {
            return place;
        }
        s.showing = Some(path.clone());
        s.viewer = None;
        let p = path.clone();
        let load = Task::perform(blocking(move || omnix::viewer::load(&p).map_err(|e| format!("{e:#}"))), move |r| Message::Sort(SortMsg::Loaded(path.clone(), Slot(Arc::new(Mutex::new(Some(r)))))));
        Task::batch([load, place])
    }

    /// The panel's choices: the trip, its parts, a new part, the day.
    fn sort_choice(&mut self, msg: SortMsg) {
        let Some(s) = &mut self.sorter else { return };
        match msg {
            SortMsg::Trip(t) => {
                s.trip = Some(t.root);
                s.parts.clear();
            }
            SortMsg::Part(name, on) => {
                s.parts.retain(|p| !dailix_sheet::parts::same_name(p, &name));
                if on {
                    s.parts.push(name);
                }
            }
            SortMsg::NewPart(typed) => s.new_part = typed,
            // A new part is ticked at once, and made in the trip at once
            // when one is chosen (the user, 2026-11-17: made here, it was
            // nowhere else till videos went in); with no trip yet, when
            // the first videos go into it (`tmp::part_ops`).
            SortMsg::AddPart => {
                let name = s.new_part.trim().to_owned();
                if !name.is_empty() && !s.parts.iter().any(|p| dailix_sheet::parts::same_name(p, &name)) {
                    s.parts.push(name.clone());
                }
                s.new_part.clear();
                if let Some(root) = s.trip.clone().filter(|_| !name.is_empty()) {
                    let known: Vec<String> = self.trips.iter().find(|t| t.root == root).map(|t| t.parts.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
                    let ops = tmp::part_ops(&known, &[name], &[]);
                    if !ops.is_empty() {
                        match dailix_sheet::Store::new(dailix_sheet::Layout::new(&root).sheet()).apply(&ops) {
                            Ok(_) => self.refresh_home(),
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                }
            }
            SortMsg::Day(typed) => s.day = typed,
            _ => {}
        }
    }

    /// The picked filed into the trip: queued, a file at a time off the
    /// window's thread, so the line under the bar says how far it is.
    fn file_picked(&mut self) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(root) = s.trip.clone().filter(|_| !s.picked.is_empty() && !s.busy()) else { return Task::none() };
        let day = s.day.trim().to_owned();
        if !day.is_empty() && !dailix_sheet::footage::is_day(&day) {
            self.error = Some(format!("«{day}» isn't a day: give it as YYYY-MM-DD."));
            return Task::none();
        }
        // Enter files as the button does, and only when it would: a video
        // with no date of its own waits for a day (Enter went past the
        // button's check, filing it into nothing).
        if day.is_empty() && s.picked.iter().any(|i| s.raws[*i].day.is_none()) {
            self.error = Some("A picked video has no date: give it a day (YYYY-MM-DD) under Day first.".into());
            return Task::none();
        }
        // The day given goes to those with none of their own only.
        let queue = s.picked.iter().map(|i| (s.raws[*i].path.clone(), s.raws[*i].day.is_none().then(|| day.clone()).filter(|d| !d.is_empty()))).collect::<std::collections::VecDeque<_>>();
        s.filing = Some(Filing { root, parts: s.parts.clone(), total: queue.len(), queue, done: 0, said: Vec::new(), last: s.picked.last().copied() });
        self.file_next()
    }

    /// The next file of the queue filed, the line saying so; the end
    /// when there's none.
    fn file_next(&mut self) -> Task<Message> {
        let place = self.tmp_place();
        let Some(f) = self.sorter.as_mut().and_then(|s| s.filing.as_mut()) else { return Task::none() };
        let (Some(place), Some((file, day))) = (place, f.queue.pop_front()) else { return self.filed() };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.notice = Some(tmp::filing_line(f.done, f.total, &dailix_sheet::project::title(&f.root), &name));
        let (root, parts) = (f.root.clone(), f.parts.clone());
        let sources = self.settings.place(crate::settings::PlaceKind::Sources).filter(|p| p.is_dir());
        Task::perform(blocking(move || tmp::file(&place, &root, &[file], day.as_deref(), &parts, sources.as_deref()).map_err(|e| e.to_string())), |said| Message::Sort(SortMsg::FiledOne(said)))
    }

    /// One file done: its line kept, the ledger read again (its tile says
    /// where it went at once), the next one; an error stops the rest.
    fn filed_one(&mut self, said: Result<Vec<crate::add::Said>, String>) -> Task<Message> {
        let place = self.tmp_place();
        let Some(s) = self.sorter.as_mut() else { return Task::none() };
        if let Some(p) = place {
            s.ledger = tmp::Ledger::now(&p, self.settings.projects.as_deref());
        }
        let Some(f) = s.filing.as_mut() else { return Task::none() };
        f.done += 1;
        match said {
            Ok(said) => f.said.extend(said),
            Err(e) => {
                f.queue.clear();
                self.error = Some(e);
            }
        }
        self.refresh_home();
        self.file_next()
    }

    /// Filing over: what it did, in a line; on to the next new tile.
    fn filed(&mut self) -> Task<Message> {
        let Some(s) = &mut self.sorter else { return Task::none() };
        let Some(f) = s.filing.take() else { return Task::none() };
        self.notice = Some(format!("{}: {}", dailix_sheet::project::title(&f.root), crate::add::summary(&f.said)));
        let order = s.order();
        let next = order.iter().skip_while(|i| Some(**i) != f.last).skip(1).find(|i| crate::archive::stage(&s.ledger, &s.raws[**i].rel) == crate::archive::Stage::New).copied();
        s.picked = next.into_iter().collect();
        s.anchor = next;
        self.refresh_home();
        next.map_or_else(Task::none, |i| self.show_raw(i))
    }

    /// The edge under the player dragged: its share of the height follows
    /// the pointer, kept when let go.
    fn sort_grip(&mut self, msg: SortMsg) {
        let room = (self.height / self.scale).max(1.0);
        let Some(s) = &mut self.sorter else { return };
        match msg {
            SortMsg::Grip => s.grip = Some(None),
            SortMsg::Drag(p) => {
                let Some(grip) = &mut s.grip else { return };
                let (y0, s0) = *grip.get_or_insert((p.y, s.share));
                s.share = tmp::player_share(s0 + (p.y - y0) / room);
            }
            SortMsg::Release => {
                s.grip = None;
                self.settings.sorter_player = Some(s.share);
                let _ = self.settings.save();
            }
            _ => {}
        }
    }

    /// The player's own messages.
    fn sort_player(&mut self, msg: SortMsg) {
        let Some(s) = &mut self.sorter else { return };
        match msg {
            SortMsg::Loaded(path, slot) if s.showing.as_ref() == Some(&path) => match slot.0.lock().ok().and_then(|mut x| x.take()) {
                Some(Ok(loaded)) => s.viewer = Some(Viewer::new(loaded)),
                Some(Err(e)) => self.error = Some(e),
                None => {}
            },
            SortMsg::Toggle => s.viewer.iter_mut().for_each(Viewer::toggle),
            SortMsg::Scrub(t) => s.viewer.iter_mut().for_each(|v| v.scrub(t)),
            SortMsg::ScrubDone => s.viewer.iter_mut().for_each(Viewer::scrub_end),
            SortMsg::Tick => s.viewer.iter_mut().for_each(Viewer::tick),
            _ => {}
        }
    }

    /// The sorter's keys: ← → to the next tile (Shift: the run grows),
    /// Space plays, Enter files, Ctrl+A picks every new one. Whether it
    /// took the key.
    pub(super) fn sort_key(&mut self, key: &iced::keyboard::Key, mods: iced::keyboard::Modifiers) -> Option<Task<Message>> {
        use iced::keyboard::Key;
        use iced::keyboard::key::Named;
        let s = self.sorter.as_mut()?;
        match key.as_ref() {
            Key::Named(Named::Space) => Some(Task::done(Message::Sort(SortMsg::Toggle))),
            Key::Named(Named::Enter) => Some(Task::done(Message::Sort(SortMsg::File))),
            Key::Character("a") if mods.command() => {
                s.picked = s.order().into_iter().filter(|i| crate::archive::stage(&s.ledger, &s.raws[*i].rel) == crate::archive::Stage::New).collect();
                Some(Task::none())
            }
            Key::Named(dir @ (Named::ArrowLeft | Named::ArrowRight)) if !mods.command() && !mods.alt() => {
                let order = s.order();
                let next = tmp::step(&order, s.shown.or(s.picked.last().copied()), if dir == Named::ArrowLeft { -1 } else { 1 })?;
                // Shift runs from where the run started; alone, just the one.
                if mods.shift() {
                    (s.picked, _) = tmp::click(&s.picked, s.anchor, next, &order, true, false);
                } else {
                    (s.picked, s.anchor) = (vec![next], Some(next));
                }
                Some(self.show_raw(next))
            }
            _ => None,
        }
    }

    /// The sorter's page: tiles by day under the player at the left, the
    /// choices at the right.
    pub(super) fn sorter_page(&self) -> Element<'_, Message> {
        let Some(s) = &self.sorter else { return space().into() };
        if s.scanning {
            return container(text("Reading TMP…").size(15).style(text::secondary)).center(Length::Fill).into();
        }
        if s.raws.is_empty() {
            let place = self.tmp_place().map(|p| omnix::paths::shown(&p)).unwrap_or_default();
            let words = column![
                text("TMP is empty").size(22),
                text(format!("Videos put into {place} show here, by the day they were filmed.")).size(14).style(text::secondary),
                ui::act("How Dailix works", ui::Kind::Secondary).on_press(Message::HowItWorks),
            ]
            .spacing(10)
            .align_x(Alignment::Center);
            return container(words).width(Length::Fill).center_x(Length::Fill).padding([80, 16]).into();
        }
        // The edge between them: dragged, the player grows or shrinks.
        let grip = iced::widget::mouse_area(container(rule::horizontal(2)).height(10).width(Length::Fill).center_y(10)).on_press(Message::Sort(SortMsg::Grip)).interaction(iced::mouse::Interaction::ResizingVertically);
        let left = column![self.sort_player_view(s), grip, scrollable(self.sort_tiles(s)).id(TILES).height(Length::Fill).width(Length::Fill)].spacing(8);
        row![left.width(Length::Fill), rule::vertical(1), container(scrollable(self.sort_side(s))).width(SIDE).padding([0, 12])].spacing(8).padding([8, 16]).into()
    }

    fn sort_player_view<'a>(&'a self, s: &'a Sorter) -> Element<'a, Message> {
        let to = |m: SortMsg| Message::Sort(m);
        let shown = s.shown.and_then(|i| s.raws.get(i));
        let name = shown.map_or_else(String::new, |r| r.rel.clone());
        let tall = (self.height / self.scale) * s.share;
        let Some(v) = &s.viewer else {
            return column![text(name).size(15), container(text(if s.showing.is_some() { "Opening…" } else { "" }).size(13).style(text::secondary)).height(tall).center_y(tall)].spacing(6).into();
        };
        let (h, w) = (tall, tall * v.aspect());
        let picture: Element<'_, Message> = match v.picture() {
            Some(p) => iced::widget::shader(p).width(w).height(h).into(),
            None => container(text("…")).width(w).height(h).center_x(w).center_y(h).into(),
        };
        let bar = row![
            ui::small(if v.playing() { "⏸" } else { "▶" }, ui::Kind::Secondary).on_press(to(SortMsg::Toggle)),
            slider(0.0..=v.duration().max(0.01), v.position(), |t| Message::Sort(SortMsg::Scrub(t))).step(0.01).on_release(to(SortMsg::ScrubDone)),
            text(format!("{} / {}", dailix_sheet::clock(v.position(), 1), dailix_sheet::clock(v.duration(), 1))).size(13).style(text::secondary),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(w.max(360.0));
        column![text(name).size(15), container(picture).center_x(Length::Fill), container(bar).center_x(Length::Fill)].spacing(6).into()
    }

    /// The tiles, a row of them a day.
    fn sort_tiles<'a>(&'a self, s: &'a Sorter) -> Element<'a, Message> {
        let place = self.tmp_place().unwrap_or_default();
        let mut days = column![].spacing(10);
        for (day, list) in tmp::by_day(&s.raws) {
            let head = self.day_head(s, day.as_deref(), &list);
            let tiles = iced::widget::row(list.iter().map(|i| self.sort_tile(s, *i, &place))).spacing(8).wrap().vertical_spacing(8);
            days = days.push(head).push(tiles);
        }
        days.padding(iced::Padding { bottom: 8.0, ..Default::default() }).into()
    }

    /// A video as a tile, as the contact sheet's are (the user, 2026-11-17:
    /// the lines were hard to read): a card on its own background, a band
    /// on top saying its stage, the poster on black in a frame of one
    /// size, the name at full brightness, «in the trip's days» in green,
    /// its part on a chip of its own.
    fn sort_tile<'a>(&'a self, s: &'a Sorter, i: usize, place: &std::path::Path) -> Element<'a, Message> {
        use crate::archive::Stage;
        let r = &s.raws[i];
        let h = (TILE * 9.0 / 16.0).round();
        let poster = tmp::poster(place, &r.rel);
        let shown: Element<'a, Message> = if poster.is_file() { image(image::Handle::from_path(poster)).height(h).into() } else { text("…").size(12).into() };
        let picture = container(shown).width(TILE).height(h).center_x(TILE).center_y(h).style(|_: &iced::Theme| container::background(iced::Color::BLACK));
        let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stage = crate::archive::stage(&s.ledger, &r.rel);
        let (went, part, in_part) = match (s.ledger.filed(&r.rel).first(), stage) {
            (Some(f), Stage::Filed | Stage::Archived) => self.went(f),
            // In the archive, its trip's copy gone since: not new.
            (_, Stage::Archived) => ("✓ In the archive".to_owned(), String::new(), false),
            _ => (format!("{:.0} s · new", r.duration), String::new(), false),
        };
        let went = text(went).size(12).style(if stage == Stage::New { text::secondary } else { text::success });
        let mut info = column![text(crate::library::middle(&name, 24)).size(13).wrapping(text::Wrapping::None), went].spacing(4).width(TILE);
        if !part.is_empty() {
            let chip: Element<'a, Message> = if in_part {
                container(text(crate::library::middle(&part, 28)).size(12)).padding([1, 6]).style(|t: &iced::Theme| container::Style {
                    background: Some(t.extended_palette().primary.weak.color.into()),
                    text_color: Some(t.extended_palette().primary.weak.text),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }).into()
            } else {
                text(part).size(12).style(text::base).into()
            };
            info = info.push(chip);
        }
        // A band as the sheet's rating band: its stage at a glance.
        let band_color = move |t: &iced::Theme| match stage {
            Stage::New => None,
            Stage::Filed => Some(t.extended_palette().success.base.color),
            Stage::Archived => Some(t.extended_palette().primary.base.color),
        };
        let band = container(space().height(3).width(TILE)).style(move |t: &iced::Theme| container::Style { background: band_color(t).map(Into::into), ..Default::default() });
        let picked = s.picked.contains(&i);
        let card = container(column![band, picture, info].spacing(5)).padding(6).style(move |t: &iced::Theme| {
            let mut c = container::rounded_box(t);
            if picked {
                c.border = iced::Border { color: t.extended_palette().primary.strong.color, width: 2.0, radius: c.border.radius };
            }
            c
        });
        let tile = iced::widget::mouse_area(card).on_press(Message::Sort(SortMsg::Click(i)));
        container(tip(tile, text(r.rel.clone()), tooltip::Position::Bottom)).id(tile_id(&r.rel)).into()
    }

    /// Where a filed video went, as its tile says it: the trip's name and
    /// the trip's parts that hold it.
    fn went(&self, f: &tmp::Filed) -> (String, String, bool) {
        let root = self.settings.projects.clone().unwrap_or_default().join(&f.trip);
        let key = format!("{}/{}", f.day, f.name);
        let trip = self.trips.iter().find(|t| t.root == root);
        let title = trip.map_or_else(|| dailix_sheet::project::title(&root), |t| t.title.clone());
        let parts: Vec<&str> = trip.map(|t| t.parts.iter().filter(|p| p.clips.contains(&key)).map(|p| p.name.as_str()).collect()).unwrap_or_default();
        let (days, part) = tmp::went_lines(&title, &parts);
        (days, part, !parts.is_empty())
    }

    /// Whether TMP's video `rel` is in a part of a trip it went to.
    pub(super) fn in_a_part(&self, s: &Sorter, rel: &str) -> bool {
        s.ledger.filed(rel).iter().any(|f| self.went(f).2)
    }

    /// The panel: what the picked become.
    fn sort_side<'a>(&'a self, s: &'a Sorter) -> Element<'a, Message> {
        let to = |m: SortMsg| Message::Sort(m);
        let label = |l: &'static str| text(l).size(13).style(text::secondary);
        let picked = s.picked.len();
        let choices: Vec<TripChoice> = self.trips.iter().map(|t| TripChoice { root: t.root.clone(), title: t.title.clone() }).collect();
        let chosen = s.trip.as_ref().and_then(|r| choices.iter().find(|c| &c.root == r).cloned());
        let trip = row![pick_list(choices, chosen.clone(), |t| Message::Sort(SortMsg::Trip(t))).placeholder("Choose a trip").width(Length::Fill), tip(ui::small("+ New", ui::Kind::Secondary).on_press(Message::NewTrip), text("A new trip, then chosen here"), tooltip::Position::Bottom)].spacing(6).align_y(Alignment::Center);
        let undated = s.picked.iter().filter(|i| s.raws[**i].day.is_none()).count();
        let days: Vec<&str> = {
            let mut d: Vec<&str> = s.picked.iter().filter_map(|i| s.raws[*i].day.as_deref()).collect();
            d.dedup();
            d
        };
        let mut day = column![label("Day"), text(if days.is_empty() { "—".to_owned() } else { format!("{} (when filmed)", days.join(", ")) }).size(14)].spacing(4);
        if undated > 0 {
            day = day.push(text_input("YYYY-MM-DD, for those with no date", &s.day).size(13).on_input(|t| Message::Sort(SortMsg::Day(t))).on_submit(to(SortMsg::File)));
        }
        let ready = chosen.is_some() && picked > 0 && !s.busy() && (undated == 0 || dailix_sheet::footage::is_day(s.day.trim()));
        let go = match &chosen {
            Some(t) => format!("File {} into {}", crate::library::count(picked, "video"), t.title),
            None => "File".to_owned(),
        };
        let file = tip(ui::act(go, ui::Kind::Primary).width(Length::Fill).on_press_maybe(ready.then_some(to(SortMsg::File))), text("Copied in, each under its day; TMP keeps them (Enter)"), tooltip::Position::Top);
        let head = row![
            text(if picked == 0 { "Nothing picked".to_owned() } else { format!("{} picked", crate::library::count(picked, "video")) }).size(18),
            space().width(Length::Fill),
            tip(ui::small("?", ui::Kind::Quiet).on_press(Message::HowItWorks), text("How Dailix works: TMP → trips → the archive → TMP freed"), tooltip::Position::Left),
        ]
        .align_y(Alignment::Center);
        let hint = text(format!("{} new in TMP · Shift or Ctrl and a click pick more", s.new_ones())).size(12).style(text::secondary);
        let next = (picked > 0).then(|| text(super::sorter_archive::next_line(crate::archive::next(&s.stages(&s.picked)))).size(13));
        column![head, hint].push(next).push(rule::horizontal(1)).push(column![ label("Trip"), trip, day, self.sort_parts(s, chosen.as_ref()), rule::horizontal(1), file].spacing(10))
            .push(space().height(18))
            .push(rule::horizontal(1))
            .push(tip(ui::act(format!("Archive all and free TMP ({})", s.raws.len()), ui::Kind::Secondary).width(Length::Fill).on_press_maybe((!s.busy() && !s.raws.is_empty()).then_some(Message::Sort(SortMsg::AllAsk))), text("Every video in TMP into the archive, each checked, then out of TMP (asked first)"), tooltip::Position::Top))
            .push(super::sorter_archive::remove_button(s))
            .spacing(10)
            .into()
    }

    /// The trip's parts to tick, and a new one.
    fn sort_parts<'a>(&'a self, s: &'a Sorter, trip: Option<&TripChoice>) -> Element<'a, Message> {
        let to = |m: SortMsg| Message::Sort(m);
        let mut names: Vec<String> = trip.and_then(|t| self.trips.iter().find(|x| x.root == t.root)).map(|t| t.parts.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
        for p in &s.parts {
            if !names.iter().any(|n| dailix_sheet::parts::same_name(n, p)) {
                names.push(p.clone());
            }
        }
        let mut col = column![text("Parts").size(13).style(text::secondary)].spacing(6);
        if names.is_empty() {
            col = col.push(text("None yet: a part is your own list of clips, from any day («Rides on Kos»).").size(12).style(text::secondary));
        }
        for n in names {
            let on = s.parts.iter().any(|p| dailix_sheet::parts::same_name(p, &n));
            let name = n.clone();
            col = col.push(checkbox(on).label(n).on_toggle(move |v| Message::Sort(SortMsg::Part(name.clone(), v))).size(16).text_size(14));
        }
        let add = row![text_input("New part", &s.new_part).size(13).on_input(|t| Message::Sort(SortMsg::NewPart(t))).on_submit(to(SortMsg::AddPart)), ui::small("Add", ui::Kind::Secondary).on_press_maybe((!s.new_part.trim().is_empty()).then_some(to(SortMsg::AddPart)))].spacing(6).align_y(Alignment::Center);
        col.push(add).into()
    }
}
