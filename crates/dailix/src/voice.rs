//! Making a clip's cleaned voice (Omnix gap 77): a recipe of
//! `dailix_sheet::voice` carried out with `deep-filter` (DeepFilterNet, a
//! program of its own) and ffmpeg (as Omnix finds it), from the original
//! into the clip's voice folder, and its loudness measured with Omnix's
//! meter, so Omnix can set the voice's level without making it again.

use dailix_sheet::Layout;
use dailix_sheet::voice::{self, Kind, Made, Recipe};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What went wrong, said plainly.
pub type Res<T> = Result<T, String>;

/// Where Dailix keeps the programs it runs that aren't the system's: its
/// config folder's `tools`.
pub fn tools_dir() -> Option<PathBuf> {
    omnix::app_dir(omnix::AppDir::Config, omnix::DAILIX).map(|d| d.join("tools"))
}

/// `deep-filter`: on the PATH, else in Dailix's tools folder.
pub fn deep_filter() -> Option<PathBuf> {
    let file = format!("deep-filter{}", std::env::consts::EXE_SUFFIX);
    let on_path = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(&file)).find(|f| f.is_file()));
    on_path.or_else(|| tools_dir().map(|d| d.join(&file)).filter(|f| f.is_file()))
}

/// Runs `cmd`; its own words when it fails.
fn run(mut cmd: Command, what: &str) -> Res<()> {
    let out = cmd.stdin(std::process::Stdio::null()).output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(if out.stderr.is_empty() { &out.stdout } else { &out.stderr }).trim().chars().take(300).collect::<String>();
    Err(format!("{what}: {said}"))
}

/// A folder of its own for one making, gone when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Res<Scratch> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        // In Dailix's TMP place when one is set: a long clip's sound is
        // hundreds of MB, too much for a /tmp held in memory (audit A15).
        let base = crate::settings::Settings::load().tmp.filter(|d| d.is_dir()).unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("dailix-voice-{}-{nanos}", std::process::id()));
        // `deep-filter` writes under the input's name into its `-o` folder:
        // given the input's own folder, it overwrites the input and says
        // nothing (footage-agent lost test files so). In and out apart.
        for sub in ["in", "out"] {
            std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("a scratch folder: {e}"))?;
        }
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Clip `key`'s voice made by `recipe` from `source` (its original), into
/// its voice folder, measured and recorded; its file.
pub fn make(layout: &Layout, key: &str, source: &Path, recipe: &Recipe) -> Res<PathBuf> {
    let dir = voice::dir(layout, key).ok_or_else(|| format!("not a clip's key: {key}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let scratch = Scratch::new()?;
    let wav = scratch.0.join("in/a.wav");
    let done = scratch.0.join("out/a.wav");
    let mut take = omnix::sound::tool("ffmpeg");
    take.args(["-nostdin", "-y", "-loglevel", "error", "-i"]).arg(source).args(["-map", "0:a:0", "-ac", "2", "-ar", "48000", "-c:a", "pcm_s16le"]).arg(&wav);
    run(take, "taking the sound out")?;
    match recipe.kind {
        Kind::Dnn => {
            let tool = deep_filter().ok_or("deep-filter isn't here: put DeepFilterNet's deep-filter on the PATH or in Dailix's tools folder")?;
            let mut clean = Command::new(tool);
            clean.args(recipe.args).arg("-o").arg(scratch.0.join("out")).arg(&wav);
            run(clean, "deep-filter")?;
        }
        Kind::Ffmpeg => {
            let mut filter = omnix::sound::tool("ffmpeg");
            filter.args(["-nostdin", "-y", "-loglevel", "error", "-i"]).arg(&wav).args(["-af", recipe.args[0]]).arg(&done);
            run(filter, "the filters")?;
        }
    }
    if !done.is_file() {
        return Err(format!("{}: nothing came out", recipe.name));
    }
    // Written whole or not at all: a half file would pass for a voice, and
    // a failed one leaves nothing behind (audit A15).
    let track = voice::track(&dir, recipe.name);
    let part = dir.join(format!(".{}.part", recipe.name));
    let mut flac = omnix::sound::tool("ffmpeg");
    flac.args(["-nostdin", "-y", "-loglevel", "error", "-i"]).arg(&done).args(["-c:a", "flac", "-compression_level", "5", "-f", "flac"]).arg(&part);
    if let Err(e) = run(flac, "writing the FLAC").and_then(|_| std::fs::rename(&part, &track).map_err(|e| format!("{}: {e}", track.display()))) {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    // Recorded even if measuring fails (its loudness then unknown, for
    // `measure` to fill): a voice without an entry was listed but never
    // measured (audit A10).
    let mut entry = Made::stamp(recipe, source).map_err(|e| format!("{}: {e}", source.display()))?;
    let measured = omnix::sound::loudness(&track).map_err(|e| format!("measuring: {e:#}"));
    entry.lufs = measured.clone().ok().flatten();
    voice::record(&dir, recipe.name, entry).map_err(|e| format!("{}: {e}", dir.display()))?;
    measured?;
    Ok(track)
}

/// The loudness of clip `key`'s variants not measured yet (made by
/// footage-agent's sheet, or measured in vain): those without an entry
/// too, given one if their recipe is known (audit A10). How many were.
pub fn measure(layout: &Layout, key: &str) -> Res<usize> {
    let Some(dir) = voice::dir(layout, key) else { return Ok(0) };
    let had = voice::made(&dir);
    let mut measured = 0;
    for name in voice::variants(&dir) {
        let entry = match had.get(&name) {
            Some(m) if m.lufs.is_some() => continue,
            Some(m) => m.clone(),
            None => match voice::recipe(&name) {
                Some(r) => Made::stamp(r, &layout.clip(key)).map_err(|e| format!("{key}: {e}"))?,
                None => continue,
            },
        };
        let lufs = omnix::sound::loudness(&voice::track(&dir, &name)).map_err(|e| format!("measuring {name}: {e:#}"))?;
        voice::record(&dir, &name, Made { lufs, ..entry }).map_err(|e| format!("{}: {e}", dir.display()))?;
        measured += 1;
    }
    Ok(measured)
}

/// Clip `key`'s original.
pub fn source(layout: &Layout, key: &str) -> Res<PathBuf> {
    let (footage, _) = dailix_sheet::footage::read(layout).map_err(|e| e.to_string())?;
    footage.days.into_iter().flat_map(|d| d.clips).find(|c| c.key == key).map(|c| c.path).ok_or_else(|| format!("no clip {key} in {}", layout.root.display()))
}

/// The project's voices as `dailix voice list` says them.
pub fn describe(layout: &Layout) -> Res<String> {
    let (footage, _) = dailix_sheet::footage::read(layout).map_err(|e| e.to_string())?;
    let sheet = dailix_sheet::store::Store::new(layout.sheet()).read().and_then(|d| d.sheet().map_err(dailix_sheet::store::StoreError::Sheet)).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for clip in footage.days.iter().flat_map(|d| &d.clips) {
        let Some(dir) = voice::dir(layout, &clip.key) else { continue };
        let made = voice::made(&dir);
        let names = voice::variants(&dir);
        if names.is_empty() {
            continue;
        }
        let chosen = sheet.clips.get(&clip.key).and_then(|c| c.audio.as_deref());
        let said: Vec<String> = names
            .iter()
            .map(|n| {
                let lufs = made.get(n).and_then(|m| m.lufs).map_or(String::new(), |l| format!(" {l:.1} LUFS"));
                format!("{}{n}{lufs}", if Some(n.as_str()) == chosen { "*" } else { "" })
            })
            .collect();
        out += &format!("{}: {}\n", clip.key, said.join(", "));
    }
    out += &format!("recipes: {}\n", voice::RECIPES.map(|r| format!("{} ({})", r.name, r.note)).join("; "));
    Ok(out)
}

/// Where clip `key` holds speech (gap 83): the cleaned voice it's found in
/// (`voice::speech_source`, the marking's choice first) and its stretches.
pub fn speech(layout: &Layout, key: &str) -> Res<(String, Vec<(f64, f64)>)> {
    let dir = voice::dir(layout, key).ok_or_else(|| format!("not a clip's key: {key}"))?;
    let sheet = dailix_sheet::store::Store::new(layout.sheet()).read().and_then(|d| d.sheet().map_err(dailix_sheet::store::StoreError::Sheet)).map_err(|e| e.to_string())?;
    let names = voice::variants(&dir);
    let chosen = sheet.clips.get(key).and_then(|c| c.audio.as_deref());
    let from = voice::speech_source(&names, chosen).ok_or_else(|| format!("{key} has no cleaned voice to look in: dailix voice make first"))?;
    let spans = omnix::sound::speech(&voice::track(&dir, from)).map_err(|e| format!("{e:#}"))?;
    Ok((from.to_owned(), spans))
}
