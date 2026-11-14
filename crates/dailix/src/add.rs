//! Videos added to a trip (Omnix gap 74a): a file, or a span of it, copied
//! into the trip's footage under the day it was filmed, with a sidecar
//! that the contact sheet reads it by. The originals are only read. Not
//! the import from cards and phones (gap 74, `docs/INGEST.md`): no
//! analysis, no proxies; a clip's poster comes when it's first shown.

use crate::import::{Copy, copy};
use dailix_sheet::footage::{Technical, day_of, write_sidecar};
use dailix_sheet::Layout;
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error>;

/// A file to add, all of it or a span (seconds).
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub file: PathBuf,
    pub span: Option<(f64, f64)>,
}

/// `FILE`, `FILE@all` or `FILE@START-END`, the times as seconds, `m:ss` or
/// `h:mm:ss` (the user's list: `21 @0:00-0:56`).
/// A file whose name has an `@` in it (`clip@2x.mp4`) is taken whole
/// when it's there (the audit, 2026-11-14).
pub fn parse_pick(arg: &str) -> Result<Pick, String> {
    if Path::new(arg.trim()).is_file() {
        return Ok(Pick { file: arg.trim().into(), span: None });
    }
    let Some((file, span)) = arg.rsplit_once('@') else { return Ok(Pick { file: arg.trim().into(), span: None }) };
    let file = PathBuf::from(file.trim());
    let span = span.trim();
    if span.is_empty() || span.eq_ignore_ascii_case("all") {
        return Ok(Pick { file, span: None });
    }
    let (a, b) = span.split_once('-').ok_or_else(|| format!("«{span}»: a span is START-END, as 0:05-0:20"))?;
    let (a, b) = (seconds(a)?, seconds(b)?);
    if b <= a {
        return Err(format!("«{span}» ends before it starts"));
    }
    Ok(Pick { file, span: Some((a, b)) })
}

/// `s`, `m:ss` or `h:mm:ss` as seconds.
fn seconds(t: &str) -> Result<f64, String> {
    let not = || format!("«{t}» isn't a time");
    t.trim().split(':').try_fold(0.0, |acc, part| part.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| acc * 60.0 + v).ok_or_else(not))
}

/// The name a pick takes in the trip: its own, or with its span
/// (`DJI_0021_0m00-0m56.MP4`; tenths when they're there, `0m05.2`), so
/// spans of one file sit side by side and two spans never share a name.
pub fn target_name(pick: &Pick) -> String {
    let name = pick.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some((a, b)) = pick.span else { return name };
    // Rounded once, then split: 59.6 s was «0m60» (the audit).
    let mmss = |t: f64| {
        let tenths = (t * 10.0).round() as u64;
        let (m, s, d) = (tenths / 600, tenths % 600 / 10, tenths % 10);
        if d == 0 { format!("{m}m{s:02}") } else { format!("{m}m{s:02}.{d}") }
    };
    let path = Path::new(&name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    format!("{stem}_{}-{}{ext}", mmss(a), mmss(b))
}

/// What ffprobe says of a video.
fn probe(file: &Path) -> Result<Technical, Error> {
    let out = omnix::sound::tool("ffprobe").args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(file).output()?;
    if !out.status.success() {
        return Err(format!("{} isn't a video ffprobe can read", file.display()).into());
    }
    let j: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let streams = j["streams"].as_array().cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s["codec_type"] == "video").ok_or_else(|| format!("{} has no picture", file.display()))?;
    let rate = video["avg_frame_rate"].as_str().unwrap_or("0/1");
    let fps = rate.split_once('/').and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok().filter(|d| *d > 0.0)?)).unwrap_or(0.0);
    let number = |v: &serde_json::Value| v.as_str().and_then(|s| s.parse::<f64>().ok());
    Ok(Technical {
        duration_s: number(&j["format"]["duration"]).unwrap_or(0.0),
        fps: (fps * 1000.0).round() / 1000.0,
        resolution: format!("{}x{}", video["width"], video["height"]),
        codec: video["codec_name"].as_str().unwrap_or_default().to_owned(),
        size_bytes: number(&j["format"]["size"]).unwrap_or(0.0) as u64,
        has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        created_at: j["format"]["tags"]["creation_time"].as_str().map(str::to_owned),
    })
}

/// `pick`'s span of its file written to `to`, as recorded (no new
/// encoding: from the key frame before its start; the picture and the
/// sound only), whole or not at all.
fn cut(pick: &Pick, (a, b): (f64, f64), to: &Path) -> Result<(), Error> {
    let dir = to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let part = dir.join(format!(".{}.part.{}", to.file_name().unwrap_or_default().to_string_lossy(), to.extension().unwrap_or_default().to_string_lossy()));
    let status = omnix::sound::tool("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-ss", &a.to_string(), "-to", &b.to_string(), "-i"])
        .arg(&pick.file)
        // The picture and the sound: a camera's own data tracks (DJI's
        // gyro streams) don't go into an MP4 copied as they are.
        .args(["-map", "0:v:0", "-map", "0:a?", "-c", "copy", "-map_metadata", "0", "-movflags", "+faststart"])
        .arg(&part)
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("ffmpeg couldn't cut {}", pick.file.display()).into());
    }
    std::fs::File::open(&part)?.sync_all()?;
    std::fs::rename(&part, to)?;
    Ok(())
}

/// What became of one pick.
#[derive(Debug, Clone, PartialEq)]
pub enum What {
    /// Copied or cut in: its length.
    Added(f64),
    /// There already, left as it is.
    There,
    /// There already, without a sidecar: now it has one.
    Seen,
    /// Not added, and why.
    Failed(String),
}

/// One pick's line: its day and name in the trip (the file's, when it
/// failed before it had them), and what became of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub day: Option<String>,
    pub name: String,
    pub what: What,
}

impl std::fmt::Display for Said {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at = self.day.as_ref().map_or_else(|| self.name.clone(), |d| format!("{d}/{}", self.name));
        match &self.what {
            What::Added(secs) => write!(f, "{at}: {secs:.1} s added"),
            What::There => write!(f, "{at}: there already, left as it is"),
            What::Seen => write!(f, "{at}: there already; now the sheet sees it"),
            What::Failed(why) => write!(f, "{at}: not added: {why}"),
        }
    }
}

/// Adds `picks` to the trip at `root`, each under the day it was filmed
/// (or `day`, `YYYY-MM-DD`); a line said for each. One already there is
/// left as it is; one that fails says why and the rest go on (one bad
/// file lost the lines of those added before it: the audit, 2026-11-14).
pub fn add(root: &Path, picks: &[Pick], day: Option<&str>) -> Result<Vec<Said>, Error> {
    let layout = Layout::new(root);
    if !layout.is_project() {
        return Err(format!("not a Dailix trip: {}", root.display()).into());
    }
    if let Some(d) = day.filter(|d| !dailix_sheet::footage::is_day(d)) {
        return Err(format!("«{d}» isn't a day: give it as YYYY-MM-DD").into());
    }
    Ok(picks.iter().map(|pick| add_one(&layout, pick, day)).collect())
}

/// One pick added (`add`).
fn add_one(layout: &Layout, pick: &Pick, day: Option<&str>) -> Said {
    let name = pick.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let failed = |day: Option<String>, name: String, why: String| Said { day, name, what: What::Failed(why) };
    let source = match probe(&pick.file) {
        Ok(s) => s,
        Err(e) => return failed(None, pick.file.display().to_string(), e.to_string()),
    };
    let Some(on) = day.map(str::to_owned).or_else(|| day_of(&name, source.created_at.as_deref())) else {
        return failed(None, pick.file.display().to_string(), "no date in its name or in it; give the day (--day)".into());
    };
    let target = target_name(pick);
    let said = |what| Said { day: Some(on.clone()), name: target.clone(), what };
    match place(layout, pick, &on, &target, source) {
        Ok(what) => said(what),
        Err(e) => said(What::Failed(e.to_string())),
    }
}

/// `pick` copied or cut to `on/target` in the trip, its sidecar written.
fn place(layout: &Layout, pick: &Pick, on: &str, target: &str, source: Technical) -> Result<What, Error> {
    let to = layout.clip(&format!("{on}/{target}"));
    let side = dailix_sheet::footage::sidecar_path(layout, on, target);
    // A sidecar of another clip with this stem (A.MOV beside A.MP4):
    // writing ours would hide that one from the sheet (the audit).
    if let Some(other) = dailix_sheet::footage::sidecar_source(&side).filter(|s| s != target) {
        return Err(format!("{other} is there under the same name; rename this file first").into());
    }
    // There already: left as it is, its sidecar written if it has none
    // (copied in by hand, the Kos trip's phone days).
    if to.exists() {
        if side.exists() {
            return Ok(What::There);
        }
        let tech = Technical { created_at: source.created_at.clone(), ..probe(&to)? };
        write_sidecar(layout, on, target, &tech)?;
        return Ok(What::Seen);
    }
    match pick.span {
        Some(span) => cut(pick, span, &to)?,
        None => {
            copy(&Copy { from: pick.file.clone(), to: to.clone(), size: std::fs::metadata(&pick.file)?.len() })?;
        }
    }
    let tech = if pick.span.is_some() { Technical { created_at: source.created_at.clone(), ..probe(&to)? } } else { source };
    write_sidecar(layout, on, target, &tech)?;
    Ok(What::Added(tech.duration_s))
}

/// What `add` said, in a line: how many went in and under which days,
/// how many were there already, how many failed (the first's reason).
pub fn summary(said: &[Said]) -> String {
    let added: Vec<&Said> = said.iter().filter(|s| matches!(s.what, What::Added(_))).collect();
    let mut days: Vec<&str> = added.iter().filter_map(|s| s.day.as_deref()).collect();
    days.sort_unstable();
    days.dedup();
    let failed: Vec<&Said> = said.iter().filter(|s| matches!(s.what, What::Failed(_))).collect();
    let there = said.len() - added.len() - failed.len();
    let mut line = if added.is_empty() { "Nothing new added".to_owned() } else { format!("{} added: {}", crate::library::count(added.len(), "video"), days.join(", ")) };
    if there > 0 {
        line += &format!("; {} there already", crate::library::count(there, "video"));
    }
    if let Some(first) = failed.first() {
        line += &format!("; {} not: {first}", crate::library::count(failed.len(), "video"));
    }
    line + "."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_is_a_file_with_all_of_it_or_a_span() {
        assert_eq!(parse_pick("a/DJI_0042.MP4@all").unwrap(), Pick { file: "a/DJI_0042.MP4".into(), span: None });
        assert_eq!(parse_pick("DJI_0075.MP4 @1:09-1:20").unwrap().span, Some((69.0, 80.0)));
        assert_eq!(parse_pick("x.mp4@5-20.5").unwrap().span, Some((5.0, 20.5)));
        assert_eq!(parse_pick("x.mp4@1:00:00-1:00:10").unwrap().span, Some((3600.0, 3610.0)));
        assert!(parse_pick("x.mp4@0:20-0:05").is_err() && parse_pick("x.mp4@soon").is_err());
        assert!(parse_pick("x.mp4@nan-5").is_err() && parse_pick("x.mp4@inf-5").is_err() && parse_pick("x.mp4@-5-3").is_err());
        assert_eq!(parse_pick("plain.mp4").unwrap().span, None);
    }

    #[test]
    fn what_was_added_is_said_in_a_line() {
        let s = |day: &str, name: &str, what| Said { day: Some(day.into()), name: name.into(), what };
        // Days apart and back again: each said once, in order.
        let said = [s("2026-08-22", "b.MP4", What::Added(15.0)), s("2026-08-18", "a.mp4", What::Added(9.0)), s("2026-08-22", "c.MP4", What::Added(5.0)), s("2026-08-23", "d.MP4", What::There)];
        assert_eq!(summary(&said), "3 videos added: 2026-08-18, 2026-08-22; 1 video there already.");
        assert_eq!(summary(&said[3..]), "Nothing new added; 1 video there already.");
        let bad = Said { day: None, name: "x.mp4".into(), what: What::Failed("no picture".into()) };
        assert_eq!(summary(&[said[1].clone(), bad.clone()]), "1 video added: 2026-08-18; 1 video not: x.mp4: not added: no picture.");
        assert_eq!(said[0].to_string(), "2026-08-22/b.MP4: 15.0 s added");
    }

    #[test]
    fn a_span_is_named_by_its_times_beside_the_file() {
        let pick = |span| Pick { file: "/f/DJI_0021.MP4".into(), span };
        assert_eq!(target_name(&pick(Some((0.0, 56.0)))), "DJI_0021_0m00-0m56.MP4");
        assert_eq!(target_name(&pick(Some((69.0, 80.0)))), "DJI_0021_1m09-1m20.MP4");
        assert_eq!(target_name(&pick(None)), "DJI_0021.MP4");
        // Rounded once: never «0m60»; tenths keep two spans apart.
        assert_eq!(target_name(&pick(Some((59.96, 119.7)))), "DJI_0021_1m00-1m59.7.MP4");
        assert_ne!(target_name(&pick(Some((5.2, 20.0)))), target_name(&pick(Some((5.4, 20.0)))));
    }
}
