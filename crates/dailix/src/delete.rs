//! A trip or one of its edits moved to the system's trash (the user,
//! 2026-11-14): restorable from there, never deleted outright, after a
//! question that says what goes. The originals in the sources are never
//! touched: a trip holds copies of them.

use std::path::{Path, PathBuf};

/// What's to go.
#[derive(Debug, Clone, PartialEq)]
pub enum Doomed {
    /// A trip, by its folder.
    Trip(PathBuf),
    /// An edit, by its `edit.json`: its folder (`videos/<name>/`, the edit
    /// and its exports) and its card's frames.
    Edit(PathBuf),
}

/// What a trash takes: the files and their bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub files: u64,
    pub bytes: u64,
}

impl Doomed {
    /// The folders it moves to the trash, the first the one it's named by.
    pub fn folders(&self) -> Vec<PathBuf> {
        match self {
            Doomed::Trip(root) => vec![root.clone()],
            Doomed::Edit(edit) => {
                let folder = edit.parent().map(Path::to_path_buf).unwrap_or_default();
                let card = omnix::cards::strip_path(edit).filter(|c| c.exists());
                std::iter::once(folder).chain(card).collect()
            }
        }
    }

    /// Its name as the question says it.
    pub fn name(&self) -> String {
        match self {
            Doomed::Trip(root) => dailix_sheet::project::title(root),
            Doomed::Edit(edit) => dailix_sheet::project::edit_name(edit),
        }
    }
}

/// The files under `path` and their bytes (a file: itself).
pub fn size(path: &Path) -> Size {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return Size::default() };
    if !meta.is_dir() {
        return Size { files: 1, bytes: meta.len() };
    }
    std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| size(&e.path())).fold(Size::default(), |a, b| Size { files: a.files + b.files, bytes: a.bytes + b.bytes })
}

/// The question's words for `doomed` taking `size`.
pub fn question(doomed: &Doomed, size: Size) -> (String, String) {
    let amount = if size.bytes >= 1_000_000_000 { format!("{:.1} GB", size.bytes as f64 / 1e9) } else { format!("{:.0} MB", (size.bytes as f64 / 1e6).max(1.0)) };
    match doomed {
        Doomed::Trip(_) => (
            format!("Are you sure you want to move the trip «{}» to the trash?", doomed.name()),
            format!("Its footage copies, marks, voices and edits go with it: {}, {amount}. The originals in the sources stay as they are. The trash can give it back.", crate::library::count(size.files as usize, "file")),
        ),
        Doomed::Edit(_) => (
            format!("Are you sure you want to move the edit «{}» to the trash?", doomed.name()),
            format!("The edit and its exports go: {}, {amount}. Close it in Omnix first. The trash can give it back.", crate::library::count(size.files as usize, "file")),
        ),
    }
}

/// Moves `doomed`'s folders to the system's trash.
pub fn trash(doomed: &Doomed) -> Result<(), String> {
    for folder in doomed.folders() {
        trash::delete(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_question_says_what_goes_and_that_the_originals_stay() {
        let (title, said) = question(&Doomed::Trip("/p/2026-11_tutorial".into()), Size { files: 12, bytes: 2_400_000_000 });
        assert!(title.starts_with("Are you sure you want to move the trip «") && said.contains("12 files, 2.4 GB") && said.contains("originals in the sources stay"), "{said}");
        let (title, said) = question(&Doomed::Edit("/p/t/videos/Day one/edit.json".into()), Size { files: 3, bytes: 10 });
        assert!(title.starts_with("Are you sure you want to move the edit «") && said.contains("3 files, 1 MB") && said.contains("Close it in Omnix"), "{title} {said}");
    }

    #[test]
    fn an_edit_takes_its_folder_and_a_trip_its_own() {
        assert_eq!(Doomed::Edit("/p/t/videos/a/edit.json".into()).folders()[0], PathBuf::from("/p/t/videos/a"));
        assert_eq!(Doomed::Trip("/p/t".into()).folders(), [PathBuf::from("/p/t")]);
    }

    #[test]
    fn a_folder_is_counted_file_by_file() {
        let root = std::env::temp_dir().join(format!("dailix-size-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/x"), [0u8; 10]).unwrap();
        std::fs::write(root.join("y"), [0u8; 5]).unwrap();
        assert_eq!(size(&root), Size { files: 2, bytes: 15 });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
