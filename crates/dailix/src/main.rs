//! Dailix: bringing footage in and choosing it. Only the command line for
//! now; the window comes next (`docs/CONTEXT.md`).

mod app;
mod footage;
mod import;
mod install;
mod settings;

use clap::{Parser, Subcommand};
use dailix_sheet::{Doc, MarkKind, Op, Rating, Sheet, Store};
use std::path::PathBuf;
use std::process::ExitCode;

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
    /// The contact sheet's marking (a project's `dailix/sheet.json`).
    #[command(subcommand)]
    Sheet(SheetCmd),
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
    /// A footage-agent project made into a Dailix one, in the projects
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

#[derive(Clone, Copy, clap::ValueEnum)]
enum SchemaOf {
    Ops,
    Sheet,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // The window, unless a command was given.
    let open = match &cli.command {
        None => Some(None),
        Some(Command::Open { project }) => Some(Some(project.clone())),
        _ => None,
    };
    if let Some(project) = open {
        return match app::run(project) {
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

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = match cli.command.expect("the window is opened in main") {
        Command::Open { .. } => unreachable!("the window is opened in main"),
        Command::Sheet(cmd) => cmd,
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
            return Ok(());
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
                let pct = if total == 0 { 100 } else { done * 100 / total };
                if pct >= shown + 5 || done == total {
                    shown = pct;
                    println!("{pct:>3} %  {:.1} of {:.1} GB", gb(done), gb(total));
                }
            })?;
            println!("done: {}", plan.layout.root.display());
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
            let doc = Doc::parse(&std::fs::read_to_string(notes)?)?;
            for p in doc.check() {
                println!("{p}");
            }
        }
        SheetCmd::Describe { notes, day } => {
            let sheet = Doc::parse(&std::fs::read_to_string(notes)?)?.sheet()?;
            print!("{}", describe(&sheet, day.as_deref()));
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

/// Seconds as m:ss.s.
fn clock(t: f64) -> String {
    let tenths = (t.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// The sheet as text, a day at a time: the day's note, then each clip
/// rated, noted or marked, then its marks by time.
fn describe(sheet: &Sheet, only: Option<&str>) -> String {
    let mut out = String::new();
    let mut days: Vec<&str> = sheet.days.keys().map(String::as_str).chain(sheet.clips.keys().filter_map(|k| k.split_once('/').map(|(d, _)| d))).collect();
    days.sort_unstable();
    days.dedup();
    for day in days.into_iter().filter(|d| only.is_none_or(|o| o == *d)) {
        let note = sheet.days.get(day).map(|d| d.note.replace('\n', " / ")).unwrap_or_default();
        out.push_str(&format!("{day}  {note}\n"));
        for (key, clip) in sheet.clips.iter().filter(|(k, _)| k.split_once('/').is_some_and(|(d, _)| d == day)) {
            if clip.rating.is_none() && clip.note.is_empty() && clip.marks.is_empty() {
                continue;
            }
            let file = key.split_once('/').map_or(key.as_str(), |(_, f)| f);
            let rating = match clip.rating {
                Some(Rating::Take) => "✓",
                Some(Rating::Think) => "?",
                Some(Rating::Junk) => "✕",
                None => "·",
            };
            let voice = clip.audio.as_ref().map(|a| format!("  voice {a}")).unwrap_or_default();
            out.push_str(&format!("  {file}  {rating}  {}{voice}\n", clip.note.replace('\n', " / ")));
            let mut marks: Vec<_> = clip.marks.iter().collect();
            marks.sort_by(|a, b| a.t.total_cmp(&b.t));
            for m in marks {
                let span = match (m.kind, m.end) {
                    (MarkKind::Segment, Some(end)) => format!("{}–{}", clock(m.t), clock(end)),
                    (MarkKind::Flag, _) => format!("{} flag", clock(m.t)),
                    _ => clock(m.t),
                };
                let role = m.role.as_deref().unwrap_or("-");
                out.push_str(&format!("    {}  {span}  {role}  {}\n", m.id.as_deref().unwrap_or("-"), m.note.replace('\n', " / ")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_reads_as_a_line_per_day_clip_and_mark() {
        let doc = Doc::parse(
            r#"{"days": {"2030-05-01": {"note": "море"}}, "clips": {
                "2030-05-01/a.mp4": {"rating": 2, "note": "кіт", "audio": "dnn6", "marks": [
                    {"t": 26, "kind": "flag", "note": "мем", "role": "cut", "id": "m2"},
                    {"t": 1.5, "end": 4, "role": "broll", "note": "фон", "id": "m1"}]},
                "2030-05-01/b.mp4": {"rating": null, "note": "", "marks": []},
                "2030-05-02/c.mp4": {"rating": 0}}}"#,
        )
        .unwrap();
        let text = describe(&doc.sheet().unwrap(), None);
        assert_eq!(
            text,
            "2030-05-01  море\n  a.mp4  ✓  кіт  voice dnn6\n    m1  0:01.5–0:04.0  broll  фон\n    m2  0:26.0 flag  cut  мем\n2030-05-02  \n  c.mp4  ✕  \n"
        );
        assert_eq!(describe(&doc.sheet().unwrap(), Some("2030-05-02")), "2030-05-02  \n  c.mp4  ✕  \n");
    }

    #[test]
    fn the_schemas_are_made() {
        let ops = serde_json::to_string(&schemars::schema_for!(Op)).unwrap();
        assert!(ops.contains("\"move\"") && ops.contains("\"mark\""));
        let sheet = serde_json::to_string(&schemars::schema_for!(Sheet)).unwrap();
        assert!(sheet.contains("\"clips\"") && sheet.contains("\"maximum\":2"));
    }
}
