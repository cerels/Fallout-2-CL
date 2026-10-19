//! Opening a file, folder or URL in whatever the desktop considers the default handler -
//! the equivalent of Qt's `QDesktopServices::openUrl`, without pulling in a crate for it
//! (`docs/adr/0005-boring-rust.md`): every desktop OS already ships a one-shot command for
//! this.

use std::path::Path;
use std::process::Command;

/// Open a local path (file or folder) or a `http(s)://` URL. Best-effort: nothing here is
/// essential to the launcher's own job, so a failure to spawn the opener is silently
/// ignored, same as the old Qt build treated `QDesktopServices::openUrl`'s return value.
pub fn open(target: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(target).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(target).spawn();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", target]).spawn();
    }
}

pub fn open_path(path: &Path) {
    open(&path.to_string_lossy());
}
