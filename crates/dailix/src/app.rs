//! Dailix's window: the home view (`home.rs`), and a project's days, each
//! with its story and its clips as tiles (a poster, the name, the length,
//! the rating, how many segments and flags, the clip's note), the player
//! and its marking in the pane at the right (`pane.rs`). Everything that
//! changes the marking goes through the sheet's core.

#[path = "pane.rs"]
mod pane;
#[path = "pane_voice.rs"]
mod pane_voice;
#[path = "bar.rs"]
mod bar;
#[path = "home.rs"]
mod home;
#[path = "menu.rs"]
mod menu;
#[path = "sheet_view.rs"]
mod sheet_view;
#[path = "settings_window.rs"]
mod settings_window;
#[path = "new_trip.rs"]
mod new_trip;

use crate::ui;
use dailix_sheet::footage::{self, Footage};
use crate::settings::Settings;
use dailix_sheet::{ClipOp, DayOp, Layout, Op, Rating, Sheet, Store};
use iced::widget::{column, container, row, rule, scrollable, space, text, text_editor, tooltip};
use iced::{Alignment, Element, Length, Task, Theme};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A tile's width; its poster is 16:9 inside it.
const TILE: f32 = 240.0;
/// How many posters are made at once (a frame of 2.7K HEVC takes about a
/// second on a CPU).
const POSTER_THREADS: usize = 4;

/// How the window starts: on a project (or where it was left), or on the
/// new trip's form (Omnix's «New trip…»).
#[derive(Clone, Debug)]
pub enum Start {
    Where(Option<PathBuf>),
    NewTrip,
}

pub fn run(start: Start) -> iced::Result {
    iced::application(move || App::new(start.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .scale_factor(|app: &App| app.scale)
        .theme(|_: &App| Theme::KanagawaDragon)
        .window(iced::window::Settings {
            size: iced::Size::new(1500.0, 920.0),
            // Docks match the window to the desktop entry by this id.
            #[cfg(target_os = "linux")]
            platform_specific: iced::window::settings::PlatformSpecific { application_id: "dailix".to_owned(), ..Default::default() },
            // Closing saves what's typed first (`Message::Closing`).
            exit_on_close_request: false,
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
    /// What a job under way is doing, or what one did: a quiet line under
    /// the bar, apart from the errors (adding videos said nothing; the UI
    /// review, 2026-11-14).
    notice: Option<String>,
    /// Posters being made, so each is made once.
    making: HashSet<PathBuf>,
    /// Voices being made: (clip, recipe). Kept here, not in the pane: the
    /// pane is new for every clip, and a make outlives it (audit A8).
    voice_makes: HashSet<(String, String)>,
    /// A day's story as being typed, till it's saved.
    drafts: HashMap<String, text_editor::Content>,
    dirty: HashSet<String>,
    /// The clip picked (its key): what the keys rate.
    selected: Option<String>,
    /// The picked clip in the player at the right.
    pane: pane::Pane,
    /// The window's height, to keep the player's picture to part of it.
    height: f32,
    /// The interface's size (Ctrl+= / Ctrl+−), remembered.
    scale: f32,
    /// The window's width, as last reported.
    width: f32,
    /// The pane's share of the window's width (not pixels: a width kept
    /// from fullscreen took more than half a smaller window, and a click on
    /// the edge then threw it to the half).
    pane_share: f32,
    /// While the edge is held: where the pointer was first seen and the
    /// width then. The width follows the pointer's move from there (it was
    /// worked out from the window's right edge, and a width the window
    /// hadn't reported yet threw the pane to its narrowest).
    grip: Option<Option<(f32, f32)>>,
    /// The keys' list shown (F1).
    help: bool,
    /// The Settings window open, at this group.
    settings_open: Option<settings_window::Group>,
    /// The home view shown (trips and days), rather than a contact sheet;
    /// its trips and places as last read.
    at_home: bool,
    /// The card under the pointer (a trip's, or an edit's), since when.
    card_hover: Option<(PathBuf, std::time::Instant)>,
    /// The edits' card frames (Omnix's, gap 110), read or being made.
    edit_frames: omnix::cards::view::Cache,
    trips: Vec<crate::library::Trip>,
    places: Vec<crate::library::Place>,
    /// A project being named on the home view: its folder, the name typed.
    naming: Option<(PathBuf, String)>,
    /// «→ Omnix» or «Empty edit» asked on a day that has such videos
    /// already: which to open, or a new one (Omnix gap 104).
    choosing: Option<home::Choosing>,
    /// A trip or an edit asked about before it goes to the trash, and the
    /// question's words.
    deleting: Option<(crate::delete::Doomed, Option<(String, String)>)>,
    /// The trip whose page the home view shows (none: the trips' cards),
    /// and its description and tags as typed there.
    trip_open: Option<PathBuf>,
    about: (String, String),
    /// The new trip's form, while open.
    new_trip: Option<new_trip::Draft>,
    /// The menu open, and where its button is once found.
    menu: Option<(menu::Menu, Option<iced::Rectangle>)>,
    /// The About window shown.
    about_open: bool,
    /// The error the last failed change of the sheet said, to clear it
    /// (and only it) once a change goes through.
    change_error: Option<String>,
    /// Trips videos are being added to.
    adding: HashSet<PathBuf>,
    /// A close refused once for a story not saved: the next one closes.
    close_anyway: bool,
    /// Edits typed in the stories, counted: a save waits for a rest.
    story_edits: u64,
    /// Edits being made for Omnix.
    making_edits: HashSet<(PathBuf, Option<String>)>,
    /// The days list at the sheet's left shown.
    days_list: bool,
    /// The player pane's sections folded.
    folded: HashSet<pane::Section>,
    /// Which tiles the sheet shows; the tile under the pointer.
    filter: sheet_view::Filter,
    tile_hover: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    /// Pick a project folder; the folder picked, if one was.
    Open,
    Opened(Option<PathBuf>),
    /// The Settings window opened or closed; one of its groups shown.
    Settings,
    SettingsGroup(settings_window::Group),
    /// A place's folder chosen (a folder dialog), from the Settings window
    /// or its chip on the home view; what was chosen.
    ChoosePlace(crate::settings::PlaceKind),
    /// The setting: a hovered card's tiles change by themselves.
    CardMotion(bool),
    /// A first run's «Use these folders»: every place not set, its default.
    UseDefaultPlaces,
    PlaceChosen(crate::settings::PlaceKind, Option<PathBuf>),
    /// A clip rated (none: unrated again).
    Rate(String, Option<u8>),
    DayEdit(String, text_editor::Action),
    /// Typing in the stories rested a moment (the count of edits then):
    /// what's typed is saved, with no Save to forget.
    StoryRest(u64),
    /// A day in the days list clicked: its heading brought to the top;
    /// where it is, found.
    JumpDay(String),
    DayAt(Placement),
    /// The days list shown or folded away.
    DaysList,
    /// An edit made for Omnix (or why not).
    EditMade(PathBuf, Option<String>, Result<PathBuf, String>),
    /// The error line, or the notice, closed.
    DismissError,
    DismissNotice,
    /// The window asked to close: what's typed is saved, then it closes.
    Closing(iced::window::Id),
    /// Posters made (or not): shown as they come.
    Posters(Vec<PathBuf>),
    /// A clip picked by a click.
    Select(String),
    /// A key pressed: the key as typed, its place on the keyboard (so
    /// letters work in any layout), the modifiers, and whether a widget
    /// (a text field) took it already.
    Key(iced::keyboard::Key, iced::keyboard::key::Physical, iced::keyboard::Modifiers, bool),
    /// The text's size, a step bigger (1), smaller (-1), or as it was (0).
    Scale(i32),
    /// The player pane's edge pressed; the pointer moved (where, in the
    /// window); the mouse button let go.
    Grip,
    Pointer(iced::Point),
    Release,
    /// The keys' list shown or hidden (F1).
    Help,
    /// Home: the trips and their days.
    Home,
    /// A project's name clicked on the home view: named there.
    Name(PathBuf),
    /// A trip's page opened from its card; its description and tags typed
    /// (saved when typing ends: Enter or leaving the page); an edit of it
    /// opened in Omnix.
    OpenTrip(PathBuf),
    /// The pointer on a trip's card (gap 110), and a screen frame while
    /// it's there: its tiles change.
    TripCard(PathBuf, omnix::cards::view::Event),
    CardTick,
    /// A trip or an edit to the trash: asked, then said no or yes to.
    DeleteAsk(crate::delete::Doomed),
    /// What it takes, counted off the window's thread (a big trip took
    /// seconds, the window frozen).
    DeleteCounted(crate::delete::Doomed, crate::delete::Size),
    DeleteCancel,
    DeleteConfirm,
    Deleted(crate::delete::Doomed, Result<(), String>),
    /// Videos added to a trip (gap 74a): the files chosen, then copied in
    /// (what each became, or why not).
    AddVideos(PathBuf),
    VideosPicked(PathBuf, Option<Vec<PathBuf>>),
    VideosAdded(PathBuf, Result<Vec<crate::add::Said>, String>),
    /// The pointer on an edit's card; its frames made (or why not).
    EditCard(PathBuf, omnix::cards::view::Event),
    EditFrames(PathBuf, Result<(), String>),
    AboutDescription(String),
    AboutTags(String),
    AboutSave,
    OpenEdit(PathBuf),
    /// The videos offered: a new one made after all, or none.
    ChooseNew,
    ChooseCancel,
    /// The new trip's form: opened, typed in, made, cancelled.
    NewTrip,
    NewTripTitle(String),
    NewTripFolder(String),
    NewTripDescription(String),
    NewTripTags(String),
    NewTripMake,
    NewTripCancel,
    NameEdit(String),
    /// The name typed kept (Enter); Esc lets it go.
    NameSave,
    /// An edit (of the project at this folder) made from its marking, of a
    /// day or of every day, and opened in Omnix.
    ToOmnix(PathBuf, Option<String>),
    /// An empty edit, of a day or of every day, opened in Omnix, its bin to
    /// build from.
    EmptyDay(PathBuf, Option<String>),
    /// A day's card clicked: its project opened at that day.
    OpenDay(PathBuf, String),
    /// Where the picked tile and the list are, to bring it into view.
    Placed(Placement),
    /// The player at the right.
    Pane(pane::PaneMsg),
    Resized(iced::Size),
    /// A menu opened (none: closed); its button found on the screen (or
    /// not); one of its entries chosen.
    Menu(Option<menu::Menu>),
    MenuAt(Option<iced::Rectangle>),
    MenuDo(Box<Message>),
    /// One step back: from the contact sheet to its trip's page, from a
    /// trip's page to the trips.
    Back,
    /// The About window shown or hidden.
    About,
    /// A trip's or an edit's folder shown in the file manager.
    ShowFolder(PathBuf),
    /// The sheet's filter chosen; the pointer onto (true) or off a tile.
    Filter(sheet_view::Filter),
    TileHover(String, bool),
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
    // The frame first, the folder after: a trip trashed while its
    // posters were being made got its folder made again (the audit).
    let Some(dir) = poster.parent() else { return false };
    let Some(jpeg) = clip.is_file().then(|| omnix::viewer::jpeg_frame(clip, duration / 3.0, 480)).flatten() else { return false };
    if !clip.is_file() || std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let part = dir.join(format!(".{}.part.jpg", poster.file_stem().unwrap_or_default().to_string_lossy()));
    std::fs::write(&part, jpeg).is_ok() && std::fs::rename(&part, poster).is_ok()
}

/// A card over the window, the rest dimmed: nothing below takes a click;
/// one outside the card sends `outside`. Every dialog's (keys, settings).
fn modal<'a>(card: impl Into<Element<'a, Message>>, outside: Message) -> Element<'a, Message> {
    let shade = container(iced::widget::center(iced::widget::opaque(card))).width(Length::Fill).height(Length::Fill).style(|_| container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55)));
    iced::widget::opaque(iced::widget::mouse_area(shade).on_press(outside))
}

/// A tooltip on a background of its own, so it reads over text.
fn tip<'a>(content: impl Into<Element<'a, Message>>, label: impl Into<Element<'a, Message>>, at: tooltip::Position) -> Element<'a, Message> {
    tooltip(content, label, at).padding(6).style(container::bordered_box).into()
}

impl App {
    fn new(start: Start) -> (App, Task<Message>) {
        let (open, new_trip) = match start {
            Start::Where(p) => (p, false),
            Start::NewTrip => (None, true),
        };
        let settings = Settings::load();
        let mut app = App { project: None, error: None, notice: None, making: HashSet::new(), voice_makes: HashSet::new(), drafts: HashMap::new(), dirty: HashSet::new(), selected: None, pane: pane::Pane::default(), height: 920.0, scale: settings.scale.unwrap_or(1.0), width: 1500.0, pane_share: settings.pane_share.unwrap_or(pane::SHARE), grip: None, help: false, settings_open: None, at_home: true, card_hover: None, edit_frames: Default::default(), trips: Vec::new(), places: Vec::new(), naming: None, choosing: None, deleting: None, trip_open: None, about: Default::default(), new_trip: None, menu: None, about_open: false, adding: HashSet::new(), change_error: None, close_anyway: false, filter: Default::default(), tile_hover: None, folded: HashSet::new(), story_edits: 0, days_list: true, making_edits: HashSet::new(), settings };
        // A project named opens; else where the window was left (home on
        // the first run, or when it was left there).
        app.refresh_home();
        if let Some(trip) = app.settings.last_trip.clone().filter(|r| Layout::new(r).is_project()) {
            app.open_trip(trip);
        }
        let left = app.settings.last.clone().filter(|r| !new_trip && Layout::new(r).is_project());
        // Omnix's «New trip…»: home, the form open.
        let form = if new_trip {
            app.trip_open = None;
            app.update(Message::NewTrip)
        } else {
            Task::none()
        };
        let clip = app.settings.last_clip.clone().filter(|_| open.is_none());
        let task = match open.or(left) {
            Some(root) => {
                app.at_home = false;
                let load = app.load(root);
                let pick = clip.filter(|k| app.project.as_ref().is_some_and(|p| p.footage.days.iter().flat_map(|d| &d.clips).any(|c| &c.key == k))).map_or_else(Task::none, |k| app.select(k));
                Task::batch([load, pick])
            }
            None => Task::none(),
        };
        // The window's real size, before any resize says it.
        let size = iced::window::latest().and_then(iced::window::size).map(Message::Resized);
        let frames = app.follow_edit_frames();
        (app, Task::batch([task, size, form, frames]))
    }

    fn title(&self) -> String {
        bar::title(&self.place())
    }

    /// Opens the project at `root` and starts making its missing posters.
    /// What's typed and not yet saved, saved: every day's story changed,
    /// the picked mark's note, the trip's description and tags. On leaving
    /// a project, going home and closing: a story typed was lost without a
    /// word (the UI review, 2026-11-14).
    /// Whether every story got saved: one that didn't stays typed, and
    /// what would throw it away waits (the audit, 2026-11-14).
    pub(super) fn save_typed(&mut self) -> bool {
        let days: Vec<String> = self.dirty.iter().cloned().collect();
        for day in days {
            self.save_story(&day);
        }
        self.save_note();
        self.save_about();
        self.dirty.is_empty()
    }

    /// Day `day`'s story as typed, saved.
    fn save_story(&mut self, day: &str) {
        let note = self.drafts.get(day).map(|c| c.text().trim_end().to_owned()).unwrap_or_default();
        if self.apply_ops(vec![Op::Day(DayOp::Note { day: day.to_owned(), note })]).is_ok() {
            self.dirty.remove(day);
        }
    }

    fn load(&mut self, root: PathBuf) -> Task<Message> {
        // A story that couldn't be saved stays on the page, its error said.
        if !self.save_typed() {
            self.at_home = false;
            return Task::none();
        }
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
        self.change_all(vec![op])
    }

    /// Applies `ops`, all or none, as one write; the id of the first thing
    /// made, if any.
    fn change_all(&mut self, ops: Vec<Op>) -> Option<String> {
        self.apply_ops(ops).ok().flatten()
    }

    /// `ops` applied to the sheet on disk: what they made, or why not. A
    /// success clears the error a change said before, and only that: a
    /// story saved by itself wiped every other error line (the audit,
    /// 2026-11-14).
    fn apply_ops(&mut self, ops: Vec<Op>) -> Result<Option<String>, ()> {
        let Some(p) = self.project.as_mut() else { return Err(()) };
        match p.store.apply(&ops).map_err(|e| e.to_string()).and_then(|(doc, out)| Ok((doc.sheet().map_err(|e| e.to_string())?, out))) {
            Ok((sheet, out)) => {
                p.sheet = sheet;
                if self.error.is_some() && self.error == self.change_error {
                    self.error = None;
                }
                self.change_error = None;
                Ok(out.into_iter().find_map(|o| o.made))
            }
            Err(e) => {
                self.error = Some(e.clone());
                self.change_error = Some(e);
                Err(())
            }
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        // Every key, with whether a field took it: Esc in the new mark's
        // note field must still cancel it, though the field takes Esc.
        let keys = iced::event::listen_with(|e, status, _| match e {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, physical_key, modifiers, .. }) => Some(Message::Key(key, physical_key, modifiers, status == iced::event::Status::Captured)),
            _ => None,
        });
        // The pointer, only while the pane's edge is held.
        let grip = if self.grip.is_some() {
            iced::event::listen_with(|e, _, _| match e {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(Message::Pointer(position)),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(Message::Release),
                _ => None,
            })
        } else {
            iced::Subscription::none()
        };
        let keys = iced::Subscription::batch([keys, grip]);
        let size = iced::window::resize_events().map(|(_, s)| Message::Resized(s));
        let size = iced::Subscription::batch([size, iced::window::close_requests().map(Message::Closing)]);
        // A hovered card's tiles change, a screen frame at a time.
        let size = if self.card_moving() { iced::Subscription::batch([size, iced::window::frames().map(|_| Message::CardTick)]) } else { size };
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
        let Some(p) = &self.project else { return Task::none() };
        let shown = |k: &str| self.filter.shows(Self::rating_of(p, k));
        match sheet_view::step_from(&self.order(), shown, self.selected.as_deref(), by) {
            Some(next) => self.select(next),
            None => Task::none(),
        }
    }

    /// Picks `key` and asks where its tile is, to scroll to it.
    fn select(&mut self, key: String) -> Task<Message> {
        self.selected = Some(key.clone());
        if self.settings.last_clip.as_ref() != Some(&key) {
            self.settings.last_clip = Some(key.clone());
            let _ = self.settings.save();
        }
        let open = self.open_clip(&key);
        let place = iced::advanced::widget::operate(Find { tile: iced::widget::Id::from(key), list: None, found: None }).map(Message::Placed);
        Task::batch([open, place])
    }

    /// The window's layout: the pane's grip dragged and let go, a picked
    /// tile scrolled into view, the scale stepped.
    fn on_layout(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Grip => self.grip = Some(None),
            Message::Pointer(p) => {
                // Moved left, the pane widens; up to half the window.
                let Some(grip) = &mut self.grip else { return Task::none() };
                let (x0, s0) = *grip.get_or_insert((p.x, self.pane_share));
                let room = (self.width / self.scale).max(1.0);
                self.pane_share = (s0 + (x0 - p.x) / room).clamp(pane::MIN_WIDTH / room, 0.5);
            }
            Message::Release => {
                self.grip = None;
                self.settings.pane_share = Some(self.pane_share);
                let _ = self.settings.save();
            }
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
            Message::Scale(step) => {
                self.scale = match step {
                    0 => 1.0,
                    s => (self.scale + 0.1 * s as f32).clamp(0.7, 2.0),
                };
                self.settings.scale = Some(self.scale);
                let _ = self.settings.save();
            }
            _ => {}
        }
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Select(key) => return self.select(key),
            Message::Pane(msg) => return self.on_pane(msg),
            Message::Resized(s) => {
                self.height = s.height;
                self.width = s.width;
            }
            Message::Help => self.help = !self.help,
            Message::DismissError => self.error = None,
            Message::DismissNotice => self.notice = None,
            m @ (Message::Filter(_) | Message::TileHover(..) | Message::Rate(..) | Message::DayEdit(..) | Message::StoryRest(_) | Message::DaysList | Message::JumpDay(_) | Message::DayAt(_)) => return self.on_sheet(m),
            Message::About => self.about_open = !self.about_open,
            Message::ShowFolder(p) => self.error = crate::editor::show(&p).err(),
            Message::Back => return self.back(),
            m @ (Message::Menu(_) | Message::MenuAt(_) | Message::MenuDo(_)) => return self.on_menu(m),
            m @ (Message::ToOmnix(..) | Message::EmptyDay(..) | Message::Name(_) | Message::NameEdit(_) | Message::NameSave | Message::Home | Message::OpenDay(..) | Message::OpenTrip(_) | Message::TripCard(..) | Message::CardTick | Message::EditCard(..) | Message::EditFrames(..) | Message::AddVideos(_) | Message::VideosPicked(..) | Message::VideosAdded(..) | Message::DeleteAsk(_) | Message::DeleteCounted(..) | Message::DeleteCancel | Message::DeleteConfirm | Message::Deleted(..) | Message::AboutDescription(_) | Message::AboutTags(_) | Message::AboutSave | Message::OpenEdit(_) | Message::ChooseNew | Message::ChooseCancel | Message::EditMade(..)) => return self.on_home(m),
            m @ (Message::Settings | Message::SettingsGroup(_) | Message::ChoosePlace(_) | Message::PlaceChosen(..) | Message::CardMotion(_) | Message::UseDefaultPlaces) => return self.on_settings(m),
            m @ (Message::NewTrip | Message::NewTripTitle(_) | Message::NewTripFolder(_) | Message::NewTripDescription(_) | Message::NewTripTags(_) | Message::NewTripMake | Message::NewTripCancel) => return self.on_new_trip(m),
            m @ (Message::Grip | Message::Pointer(_) | Message::Release | Message::Placed(_) | Message::Scale(_)) => return self.on_layout(m),
            Message::Key(key, physical, mods, taken) => return self.on_key(key, physical, mods, taken),
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
            Message::Opened(Some(root)) => {
                self.at_home = false;
                return self.load(root);
            }
            Message::Opened(None) => {}
            // A story that couldn't be saved: said, and the window stays
            // until it's closed again.
            Message::Closing(window) => {
                if self.save_typed() || self.close_anyway {
                    return iced::window::close(window);
                }
                self.close_anyway = true;
                self.error = Some(format!("A day's story couldn't be saved ({}). Close again to close anyway.", self.error.take().unwrap_or_default()));
            }
            Message::Posters(made) => {
                for p in made {
                    self.making.remove(&p);
                }
            }
        }
        Task::none()
    }

    /// A key pressed: the keys' list (F1), a mark's role being asked,
    /// the text's size, then the contact sheet's keys (ratings, moving
    /// between tiles, the player's and the marking's). `taken`: a field
    /// used it already.
    /// A window over the page takes the keys: Esc closes it, the rest
    /// wait — the keys' list, the day's videos offered (Omnix gap 104; the
    /// audit, 2026-11-12: Esc didn't close it), the new trip's form (its
    /// fields take what they type), Settings. Whether one was open.
    fn over_the_page(&mut self, esc: bool) -> bool {
        let open = self.help || self.about_open || self.menu.is_some() || self.choosing.is_some() || self.new_trip.is_some() || self.settings_open.is_some() || self.deleting.is_some();
        if open && esc {
            (self.help, self.about_open, self.menu, self.choosing, self.new_trip, self.settings_open, self.deleting) = (false, false, None, None, None, None, None);
        }
        open
    }

    /// A new mark being described: Esc and Tab take it away, even from its
    /// note field; ↑ ↓ Enter choose its role; nothing else.
    fn pending_mark_key(&mut self, key: &iced::keyboard::Key, taken: bool) -> Task<Message> {
        use iced::keyboard::Key;
        use iced::keyboard::key::Named;
        match key.as_ref() {
            Key::Named(Named::Escape) => self.cancel_pending(),
            Key::Named(Named::Tab) => {
                self.cancel_pending();
                self.pane.focused = false;
            }
            _ if !taken => return self.pending_key(key),
            _ => {}
        }
        Task::none()
    }

    fn on_key(&mut self, key: iced::keyboard::Key, physical: iced::keyboard::key::Physical, mods: iced::keyboard::Modifiers, taken: bool) -> Task<Message> {
        use iced::keyboard::key::{Code, Named, Physical};
        use iced::keyboard::Key;
        // F1 shows or hides the keys' list; Esc hides it.
        if physical == Physical::Code(Code::F1) {
            self.help = !self.help;
            return Task::none();
        }
        if self.over_the_page(matches!(key.as_ref(), Key::Named(Named::Escape))) {
            return Task::none();
        }
        if mods.command() && matches!(key.as_ref(), Key::Character("n")) {
            return self.update(Message::NewTrip);
        }
        if mods.command() && matches!(key.as_ref(), Key::Character(",")) {
            self.settings_open = Some(settings_window::Group::Places);
            return Task::none();
        }
        if self.naming.is_some() && matches!(key.as_ref(), Key::Named(Named::Escape)) {
            self.naming = None;
            return Task::none();
        }
        if self.pane.pending.is_some() {
            return self.pending_mark_key(&key, taken);
        }
        if taken {
            return Task::none();
        }
        // The text's size: Ctrl and = / − / 0.
        if mods.command() {
            match physical {
                Physical::Code(Code::Equal | Code::NumpadAdd) => return self.update(Message::Scale(1)),
                Physical::Code(Code::Minus | Code::NumpadSubtract) => return self.update(Message::Scale(-1)),
                Physical::Code(Code::Digit0 | Code::Numpad0) => return self.update(Message::Scale(0)),
                _ => {}
            }
        }
        if mods.alt() && matches!(key.as_ref(), Key::Named(Named::ArrowLeft)) {
            return self.back();
        }
        if mods.command() || mods.alt() || self.at_home {
            return Task::none();
        }
        // Tab: the keys go to the player, or back to the tiles.
        if matches!(key.as_ref(), Key::Named(Named::Tab)) && self.pane.key.is_some() {
            self.pane.focused = !self.pane.focused;
            return Task::none();
        }
        // ← → in the player: a frame, with Shift a second, as Omnix.
        if self.pane.focused
            && let Key::Named(dir @ (Named::ArrowLeft | Named::ArrowRight)) = key.as_ref()
            && let Some(v) = &mut self.pane.viewer
        {
            let sign = if dir == Named::ArrowLeft { -1.0 } else { 1.0 };
            if mods.shift() {
                v.jump((v.position() + sign).clamp(0.0, v.duration()));
            } else {
                v.step(sign);
            }
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
        Task::none()
    }

    /// The pane's width now: its share of the window, at least
    /// `MIN_WIDTH`, at most half.
    fn pane_width(&self) -> f32 {
        let room = self.width / self.scale;
        (self.pane_share * room).clamp(pane::MIN_WIDTH.min(room / 2.0), (room / 2.0).max(pane::MIN_WIDTH))
    }

    /// The window, and the keys' list over it when shown. The layer is
    /// always there (empty when hidden), so the window under it keeps its
    /// state (a scroll, a field's focus) when the list comes up.
    fn view(&self) -> Element<'_, Message> {
        iced::widget::stack![self.page(), self.menu_layer(), self.settings_layer(), self.new_trip_layer(), self.choose_layer(), self.delete_layer(), self.help_layer(), self.about_layer()].into()
    }

    fn help_layer(&self) -> Element<'_, Message> {
        if !self.help {
            return space().into();
        }
        let group = |(title, lines): &(&'static str, &'static [(&'static str, &'static str)])| -> Element<'_, Message> {
            let rows = lines.iter().map(|(k, what)| row![text(*k).size(13).width(210), text(*what).size(13).style(text::secondary)].spacing(10).into());
            column![text(*title).size(15)].push(column(rows).spacing(4)).spacing(8).into()
        };
        let half = crate::help::GROUPS.len().div_ceil(2);
        let col = |gs: &'static [(&'static str, &'static [(&'static str, &'static str)])]| column(gs.iter().map(group)).spacing(18).width(Length::Fill);
        let body = row![col(&crate::help::GROUPS[..half]), col(&crate::help::GROUPS[half..])].spacing(28);
        let card = ui::dialog(
            column![
                row![text("Keys and mouse").size(18), space().width(Length::Fill), ui::act("Close", ui::Kind::Secondary).on_press(Message::Help)].align_y(Alignment::Center),
                rule::horizontal(1),
                scrollable(body).height(Length::Shrink),
            ]
            .spacing(ui::DIALOG_GAP),
            900.0,
        )
        .max_height(720)
        .style(container::rounded_box);
        modal(card, Message::Help)
    }

    fn page(&self) -> Element<'_, Message> {
        let mut page = column![self.top_bar()];
        // What went wrong, and what a job did, each closed by its ✕.
        let line = |said: String, style: fn(&Theme) -> text::Style, close: Message| container(row![text(said).size(13).style(style).width(Length::Fill), ui::small("✕", ui::Kind::Quiet).on_press(close)].spacing(8).align_y(Alignment::Center)).padding([0, 16]);
        if let Some(e) = &self.error {
            page = page.push(line(e.clone(), text::danger, Message::DismissError));
        }
        if let Some(n) = &self.notice {
            page = page.push(line(n.clone(), text::secondary, Message::DismissNotice));
        }
        if self.at_home {
            return page.push(self.home()).into();
        }
        let Some(p) = &self.project else {
            return page.push(container(text("No project open").size(16)).center(Length::Fill)).into();
        };
        let mut days = column![].spacing(18).padding([8, 16]);
        for day in &p.footage.days {
            days = days.push(rule::horizontal(1)).push(self.day(p, day));
        }
        let list = column![container(self.filter_bar(p)).padding([4, 16]), scrollable(days).id(LIST).height(Length::Fill).width(Length::Fill)];
        let list = row![self.days_side(p)].push(list).height(Length::Fill);
        match self.pane() {
            Some(pane) => {
                let grip = iced::widget::mouse_area(container(rule::vertical(2)).width(8).height(Length::Fill).center_x(8)).on_press(Message::Grip).interaction(iced::mouse::Interaction::ResizingHorizontally);
                page.push(row![list, grip, pane].spacing(2).padding(iced::Padding { right: 8.0, bottom: 8.0, ..Default::default() })).into()
            }
            None => page.push(list).into(),
        }
    }
}

fn rating_value(r: Rating) -> u8 {
    match r {
        Rating::Junk => 0,
        Rating::Think => 1,
        Rating::Take => 2,
    }
}
