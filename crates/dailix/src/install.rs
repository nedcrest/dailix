//! `dailix install`: the icon and a desktop entry, in the menu and on the
//! desktop, starting this very binary (`omnix::desktop` writes them).

/// The icon: a film strip (rushes), its middle frame picked.
pub const ICON: &str = include_str!("../assets/dailix.svg");

/// The menu's and the desktop's entry and the icon, written by Omnix's
/// library (one installer for both programs).
pub fn install() -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    // StartupWMClass: the window's class (its application id), so the
    // dock shows this entry's icon for the open window.
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Dailix\nComment=Відзнятий матеріал: переглянути й відібрати\nExec=\"{}\"\nIcon=dailix\nTerminal=false\nCategories=AudioVideo;Video;\nStartupWMClass=dailix\n",
        std::env::current_exe()?.display()
    );
    omnix::desktop::install("dailix", &entry, ICON)
}
