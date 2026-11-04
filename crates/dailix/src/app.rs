//! Dailix's window, its first form: a project's days, each with its story
//! and its clips as tiles (a poster, the name, the length, the rating, how
//! many segments and flags, the clip's note). Rating a clip and writing a
//! day's story go through the sheet's core, as everything that changes
//! the marking does. The player and the segments come next (as Omnix's
//! bin has them).

#[path = "pane.rs"]
mod pane;

use crate::footage::{self, Footage};
use crate::settings::Settings;
use dailix_sheet::{ClipOp, DayOp, Layout, MarkKind, Op, Rating, Sheet, Store};
use iced::widget::{button, column, container, image, row, rule, scrollable, space, text, text_editor, tooltip};
use iced::{Alignment, Element, Length, Task, Theme};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A tile's width; its poster is 16:9 inside it.
const TILE: f32 = 240.0;
/// How many posters are made at once (a frame of 2.7K HEVC takes about a
/// second on a CPU).
const POSTER_THREADS: usize = 4;

pub fn run(open: Option<PathBuf>) -> iced::Result {
    iced::application(move || App::new(open.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(|_: &App| Theme::KanagawaDragon)
        .window(iced::window::Settings {
            size: iced::Size::new(1500.0, 920.0),
            // Docks match the window to the desktop entry by this id.
            #[cfg(target_os = "linux")]
            platform_specific: iced::window::settings::PlatformSpecific { application_id: "dailix".to_owned(), ..Default::default() },
            ..Default::default()
        })
        .run()
}

/// The open project: its footage and its marking as last read or written.
struct Project {
    footage: Footage,
    store: Store,
    sheet: Sheet,
}

struct App {
    settings: Settings,
    project: Option<Project>,
    error: Option<String>,
    /// Posters being made, so each is made once.
    making: HashSet<PathBuf>,
    /// A day's story as being typed, till it's saved.
    drafts: HashMap<String, text_editor::Content>,
    dirty: HashSet<String>,
    /// The clip picked (its key): what the keys rate.
    selected: Option<String>,
    /// The picked clip in the player at the right.
    pane: pane::Pane,
    /// The window's height, to keep the player's picture to part of it.
    height: f32,
}

#[derive(Debug, Clone)]
enum Message {
    /// Pick a project folder; the folder picked, if one was.
    Open,
    Opened(Option<PathBuf>),
    /// A clip rated (none: unrated again).
    Rate(String, Option<u8>),
    DayEdit(String, text_editor::Action),
    DaySave(String),
    /// Posters made (or not): shown as they come.
    Posters(Vec<PathBuf>),
    /// A clip picked by a click.
    Select(String),
    /// A key pressed outside a text field: the key as typed, the key's
    /// place on the keyboard (so letters work in any layout), the modifiers.
    Key(iced::keyboard::Key, iced::keyboard::key::Physical, iced::keyboard::Modifiers),
    /// Where the picked tile and the list are, to bring it into view.
    Placed(Placement),
    /// The player at the right.
    Pane(pane::PaneMsg),
    Resized(iced::Size),
}

/// The tiles' list: its id, for scrolling it.
const LIST: &str = "clips";

/// The list's viewport and content, how far it's scrolled, and the picked
/// tile's bounds (content coordinates, as laid out unscrolled).
#[derive(Debug, Clone, Copy)]
struct Placement {
    view: iced::Rectangle,
    content: iced::Rectangle,
    scrolled: iced::Vector,
    tile: iced::Rectangle,
}

/// Finds the list and the tile with id `tile` in the window's widgets.
struct Find {
    tile: iced::widget::Id,
    list: Option<(iced::Rectangle, iced::Rectangle, iced::Vector)>,
    found: Option<iced::Rectangle>,
}

impl iced::advanced::widget::Operation<Placement> for Find {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<Placement>)) {
        operate(self);
    }
    fn scrollable(&mut self, id: Option<&iced::widget::Id>, bounds: iced::Rectangle, content: iced::Rectangle, translation: iced::Vector, _: &mut dyn iced::advanced::widget::operation::Scrollable) {
        if id == Some(&iced::widget::Id::new(LIST)) {
            self.list = Some((bounds, content, translation));
        }
    }
    fn container(&mut self, id: Option<&iced::widget::Id>, bounds: iced::Rectangle) {
        if id == Some(&self.tile) {
            self.found = Some(bounds);
        }
    }
    fn finish(&self) -> iced::advanced::widget::operation::Outcome<Placement> {
        match (self.list, self.found) {
            (Some((view, content, scrolled)), Some(tile)) => iced::advanced::widget::operation::Outcome::Some(Placement { view, content, scrolled, tile }),
            _ => iced::advanced::widget::operation::Outcome::None,
        }
    }
}

/// Runs `f` on a thread of its own: work that blocks (ffmpeg) without
/// holding up the window.
fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = T> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    async move { rx.await.expect("a worker thread panicked") }
}

/// A clip's poster: a frame a third in (past a clip's often shaky start),
/// 480 wide, as JPEG. False if ffmpeg couldn't.
fn make_poster(clip: &std::path::Path, duration: f64, poster: &std::path::Path) -> bool {
    let Some(dir) = poster.parent() else { return false };
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let part = dir.join(format!(".{}.part.jpg", poster.file_stem().unwrap_or_default().to_string_lossy()));
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y", "-ss", &format!("{:.2}", duration / 3.0), "-i"])
        .arg(clip)
        .args(["-frames:v", "1", "-vf", "scale=480:-2", "-q:v", "4"])
        .arg(&part)
        .stdin(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    ok && std::fs::rename(&part, poster).is_ok()
}

fn clock(t: f64) -> String {
    let s = t.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl App {
    fn new(open: Option<PathBuf>) -> (App, Task<Message>) {
        let settings = Settings::load();
        let mut app = App { project: None, error: None, making: HashSet::new(), drafts: HashMap::new(), dirty: HashSet::new(), selected: None, pane: pane::Pane::default(), height: 920.0, settings };
        let root = open.or_else(|| app.settings.last.clone());
        let task = match root {
            Some(root) => app.load(root),
            None => Task::none(),
        };
        (app, task)
    }

    fn title(&self) -> String {
        match &self.project {
            Some(p) => format!("Dailix · {}", p.footage.layout.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            None => "Dailix".to_owned(),
        }
    }

    /// Opens the project at `root` and starts making its missing posters.
    fn load(&mut self, root: PathBuf) -> Task<Message> {
        let layout = Layout::new(&root);
        if !layout.is_project() {
            self.error = Some(format!("{} isn't a Dailix project (no dailix/project.json)", root.display()));
            return Task::none();
        }
        let store = Store::new(layout.sheet());
        let read = footage::read(&layout).map_err(|e| e.to_string()).and_then(|(f, skipped)| Ok((f, skipped, store.read().map_err(|e| e.to_string())?.sheet().map_err(|e| e.to_string())?)));
        let (footage, skipped, sheet) = match read {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e);
                return Task::none();
            }
        };
        self.error = (!skipped.is_empty()).then(|| format!("{} sidecars couldn't be read; first: {}", skipped.len(), skipped[0]));
        self.drafts = footage.days.iter().map(|d| (d.day.clone(), text_editor::Content::with_text(&sheet.days.get(&d.day).map(|n| n.note.clone()).unwrap_or_default()))).collect();
        self.dirty.clear();
        self.settings.last = Some(root);
        let _ = self.settings.save();
        // Posters not made yet, a few threads at a time.
        let todo: Vec<(PathBuf, f64, PathBuf)> = footage.days.iter().flat_map(|d| &d.clips).filter(|c| !c.poster.is_file() && c.path.is_file()).map(|c| (c.path.clone(), c.duration, c.poster.clone())).collect();
        self.making = todo.iter().map(|(_, _, p)| p.clone()).collect();
        self.project = Some(Project { footage, store, sheet });
        let per = todo.len().div_ceil(POSTER_THREADS).max(1);
        Task::batch(todo.chunks(per).map(|chunk| {
            let chunk = chunk.to_vec();
            Task::perform(blocking(move || chunk.into_iter().filter(|(c, d, p)| make_poster(c, *d, p)).map(|(_, _, p)| p).collect()), Message::Posters)
        }))
    }

    /// Applies `op` to the sheet on disk and keeps the result; the id of
    /// what it made, if it made something.
    fn change(&mut self, op: Op) -> Option<String> {
        let p = self.project.as_mut()?;
        match p.store.apply(&[op]).map_err(|e| e.to_string()).and_then(|(doc, out)| Ok((doc.sheet().map_err(|e| e.to_string())?, out))) {
            Ok((sheet, out)) => {
                p.sheet = sheet;
                self.error = None;
                out.into_iter().find_map(|o| o.made)
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        let keys = iced::keyboard::listen().filter_map(|e| match e {
            iced::keyboard::Event::KeyPressed { key, physical_key, modifiers, .. } => Some(Message::Key(key, physical_key, modifiers)),
            _ => None,
        });
        let size = iced::window::resize_events().map(|(_, s)| Message::Resized(s));
        // The player's frames, every screen frame while it plays or seeks.
        if self.pane_busy() {
            iced::Subscription::batch([keys, size, iced::window::frames().map(|_| Message::Pane(pane::PaneMsg::Tick))])
        } else {
            iced::Subscription::batch([keys, size])
        }
    }

    /// Every clip's key, day by day: the order the arrows go in.
    fn order(&self) -> Vec<String> {
        self.project.iter().flat_map(|p| p.footage.days.iter().flat_map(|d| d.clips.iter().map(|c| c.key.clone()))).collect()
    }

    /// Picks the clip `by` places from the one picked (the first if none),
    /// and brings it into view.
    fn step(&mut self, by: isize) -> Task<Message> {
        let order = self.order();
        if order.is_empty() {
            return Task::none();
        }
        let at = self.selected.as_ref().and_then(|s| order.iter().position(|k| k == s));
        let next = match at {
            Some(i) => (i as isize + by).clamp(0, order.len() as isize - 1) as usize,
            None => 0,
        };
        self.select(order[next].clone())
    }

    /// Picks `key` and asks where its tile is, to scroll to it.
    fn select(&mut self, key: String) -> Task<Message> {
        self.selected = Some(key.clone());
        let open = self.open_clip(&key);
        let place = iced::advanced::widget::operate(Find { tile: iced::widget::Id::from(key), list: None, found: None }).map(Message::Placed);
        Task::batch([open, place])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Select(key) => return self.select(key),
            Message::Pane(msg) => return self.on_pane(msg),
            Message::Resized(s) => self.height = s.height,
            Message::Placed(p) => {
                // Scrolled only as far as needed, with a margin.
                let (top, bottom) = (p.tile.y - p.content.y, p.tile.y - p.content.y + p.tile.height);
                let margin = 12.0;
                let y = if top < p.scrolled.y + margin {
                    Some(top - margin)
                } else if bottom > p.scrolled.y + p.view.height - margin {
                    Some(bottom - p.view.height + margin)
                } else {
                    None
                };
                if let Some(y) = y {
                    return iced::widget::operation::scroll_to(LIST, iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: y.max(0.0) });
                }
            }
            Message::Key(key, physical, mods) => {
                use iced::keyboard::key::{Code, Named, Physical};
                use iced::keyboard::Key;
                if mods.command() || mods.alt() {
                    return Task::none();
                }
                // The player's keys first, while a clip is open in it.
                if self.pane.key.is_some()
                    && let Physical::Code(code) = physical
                    && self.pane_key(code)
                {
                    return Task::none();
                }
                // Rated by a key, by its place (P, X, U and the ? key read
                // the same in Ukrainian): set, not toggled, then on to the
                // next clip.
                let rating = match physical {
                    Physical::Code(Code::KeyP) => Some(Some(2)),
                    Physical::Code(Code::Slash) => Some(Some(1)),
                    Physical::Code(Code::KeyX) => Some(Some(0)),
                    Physical::Code(Code::KeyU) => Some(None),
                    _ => None,
                };
                if let Some(rating) = rating {
                    let Some(clip) = self.selected.clone() else { return self.step(0) };
                    self.change(Op::Clip(ClipOp::Rate { clip, rating }));
                    return self.step(1);
                }
                match key {
                    Key::Named(Named::ArrowRight) => return self.step(1),
                    Key::Named(Named::ArrowLeft) => return self.step(-1),
                    _ => {}
                }
            }
            Message::Open => {
                let start = self.settings.projects.clone();
                return Task::perform(
                    async move {
                        let mut d = rfd::AsyncFileDialog::new().set_title("Open a Dailix project");
                        if let Some(s) = start {
                            d = d.set_directory(s);
                        }
                        d.pick_folder().await.map(|f| f.path().to_owned())
                    },
                    Message::Opened,
                );
            }
            Message::Opened(Some(root)) => return self.load(root),
            Message::Opened(None) => {}
            Message::Rate(clip, rating) => {
                // The same rating again takes it off, as the sheet does.
                let now = self.project.as_ref().and_then(|p| p.sheet.clips.get(&clip)).and_then(|c| c.rating).map(rating_value);
                let rating = if now == rating { None } else { rating };
                self.change(Op::Clip(ClipOp::Rate { clip, rating }));
            }
            Message::DayEdit(day, action) => {
                if let Some(c) = self.drafts.get_mut(&day) {
                    if action.is_edit() {
                        self.dirty.insert(day.clone());
                    }
                    c.perform(action);
                }
            }
            Message::DaySave(day) => {
                let note = self.drafts.get(&day).map(|c| c.text().trim_end().to_owned()).unwrap_or_default();
                self.change(Op::Day(DayOp::Note { day: day.clone(), note }));
                if self.error.is_none() {
                    self.dirty.remove(&day);
                }
            }
            Message::Posters(made) => {
                for p in made {
                    self.making.remove(&p);
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let open = button(text("Open…")).on_press(Message::Open);
        let mut top = row![text(self.title()).size(20), space().width(Length::Fill)].spacing(12).align_y(Alignment::Center);
        if !self.making.is_empty() {
            top = top.push(text(format!("making posters: {} left", self.making.len())).size(13));
        }
        let top = top.push(open);
        let mut page = column![container(top).padding([10, 16])];
        if let Some(e) = &self.error {
            page = page.push(container(text(e.clone()).size(13).style(text::danger)).padding([0, 16]));
        }
        let Some(p) = &self.project else {
            return page.push(container(text("No project open").size(16)).center(Length::Fill)).into();
        };
        let mut days = column![].spacing(18).padding([8, 16]);
        for day in &p.footage.days {
            days = days.push(rule::horizontal(1)).push(self.day(p, day));
        }
        let list = scrollable(days).id(LIST).height(Length::Fill).width(Length::Fill);
        match self.pane() {
            Some(pane) => page.push(row![list, pane].spacing(8).padding(iced::Padding { right: 8.0, bottom: 8.0, ..Default::default() })).into(),
            None => page.push(list).into(),
        }
    }

    /// A day: its date, how far it's rated, its story, its clips.
    fn day<'a>(&'a self, p: &'a Project, day: &'a footage::Day) -> Element<'a, Message> {
        let rated = day.clips.iter().filter(|c| p.sheet.clips.get(&c.key).is_some_and(|s| s.rating.is_some())).count();
        let head = row![text(day.day.clone()).size(18), text(format!("{} clips · {rated} rated", day.clips.len())).size(13).style(text::secondary)].spacing(12).align_y(Alignment::Center);
        let story: Element<'a, Message> = match self.drafts.get(&day.day) {
            Some(content) => {
                let d = day.day.clone();
                let editor = text_editor(content).placeholder("the day's story").size(14).height(Length::Shrink).on_action(move |a| Message::DayEdit(d.clone(), a));
                let save = self.dirty.contains(&day.day).then(|| button(text("Save").size(13)).padding([4, 12]).on_press(Message::DaySave(day.day.clone())));
                row![editor].push(save).spacing(8).align_y(Alignment::Start).into()
            }
            None => space().into(),
        };
        let tiles = iced::widget::row(day.clips.iter().map(|c| self.tile(p, c))).spacing(10).wrap().vertical_spacing(10);
        column![head, story, tiles].spacing(10).into()
    }

    /// A clip as a tile.
    fn tile<'a>(&'a self, p: &'a Project, c: &'a footage::Clip) -> Element<'a, Message> {
        let h = (TILE * 9.0 / 16.0).round();
        let poster: Element<'a, Message> = if c.poster.is_file() && !self.making.contains(&c.poster) {
            image(image::Handle::from_path(&c.poster)).width(TILE).height(h).into()
        } else {
            container(text(if c.path.is_file() { "…" } else { "missing" }).size(14)).width(TILE).height(h).center_x(TILE).center_y(h).style(container::rounded_box).into()
        };
        let mark = p.sheet.clips.get(&c.key);
        let rating = mark.and_then(|m| m.rating).map(rating_value);
        let rate = |r: u8, label: &'static str, hint: &'static str| -> Element<'a, Message> {
            let on = rating == Some(r);
            tooltip(
                button(text(label).size(14)).padding([2, 10]).style(if on { button::primary } else { button::secondary }).on_press(Message::Rate(c.key.clone(), Some(r))),
                text(hint),
                tooltip::Position::Top,
            )
            .into()
        };
        let segments = mark.map_or(0, |m| m.marks.iter().filter(|x| x.kind != MarkKind::Flag).count());
        let flags = mark.map_or(0, |m| m.marks.iter().filter(|x| x.kind == MarkKind::Flag).count());
        let count = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
        let counts = match (segments, flags) {
            (0, 0) => String::new(),
            (s, 0) => count(s, "segment"),
            (0, f) => count(f, "flag"),
            (s, f) => format!("{} · {}", count(s, "segment"), count(f, "flag")),
        };
        let name = c.file.clone();
        let note = mark.map(|m| m.note.clone()).unwrap_or_default();
        let info = column![
            text(name).size(12).wrapping(text::Wrapping::None),
            row![text(clock(c.duration)).size(12).style(text::secondary), space().width(Length::Fill), rate(2, "✓", "Take"), rate(1, "?", "Think about it"), rate(0, "✕", "Junk")].spacing(4).align_y(Alignment::Center),
            text(counts).size(12).style(text::secondary),
            text(note).size(12),
        ]
        .spacing(4)
        .width(TILE)
        .clip(true);
        let picked = self.selected.as_deref() == Some(c.key.as_str());
        let tile = container(column![poster, info].spacing(6)).padding(6).id(c.key.clone()).style(move |t: &Theme| {
            let mut s = container::rounded_box(t);
            if picked {
                s.border = iced::Border { color: t.extended_palette().primary.strong.color, width: 2.0, radius: s.border.radius };
            }
            s
        });
        iced::widget::mouse_area(tile).on_press(Message::Select(c.key.clone())).into()
    }
}

fn rating_value(r: Rating) -> u8 {
    match r {
        Rating::Junk => 0,
        Rating::Think => 1,
        Rating::Take => 2,
    }
}
