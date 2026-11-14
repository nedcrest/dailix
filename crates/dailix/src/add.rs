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
pub fn parse_pick(arg: &str) -> Result<Pick, String> {
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
    t.trim().split(':').try_fold(0.0, |acc, part| part.parse::<f64>().map(|v| acc * 60.0 + v).map_err(|_| format!("«{t}» isn't a time")))
}

/// The name a pick takes in the trip: its own, or with its span
/// (`DJI_0021_0m00-0m56.MP4`), so spans of one file sit side by side.
pub fn target_name(pick: &Pick) -> String {
    let name = pick.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some((a, b)) = pick.span else { return name };
    let mmss = |t: f64| format!("{}m{:02}", (t / 60.0) as u64, (t % 60.0).round() as u64);
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
/// encoding: from the key frame before its start), whole or not at all.
fn cut(pick: &Pick, (a, b): (f64, f64), to: &Path) -> Result<(), Error> {
    let dir = to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let part = dir.join(format!(".{}.part.{}", to.file_name().unwrap_or_default().to_string_lossy(), to.extension().unwrap_or_default().to_string_lossy()));
    let status = omnix::sound::tool("ffmpeg")
        .args(["-v", "error", "-y", "-ss", &a.to_string(), "-to", &b.to_string(), "-i"])
        .arg(&pick.file)
        .args(["-map", "0", "-c", "copy", "-map_metadata", "0", "-movflags", "+faststart"])
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

/// Adds `picks` to the trip at `root`, each under the day it was filmed
/// (or `day`); a line said for each. One already there is left as it is.
pub fn add(root: &Path, picks: &[Pick], day: Option<&str>) -> Result<Vec<String>, Error> {
    let layout = Layout::new(root);
    if !layout.is_project() {
        return Err(format!("not a Dailix trip: {}", root.display()).into());
    }
    let mut said = Vec::new();
    for pick in picks {
        let source = probe(&pick.file)?;
        let name = pick.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let on = day.map(str::to_owned).or_else(|| day_of(&name, source.created_at.as_deref())).ok_or_else(|| format!("{}: no date in its name or in it; give the day (--day)", pick.file.display()))?;
        let target = target_name(pick);
        let to = layout.clip(&format!("{on}/{target}"));
        if to.exists() {
            said.push(format!("{on}/{target}: there already, left as it is"));
            continue;
        }
        match pick.span {
            Some(span) => cut(pick, span, &to)?,
            None => {
                copy(&Copy { from: pick.file.clone(), to: to.clone(), size: std::fs::metadata(&pick.file)?.len() })?;
            }
        }
        let tech = if pick.span.is_some() { Technical { created_at: source.created_at.clone(), ..probe(&to)? } } else { source };
        write_sidecar(&layout, &on, &target, &tech)?;
        said.push(format!("{on}/{target}: {:.1} s added", tech.duration_s));
    }
    Ok(said)
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
        assert_eq!(parse_pick("plain.mp4").unwrap().span, None);
    }

    #[test]
    fn a_span_is_named_by_its_times_beside_the_file() {
        let pick = |span| Pick { file: "/f/DJI_0021.MP4".into(), span };
        assert_eq!(target_name(&pick(Some((0.0, 56.0)))), "DJI_0021_0m00-0m56.MP4");
        assert_eq!(target_name(&pick(Some((69.0, 80.0)))), "DJI_0021_1m09-1m20.MP4");
        assert_eq!(target_name(&pick(None)), "DJI_0021.MP4");
    }
}
