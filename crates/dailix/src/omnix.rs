//! Opening an edit in Omnix: finding its binary and starting it on the
//! file, in a session of its own so it stays open when Dailix closes.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Omnix's binary's name (gap 76 renames it).
const BIN: &str = "rosmo360";

/// Omnix's binary: `OMNIX_BIN`, else on PATH, else a release build beside
/// this one's repo (`../omnix-360-studio`, for working on both).
pub fn locate() -> Option<PathBuf> {
    locate_from(std::env::var_os("OMNIX_BIN").map(PathBuf::from), std::env::var_os("PATH"), std::env::current_exe().ok())
}

/// `locate`, from what it reads of the process: the variable, PATH and
/// this binary's place (`<repo>/target/release/dailix`).
fn locate_from(env_bin: Option<PathBuf>, path: Option<OsString>, exe: Option<PathBuf>) -> Option<PathBuf> {
    let name = format!("{BIN}{}", std::env::consts::EXE_SUFFIX);
    let on_path = || path.as_ref().and_then(|p| std::env::split_paths(p).map(|d| d.join(&name)).find(|f| f.is_file()));
    let beside = || {
        let repo = exe.as_deref()?.parent()?.parent()?.parent()?;
        Some(repo.parent()?.join("omnix-360-studio/target/release").join(&name)).filter(|f| f.is_file())
    };
    env_bin.filter(|f| f.is_file()).or_else(on_path).or_else(beside)
}

/// Opens `edit` in Omnix; what went wrong, said plainly.
pub fn open(edit: &Path) -> Result<(), String> {
    let bin = locate().ok_or_else(|| format!("Written {}, but Omnix wasn't found to open it (OMNIX_BIN)", edit.display()))?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("play").arg(edit).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.spawn().map(drop).map_err(|e| format!("{}: {e}", bin.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omnix_is_looked_for_in_the_variable_then_path_then_beside() {
        let base = std::env::temp_dir().join(format!("dailix-omnix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let name = format!("{BIN}{}", std::env::consts::EXE_SUFFIX);
        let file = |p: PathBuf| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
            p
        };
        let set = file(base.join("set").join(&name));
        let on_path = file(base.join("bin").join(&name));
        let beside = file(base.join("omnix-360-studio/target/release").join(&name));
        let exe = Some(base.join("dailix/target/release/dailix"));
        let path = Some(std::env::join_paths([base.join("empty"), base.join("bin")]).unwrap());
        assert_eq!(locate_from(Some(set.clone()), path.clone(), exe.clone()), Some(set));
        assert_eq!(locate_from(Some(base.join("gone")), path, exe.clone()), Some(on_path), "a variable naming nothing is passed over");
        assert_eq!(locate_from(None, None, exe), Some(beside));
        assert_eq!(locate_from(None, None, None), None);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
