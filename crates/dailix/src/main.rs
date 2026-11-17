//! Dailix: bringing footage in and choosing it. The window (`app`), and
//! the command line for tests and an AI: the same core under both.

mod add;
mod archive;
mod app;
mod delete;
mod help;
mod import;
mod library;
mod editor;
mod install;
mod settings;
mod tmp;
mod ui;
mod voice;

use clap::{Parser, Subcommand};
use dailix_sheet::{Doc, Op, Sheet, Store};
use std::path::PathBuf;
use std::process::ExitCode;

/// Dailix's command line as the guide's reference page (made as Omnix's).
fn commands_page() -> String {
    use clap::CommandFactory;
    omnix::reference::commands(&Cli::command(), "dailix", "Commands: Dailix", "Made by `dailix commands`")
}

#[derive(Parser)]
#[command(name = "dailix", version, about = "Bringing footage in and choosing it")]
struct Cli {
    /// Without a command: the window, on the project last opened.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// The window, on the project at this folder.
    Open { project: PathBuf },
    // A new project is made here; Omnix asks for one (user, 2026-11-08).
    /// A new trip in the Trips place, in the fixed layout. With a title
    /// it's made and its folder printed; without, the window opens on the
    /// form.
    NewTrip {
        title: Option<String>,
        /// Its folder's name (default: the month and the title in Latin).
        #[arg(long)]
        folder: Option<String>,
        #[arg(long, default_value = "")]
        description: String,
        /// Comma-separated.
        #[arg(long, default_value = "")]
        tags: String,
    },
    /// Videos added to a trip, each under the day it was filmed (its
    /// name's date, else the file's), whole or a span:
    /// `FILE`, `FILE@all`, `FILE@0:05-0:20`. The originals are only read.
    Add {
        trip: PathBuf,
        #[arg(required = true)]
        files: Vec<String>,
        /// The day to put them under instead (YYYY-MM-DD).
        #[arg(long)]
        day: Option<String>,
    },
    /// The TMP place (gap 117): its videos by day and where each went;
    /// filing them into a trip.
    #[command(subcommand)]
    Tmp(TmpCmd),
    /// The contact sheet's marking (a project's `dailix/sheet.json`).
    #[command(subcommand)]
    Sheet(SheetCmd),
    // Omnix gap 77.
    /// Clips' cleaned voices: what's made, making one,
    /// measuring their loudness.
    #[command(subcommand)]
    Voice(VoiceCmd),
    /// The three places (README): shows them, or sets those given.
    Places {
        #[arg(long)]
        tmp: Option<PathBuf>,
        #[arg(long)]
        projects: Option<PathBuf>,
        /// The originals' folder (a local, removable or mounted network one).
        #[arg(long)]
        sources: Option<PathBuf>,
    },
    /// A footage-agent project made into a Dailix one, in the Trips
    /// place; the old one and the archive are only read.
    ImportLegacy {
        old: PathBuf,
        /// The new project's folder name (default: the old one's).
        #[arg(long)]
        name: Option<String>,
        /// Say what it would copy, and copy nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// The icon and a desktop entry, in the menu and on the desktop (Linux).
    Install,
    /// Every key and gesture of the window, as Markdown tables (the
    /// guide's reference page, the F1 list).
    Keys,
    /// Every command and its arguments, as Markdown (the guide's
    /// reference page; the same text as --help).
    Commands,
    /// The trips in the Trips place (or in the folder given) as the home
    /// view sums them up: a line per trip and per day.
    Trips { projects: Option<PathBuf> },
    /// A project's name as people say it: shows it, or sets it (an empty
    /// one gives back the folder's name). The folder isn't renamed.
    Title { project: PathBuf, title: Option<String> },
    /// A project's description and tags (as the home view edits them):
    /// shows them, or sets those given; an empty one is taken out.
    About {
        project: PathBuf,
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated.
        #[arg(long)]
        tags: Option<String>,
    },
    /// The edits lying loose in a project's old 04_project, each moved
    /// into a video of its own (videos/<name>/edit.json); copies and empty
    /// ones to the trash, other files left. Says what it would do; does it
    /// with --apply.
    TidyEdits {
        project: PathBuf,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
enum SheetCmd {
    /// Applies ops (a JSON op or a list of them; `-` reads them from
    /// stdin) to a sheet: all or none, checked, written. A line per op:
    /// what it made, or why the whole run was refused.
    Apply { notes: PathBuf, ops: PathBuf },
    /// What's wrong with a sheet, a line each; nothing when it's sound.
    Check { notes: PathBuf },
    /// A sheet as text: a line per day, rated clip and mark, what an AI
    /// reads in place of the JSON.
    Describe {
        notes: PathBuf,
        /// One day only (YYYY-MM-DD).
        #[arg(long)]
        day: Option<String>,
    },
    /// The JSON Schema of the sheet's file, or of the ops.
    Schema {
        #[arg(value_enum, default_value = "ops")]
        of: SchemaOf,
    },
}

#[derive(Subcommand)]
enum TmpCmd {
    /// TMP's videos, a day at a time: each one's length, and the trip,
    /// day and name it was filed under (or «new»).
    List,
    /// Videos of TMP filed into a trip: copied in under the day each was
    /// filmed (or --day), put into the parts named (made if the trip
    /// hasn't them). TMP keeps them, and its ledger says where they went.
    File {
        trip: PathBuf,
        /// TMP's videos: their places from TMP's top, or paths.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// The day to put them under instead (YYYY-MM-DD).
        #[arg(long)]
        day: Option<String>,
        /// A part of the trip to put them into; again for more.
        #[arg(long)]
        part: Vec<String>,
    },
    /// TMP's filed videos not in the archive yet copied into the Sources
    /// folder (`<trip>/<day>/`), each checked there by its hash.
    Archive,
    /// TMP's videos the archive holds, to the trash, each archive copy
    /// checked again first. Without --yes, only says what would go.
    Free {
        #[arg(long)]
        yes: bool,
    },
}

/// `dailix tmp …`.
fn tmp_cmd(cmd: TmpCmd) -> Result<(), Box<dyn std::error::Error>> {
    let place = settings::Settings::load().tmp.filter(|d| d.is_dir()).ok_or("TMP isn't set, or isn't there (dailix places --tmp FOLDER)")?;
    let ledger = || tmp::Ledger::now(&place, settings::Settings::load().projects.as_deref());
    match cmd {
        TmpCmd::Archive => {
            let sources = settings::Settings::load().place(settings::PlaceKind::Sources).filter(|p| p.is_dir()).ok_or("the archive (Sources) isn't set, or isn't there (dailix places --sources FOLDER)")?;
            let l = ledger();
            let todo: Vec<tmp::Raw> = tmp::scan(&place).into_iter().filter(|r| archive::stage(&l, &r.rel) == archive::Stage::Filed).collect();
            for r in todo {
                let filed = l.filed(&r.rel)[0].clone();
                match archive::archive_one(&r.path, &filed, &sources) {
                    Ok(a) => {
                        println!("{}  → {}", r.rel, a.path.display());
                        tmp::Ledger::update(&place, |l| {
                            l.archived.insert(r.rel.clone(), a);
                        })?;
                    }
                    Err(e) => println!("{}  not archived: {e}", r.rel),
                }
            }
        }
        TmpCmd::Free { yes } => {
            let l = ledger();
            for r in tmp::scan(&place).into_iter().filter(|r| archive::stage(&l, &r.rel) == archive::Stage::Archived) {
                if !yes {
                    println!("{}  would go (archived at {})", r.rel, l.archived[&r.rel].path.display());
                    continue;
                }
                match archive::free_one(&r.path, &tmp::poster(&place, &r.rel), &l.archived[&r.rel]) {
                    Ok(()) => {
                        tmp::Ledger::update(&place, |l| l.forget(&r.rel))?;
                        println!("{}  to the trash", r.rel);
                    }
                    Err(e) => println!("{}  kept: {e}", r.rel),
                }
            }
        }
        TmpCmd::List => print!("{}", tmp::describe(&tmp::scan(&place), &tmp::Ledger::load(&place))),
        TmpCmd::File { trip, files, day, part } => {
            let files: Vec<PathBuf> = files.into_iter().map(|f| if f.is_absolute() { f } else { place.join(f) }).collect();
            let said = tmp::file(&place, &trip, &files, day.as_deref(), &part)?;
            for line in &said {
                println!("{line}");
            }
            let failed = said.iter().filter(|s| matches!(s.what, add::What::Failed(_))).count();
            if failed > 0 {
                return Err(format!("{failed} not filed").into());
            }
        }
    }
    Ok(())
}

#[derive(Subcommand)]
enum VoiceCmd {
    /// A line per clip with voices: each one made, its loudness, the one
    /// the edit uses (*), and the recipes there are.
    List { project: PathBuf },
    /// Clip `<day>/<file>`'s voice made by a recipe (dnn6 unless asked),
    /// from its original; made again if it's there.
    Make {
        project: PathBuf,
        clip: String,
        #[arg(long, default_value = dailix_sheet::voice::DEFAULT)]
        recipe: String,
    },
    /// The loudness of every voice not measured yet (made by footage-agent).
    Measure { project: PathBuf },
    // Gap 83.
    /// Where clip `<day>/<file>` holds speech, found in its
    /// cleaned voice: a line per stretch, seconds.
    Speech { project: PathBuf, clip: String },
    /// Clip `<day>/<file>`'s edit to use its voice made by RECIPE, or its
    /// own sound without one (the pane's «Use for the edit»).
    Use { project: PathBuf, clip: String, recipe: Option<String> },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum SchemaOf {
    Ops,
    Sheet,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // The window, unless a command was given.
    let open = match &cli.command {
        None => Some(app::Start::Where(None)),
        Some(Command::Open { project }) => Some(app::Start::Where(Some(project.clone()))),
        Some(Command::NewTrip { title: None, .. }) => Some(app::Start::NewTrip),
        _ => None,
    };
    if let Some(start) = open {
        return match app::run(start) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("dailix: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dailix: {e}");
            ExitCode::FAILURE
        }
    }
}

fn voice_cmd(cmd: VoiceCmd) -> Result<(), Box<dyn std::error::Error>> {
    let project = match &cmd {
        VoiceCmd::List { project } | VoiceCmd::Make { project, .. } | VoiceCmd::Measure { project } | VoiceCmd::Speech { project, .. } | VoiceCmd::Use { project, .. } => dailix_sheet::Layout::new(project),
    };
    if !project.is_project() {
        return Err(format!("not a Dailix project: {}", project.root.display()).into());
    }
    match cmd {
        VoiceCmd::List { .. } => print!("{}", voice::describe(&project)?),
        VoiceCmd::Make { clip, recipe, .. } => {
            let recipe = dailix_sheet::voice::recipe(&recipe).ok_or_else(|| format!("no recipe {recipe}: {}", dailix_sheet::voice::RECIPES.map(|r| r.name).join(", ")))?;
            let source = voice::source(&project, &clip)?;
            println!("{}", voice::make(&project, &clip, &source, recipe)?.display());
        }
        VoiceCmd::Measure { .. } => {
            let (footage, _) = dailix_sheet::footage::read(&project)?;
            let mut n = 0;
            for clip in footage.days.iter().flat_map(|d| &d.clips) {
                n += voice::measure(&project, &clip.key)?;
            }
            println!("{n} measured");
        }
        VoiceCmd::Use { clip, recipe, .. } => {
            voice::use_voice(&project, &clip, recipe.as_deref())?;
            println!("{clip}: {}", recipe.as_deref().unwrap_or("its own sound"));
        }
        VoiceCmd::Speech { clip, .. } => {
            let (from, spans) = voice::speech(&project, &clip)?;
            println!("in {from}:");
            for (a, b) in spans {
                println!("{} – {}", dailix_sheet::clock(a, 1), dailix_sheet::clock(b, 1));
            }
        }
    }
    Ok(())
}

/// The places and the trips in them: a new trip, the places set, an old project brought in, the trips listed.
fn places_cmd(cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Command::NewTrip { title: Some(title), folder, description, tags } => {
            let place = settings::Settings::load().projects.ok_or("no projects place set (dailix places --projects …)")?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let folder = folder.unwrap_or_else(|| dailix_sheet::project::trip_folder(&dailix_sheet::project::month_of(now), &title));
            let about = dailix_sheet::project::About { description, tags: dailix_sheet::project::parse_tags(&tags) };
            println!("{}", dailix_sheet::project::new_trip(&place, &folder, &title, &about)?.display());
            Ok(())
        }
        Command::Places { tmp, projects, sources } => {
            let mut s = settings::Settings::load();
            let changed = tmp.is_some() || projects.is_some() || sources.is_some();
            s.tmp = tmp.or(s.tmp);
            s.projects = projects.or(s.projects);
            s.sources = sources.map(|path| settings::Sources::Folder { path }).or(s.sources);
            if changed {
                s.save()?;
            }
            print!("{}", s.describe());
            Ok(())
        }
        Command::ImportLegacy { old, name, dry_run } => {
            let projects = settings::Settings::load().projects.ok_or("set the projects place first: dailix places --projects <folder>")?;
            let name = name.or_else(|| old.file_name().map(|n| n.to_string_lossy().into_owned())).ok_or("name the new project: --name")?;
            let plan = import::plan(&old, &projects.join(&name))?;
            let gb = |b: u64| b as f64 / 1e9;
            println!("{} → {}: {} files, {:.1} GB", old.display(), plan.layout.root.display(), plan.copies.len(), gb(plan.bytes()));
            if dry_run {
                return Ok(());
            }
            let mut shown = 0;
            import::run(&plan, |done, total| {
                // A line every 5 % or so.
                let pct = (done * 100).checked_div(total).unwrap_or(100);
                if pct >= shown + 5 || done == total {
                    shown = pct;
                    println!("{pct:>3} %  {:.1} of {:.1} GB", gb(done), gb(total));
                }
            })?;
            println!("done: {}", plan.layout.root.display());
            Ok(())
        }
        Command::Trips { projects } => {
            let projects = projects.or(settings::Settings::load().projects).ok_or("no projects place: dailix places --projects <folder>")?;
            let (trips, skipped) = library::trips(&projects);
            print!("{}", library::describe(&trips));
            for s in skipped {
                eprintln!("skipped: {s}");
            }
            Ok(())
        }
        _ => unreachable!("places_cmd is given only its own commands"),
    }
}

/// One trip: its title, videos added, its about, its old edits tidied.
fn trip_cmd(cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Command::Title { project, title } => {
            if !dailix_sheet::Layout::new(&project).is_project() {
                return Err(format!("not a Dailix project: {}", project.display()).into());
            }
            if let Some(t) = title {
                dailix_sheet::project::set_title(&project, &t)?;
            }
            println!("{}", dailix_sheet::project::title(&project));
            Ok(())
        }
        Command::Add { trip, files, day } => {
            let picks = files.iter().map(|f| add::parse_pick(f)).collect::<Result<Vec<_>, _>>()?;
            let said = add::add(&trip, &picks, day.as_deref())?;
            for line in &said {
                println!("{line}");
            }
            let failed = said.iter().filter(|s| matches!(s.what, add::What::Failed(_))).count();
            if failed > 0 {
                return Err(format!("{failed} not added").into());
            }
            Ok(())
        }
        Command::About { project, description, tags } => {
            if !dailix_sheet::Layout::new(&project).is_project() {
                return Err(format!("not a Dailix project: {}", project.display()).into());
            }
            let mut about = dailix_sheet::project::about(&project);
            if description.is_some() || tags.is_some() {
                about = dailix_sheet::project::About { description: description.unwrap_or(about.description), tags: tags.map_or(about.tags, |t| dailix_sheet::project::parse_tags(&t)) };
                dailix_sheet::project::set_about(&project, &about)?;
            }
            println!("description: {}\ntags: {}", about.description, about.tags.join(", "));
            Ok(())
        }
        Command::TidyEdits { project, apply } => {
            use dailix_sheet::project::Tidy;
            if !dailix_sheet::Layout::new(&project).is_project() {
                return Err(format!("not a Dailix project: {}", project.display()).into());
            }
            for t in dailix_sheet::project::tidy_old_edits(&project) {
                match t {
                    Tidy::Move { from, to } => {
                        println!("move   {} → {}", from.display(), to.display());
                        if apply {
                            std::fs::create_dir_all(to.parent().unwrap_or(&to))?;
                            std::fs::rename(&from, &to)?;
                        }
                    }
                    Tidy::Same { path, as_ } => {
                        println!("trash  {} (the same as {})", path.display(), as_.display());
                        if apply {
                            trash::delete(&path)?;
                        }
                    }
                    Tidy::Empty { path } => {
                        println!("trash  {} (empty)", path.display());
                        if apply {
                            trash::delete(&path)?;
                        }
                    }
                    Tidy::Other { path } => println!("keep   {} (not an Omnix edit)", path.display()),
                }
            }
            // The old folder goes once nothing's left in it.
            let old = dailix_sheet::Layout::new(&project).old_edits();
            if apply && std::fs::read_dir(&old).is_ok_and(|mut d| d.next().is_none()) {
                std::fs::remove_dir(&old)?;
                println!("gone   {}", old.display());
            }
            Ok(())
        }
        _ => unreachable!("trip_cmd is given only its own commands"),
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = match cli.command.expect("the window is opened in main") {
        Command::Open { .. } | Command::NewTrip { title: None, .. } => unreachable!("the window is opened in main"),
        Command::Sheet(cmd) => cmd,
        Command::Voice(cmd) => return voice_cmd(cmd),
        Command::Tmp(cmd) => return tmp_cmd(cmd),
        c @ (Command::ImportLegacy { .. } | Command::NewTrip { .. } | Command::Places { .. } | Command::Trips { .. }) => return places_cmd(c),
        c @ (Command::About { .. } | Command::Add { .. } | Command::TidyEdits { .. } | Command::Title { .. }) => return trip_cmd(c),
        Command::Keys => {
            print!("{}", help::page());
            return Ok(());
        }
        Command::Commands => {
            print!("{}", commands_page());
            return Ok(());
        }
        Command::Install => {
            for f in install::install()? {
                println!("{}", f.display());
            }
            return Ok(());
        }
    };
    match cmd {
        SheetCmd::Apply { notes, ops } => {
            let text = if ops.as_os_str() == "-" { std::io::read_to_string(std::io::stdin())? } else { std::fs::read_to_string(&ops)? };
            let value: serde_json::Value = serde_json::from_str(&text)?;
            let ops: Vec<Op> = match value {
                serde_json::Value::Array(_) => serde_json::from_value(value)?,
                one => vec![serde_json::from_value(one)?],
            };
            let (_, outcomes) = Store::new(notes).apply(&ops)?;
            for (i, o) in outcomes.iter().enumerate() {
                match &o.made {
                    Some(made) => println!("{}: made {made}", i + 1),
                    None => println!("{}: done", i + 1),
                }
            }
        }
        SheetCmd::Check { notes } => {
            let doc = Doc::parse(&std::fs::read_to_string(&notes)?)?;
            for p in doc.check() {
                println!("{p}");
            }
            // A project's own sheet: its voices are looked for too.
            let root = notes.parent().and_then(std::path::Path::parent).map(dailix_sheet::Layout::new);
            if let (Some(layout), Ok(sheet)) = (root.filter(|l| l.is_project() && l.sheet() == notes), doc.sheet()) {
                for p in dailix_sheet::voice::gone(&layout, &sheet) {
                    println!("{p}");
                }
            }
        }
        SheetCmd::Describe { notes, day } => {
            let sheet = Doc::parse(&std::fs::read_to_string(notes)?)?.sheet()?;
            print!("{}", sheet.describe(day.as_deref()));
        }
        SheetCmd::Schema { of } => {
            let schema = match of {
                SchemaOf::Ops => schemars::schema_for!(Op),
                SchemaOf::Sheet => schemars::schema_for!(Sheet),
            };
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The guide (in Omnix's repo beside this one) shows this command line
    /// as it is now: `OMNIX_BLESS=1 cargo test` writes it.
    #[test]
    fn the_guides_commands_page_is_the_programs() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../omnix-360-studio/docs/guide/commands-dailix.md");
        let page = super::commands_page();
        if std::env::var_os("OMNIX_BLESS").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), page, "{} is behind the commands: OMNIX_BLESS=1 cargo test writes it", path.display());
    }

    use super::*;

    #[test]
    fn the_schemas_are_made() {
        let ops = serde_json::to_string(&schemars::schema_for!(Op)).unwrap();
        assert!(ops.contains("\"move\"") && ops.contains("\"mark\""));
        let sheet = serde_json::to_string(&schemars::schema_for!(Sheet)).unwrap();
        assert!(sheet.contains("\"clips\"") && sheet.contains("\"maximum\":2"));
    }
}
