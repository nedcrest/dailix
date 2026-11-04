//! The command line end to end: ops applied to a sheet file, then read
//! back as text; a refused run writes nothing and says why.

use std::process::Command;

fn dailix(args: &[&str], stdin: Option<&str>) -> (bool, String, String) {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_dailix")).args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    if let Some(input) = stdin {
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    (out.status.success(), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn ops_applied_then_described() {
    let dir = std::env::temp_dir().join(format!("dailix-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let notes = dir.join("notes.json");
    std::fs::write(&notes, "{}").unwrap();
    let notes = notes.to_str().unwrap();

    let ops = r#"[
        {"area": "day", "op": "note", "day": "2030-05-01", "note": "море"},
        {"area": "mark", "op": "add", "clip": "2030-05-01/a.mp4", "t": 1.5, "end": 4, "role": "broll", "note": "фон"},
        {"area": "clip", "op": "rate", "clip": "2030-05-01/a.mp4", "rating": 2}
    ]"#;
    let (ok, out, err) = dailix(&["sheet", "apply", notes, "-"], Some(ops));
    assert!(ok, "{err}");
    assert_eq!(out, "1: done\n2: made m1\n3: done\n");

    let (ok, out, _) = dailix(&["sheet", "describe", notes], None);
    assert!(ok);
    assert_eq!(out, "2030-05-01  море\n  a.mp4  ✓  \n    m1  0:01.5–0:04.0  broll  фон\n");

    // Refused: nothing written, and the reason on stderr.
    let before = std::fs::read_to_string(notes).unwrap();
    let (ok, _, err) = dailix(&["sheet", "apply", notes, "-"], Some(r#"{"area": "mark", "op": "move", "mark": "m1", "t": 3, "end": 3.1}"#));
    assert!(!ok);
    assert!(err.contains("op 1: refused") && err.contains("shorter than"), "{err}");
    assert_eq!(std::fs::read_to_string(notes).unwrap(), before);

    let (ok, out, _) = dailix(&["sheet", "check", notes], None);
    assert!(ok && out.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}
