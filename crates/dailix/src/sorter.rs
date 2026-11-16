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
use iced::widget::{button, checkbox, column, container, image, pick_list, row, rule, scrollable, slider, space, text, text_input, tooltip};
use iced::{Alignment, Element, Length, Task};
use omnix::viewer::Viewer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A tile's width; its poster 16:9.
const TILE: f32 = 176.0;
/// The panel at the right.
const SIDE: f32 = 340.0;

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
    pub filing: bool,
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
    Filed(PathBuf, Result<Vec<crate::add::Said>, String>),
    Loaded(PathBuf, Slot),
    Toggle,
    Scrub(f64),
    ScrubDone,
    Tick,
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
                if let Some(s) = &mut self.sorter {
                    s.mods = m;
                }
            }
            m @ (SortMsg::Trip(_) | SortMsg::Part(..) | SortMsg::NewPart(_) | SortMsg::AddPart | SortMsg::Day(_)) => self.sort_choice(m),
            SortMsg::File => return self.file_picked(),
            SortMsg::Filed(root, said) => return self.filed(root, said),
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
        let _ = self.save_typed();
        self.at_home = true;
        self.trip_open = None;
        let trip = self.sorter.as_ref().and_then(|s| s.trip.clone());
        self.sorter = Some(Sorter { scanning: true, ledger: tmp::Ledger::load(&place), trip, ..Default::default() });
        Task::perform(blocking(move || tmp::scan(&place)), |raws| Message::Sort(SortMsg::Scanned(raws)))
    }

    /// TMP read: its posters made, a few at a time; the first new one picked.
    fn scanned(&mut self, raws: Vec<Raw>) -> Task<Message> {
        let (Some(place), Some(s)) = (self.tmp_place(), &mut self.sorter) else { return Task::none() };
        s.scanning = false;
        s.raws = raws;
        let todo: Vec<(PathBuf, f64, PathBuf)> = s.raws.iter().map(|r| (r.path.clone(), r.duration, tmp::poster(&place, &r.rel))).filter(|(_, _, p)| !p.is_file()).collect();
        let first = s.order().into_iter().find(|i| s.ledger.filed(&s.raws[*i].rel).is_empty());
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
        if s.showing.as_ref() == Some(&path) {
            return Task::none();
        }
        s.showing = Some(path.clone());
        s.viewer = None;
        let p = path.clone();
        Task::perform(blocking(move || omnix::viewer::load(&p).map_err(|e| format!("{e:#}"))), move |r| Message::Sort(SortMsg::Loaded(path.clone(), Slot(Arc::new(Mutex::new(Some(r)))))))
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
            // A new part is ticked at once; it's made in the trip when
            // the first videos go into it (`tmp::part_ops`).
            SortMsg::AddPart => {
                let name = s.new_part.trim().to_owned();
                if !name.is_empty() && !s.parts.iter().any(|p| dailix_sheet::parts::same_name(p, &name)) {
                    s.parts.push(name);
                }
                s.new_part.clear();
            }
            SortMsg::Day(typed) => s.day = typed,
            _ => {}
        }
    }

    /// The picked filed into the trip, off the window's thread.
    fn file_picked(&mut self) -> Task<Message> {
        let (Some(place), Some(s)) = (self.tmp_place(), &mut self.sorter) else { return Task::none() };
        let Some(root) = s.trip.clone().filter(|_| !s.picked.is_empty() && !s.filing) else { return Task::none() };
        let day = s.day.trim().to_owned();
        if !day.is_empty() && !dailix_sheet::footage::is_day(&day) {
            self.error = Some(format!("«{day}» isn't a day: give it as YYYY-MM-DD."));
            return Task::none();
        }
        s.filing = true;
        let files: Vec<PathBuf> = s.picked.iter().map(|i| s.raws[*i].path.clone()).collect();
        // The day given goes to those with none of their own only.
        let (dated, undated): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|f| s.raws.iter().any(|r| &r.path == f && r.day.is_some()));
        let parts = s.parts.clone();
        let r = root.clone();
        Task::perform(
            blocking(move || {
                let mut said = tmp::file(&place, &r, &dated, None, &parts).map_err(|e| e.to_string())?;
                if !undated.is_empty() {
                    said.extend(tmp::file(&place, &r, &undated, (!day.is_empty()).then_some(day.as_str()), &parts).map_err(|e| e.to_string())?);
                }
                Ok(said)
            }),
            move |said| Message::Sort(SortMsg::Filed(root.clone(), said)),
        )
    }

    /// What filing did, in a line; the ledger read again; on to the next
    /// new tile.
    fn filed(&mut self, root: PathBuf, said: Result<Vec<crate::add::Said>, String>) -> Task<Message> {
        let place = self.tmp_place();
        let Some(s) = &mut self.sorter else { return Task::none() };
        s.filing = false;
        match said {
            Ok(said) => {
                let title = dailix_sheet::project::title(&root);
                self.notice = Some(format!("{title}: {}", crate::add::summary(&said)));
            }
            Err(e) => self.error = Some(e),
        }
        if let Some(p) = place {
            s.ledger = tmp::Ledger::load(&p);
        }
        let order = s.order();
        let last = s.picked.last().copied();
        let next = order.iter().skip_while(|i| Some(**i) != last).skip(1).find(|i| s.ledger.filed(&s.raws[**i].rel).is_empty()).copied();
        s.picked = next.into_iter().collect();
        s.anchor = next;
        self.refresh_home();
        next.map_or_else(Task::none, |i| self.show_raw(i))
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
                s.picked = s.order().into_iter().filter(|i| s.ledger.filed(&s.raws[*i].rel).is_empty()).collect();
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
            let place = self.tmp_place().map(|p| p.display().to_string()).unwrap_or_default();
            return container(column![text("TMP is empty").size(20), text(format!("Videos put into {place} show here, by the day they were filmed.")).size(14).style(text::secondary)].spacing(8).align_x(Alignment::Center)).center(Length::Fill).into();
        }
        let left = column![self.sort_player_view(s), rule::horizontal(1), scrollable(self.sort_tiles(s)).height(Length::Fill).width(Length::Fill)].spacing(8);
        row![left.width(Length::Fill), rule::vertical(1), container(scrollable(self.sort_side(s))).width(SIDE).padding([0, 12])].spacing(8).padding([8, 16]).into()
    }

    fn sort_player_view<'a>(&'a self, s: &'a Sorter) -> Element<'a, Message> {
        let to = |m: SortMsg| Message::Sort(m);
        let shown = s.shown.and_then(|i| s.raws.get(i));
        let name = shown.map_or_else(String::new, |r| r.rel.clone());
        let tall = (self.height / self.scale) * 0.32;
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
            let filed = list.iter().filter(|i| !s.ledger.filed(&s.raws[**i].rel).is_empty()).count();
            let head = format!("{} · {} · {filed} filed", day.as_deref().unwrap_or("No date"), crate::library::count(list.len(), "video"));
            let tiles = iced::widget::row(list.iter().map(|i| self.sort_tile(s, *i, &place))).spacing(8).wrap().vertical_spacing(8);
            days = days.push(text(head).size(14).style(text::secondary)).push(tiles);
        }
        days.padding(iced::Padding { bottom: 8.0, ..Default::default() }).into()
    }

    fn sort_tile<'a>(&'a self, s: &'a Sorter, i: usize, place: &std::path::Path) -> Element<'a, Message> {
        let r = &s.raws[i];
        let h = (TILE * 9.0 / 16.0).round();
        let poster = tmp::poster(place, &r.rel);
        let picture: Element<'a, Message> = if poster.is_file() { image(image::Handle::from_path(poster)).width(TILE).height(h).into() } else { container(text("…").size(12)).width(TILE).height(h).center_x(TILE).center_y(h).style(container::rounded_box).into() };
        let filed = s.ledger.filed(&r.rel);
        let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let went = filed.first().map_or_else(|| format!("{:.0} s · new", r.duration), |f| format!("✓ {}", dailix_sheet::project::title(&self.settings.projects.clone().unwrap_or_default().join(&f.trip))));
        let info = column![text(name).size(12), text(went).size(11).style(if filed.is_empty() { text::secondary } else { text::success })].spacing(2).width(TILE);
        let picked = s.picked.contains(&i);
        let tile = button(column![picture, info].spacing(4)).padding(4).on_press(Message::Sort(SortMsg::Click(i))).style(move |t, st| {
            let mut b = if picked { button::primary(t, st) } else { button::text(t, st) };
            if !picked && !filed.is_empty() {
                b.text_color = t.extended_palette().background.strong.color;
            }
            b
        });
        tip(tile, text(r.rel.clone()), tooltip::Position::Bottom)
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
        let ready = chosen.is_some() && picked > 0 && !s.filing && (undated == 0 || dailix_sheet::footage::is_day(s.day.trim()));
        let go = match &chosen {
            Some(t) => format!("File {} into {}", crate::library::count(picked, "video"), t.title),
            None => "File".to_owned(),
        };
        let file = tip(ui::act(go, ui::Kind::Primary).width(Length::Fill).on_press_maybe(ready.then_some(to(SortMsg::File))), text("Copied in, each under its day; TMP keeps them (Enter)"), tooltip::Position::Top);
        let head = text(if picked == 0 { "Nothing picked".to_owned() } else { format!("{} picked", crate::library::count(picked, "video")) }).size(18);
        let hint = text(format!("{} new in TMP · Shift or Ctrl and a click pick more", s.new_ones())).size(12).style(text::secondary);
        column![head, hint, rule::horizontal(1), label("Trip"), trip, day, self.sort_parts(s, chosen.as_ref()), rule::horizontal(1), file].spacing(10).into()
    }

    /// The trip's parts to tick, and a new one.
    fn sort_parts<'a>(&'a self, s: &'a Sorter, trip: Option<&TripChoice>) -> Element<'a, Message> {
        let to = |m: SortMsg| Message::Sort(m);
        let mut names: Vec<String> = trip.map(|t| crate::library::parts(&t.root).into_iter().map(|p| p.name).collect()).unwrap_or_default();
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
