//! Opening an edit in Omnix (Omnix's `sibling` finds and starts it).

use std::path::Path;

/// Opens `edit` in Omnix; what went wrong, said plainly.
pub fn open(edit: &Path) -> Result<(), String> {
    omnix::sibling::open_in_omnix(edit)
}

/// Shows `path` in the file manager: a folder as it is, a file's folder.
pub fn show(path: &Path) -> Result<(), String> {
    let folder = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    std::process::Command::new("xdg-open").arg(folder).spawn().map(|_| ()).map_err(|e| format!("couldn't open the file manager: {e}"))
}
