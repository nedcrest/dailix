//! `rushix install`: the icon and a desktop entry, in the menu and on the
//! desktop, starting this very binary. Linux only for now (freedesktop's
//! icon theme and `.desktop` files); elsewhere it says so.

/// The icon: a film strip (rushes), its middle frame picked.
pub const ICON: &str = include_str!("../assets/rushix.svg");

#[cfg(target_os = "linux")]
pub fn install() -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("no HOME")?);
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let exe = std::env::current_exe()?;
    let mut written = Vec::new();

    let icons = data.join("icons/hicolor/scalable/apps");
    std::fs::create_dir_all(&icons)?;
    std::fs::write(icons.join("rushix.svg"), ICON)?;
    written.push(icons.join("rushix.svg"));

    // StartupWMClass: the window's class (its application id), so the
    // dock shows this entry's icon for the open window.
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Rushix\nComment=Відзнятий матеріал: переглянути й відібрати\nExec=\"{}\"\nIcon=rushix\nTerminal=false\nCategories=AudioVideo;Video;\nStartupWMClass=rushix\n",
        exe.display()
    );
    let desktop = std::process::Command::new("xdg-user-dir").arg("DESKTOP").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).map(|s| PathBuf::from(s.trim())).filter(|p| p.is_dir()).unwrap_or_else(|| home.join("Desktop"));
    for dir in [data.join("applications"), desktop] {
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("rushix.desktop");
        std::fs::write(&file, &entry)?;
        // GNOME launches a desktop file only once it's executable and
        // marked trusted (the system's gio: a Homebrew one lacks gvfs).
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        let gio = if std::path::Path::new("/usr/bin/gio").exists() { "/usr/bin/gio" } else { "gio" };
        let _ = std::process::Command::new(gio).args(["set", "-t", "string"]).arg(&file).args(["metadata::trusted", "true"]).status();
        written.push(file);
    }
    let _ = std::process::Command::new("gtk-update-icon-cache").args(["-f", "-t"]).arg(data.join("icons/hicolor")).status();
    Ok(written)
}

#[cfg(not(target_os = "linux"))]
pub fn install() -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    Err("rushix install makes a Linux desktop entry; on this system, pin the binary yourself".into())
}
