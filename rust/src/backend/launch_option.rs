//! The exact Steam Launch Options line for this OS.

use std::path::{Path, PathBuf};

/// The file to point Steam - or a non-Steam shortcut, see `steam_shortcuts.rs` once it's
/// ported - at: this launcher itself.
///
/// Python had a frozen-vs-source-checkout split here: a PyInstaller build targeted its own
/// exe directly, while a source checkout targeted `launch.sh`/`launch.bat`, small wrapper
/// scripts whose only job was finding and running `f2launcher.py` with the right Python
/// interpreter. Rust has no such split - every build is already a single, directly-runnable
/// executable - so there's nothing left for a wrapper script to do, and this is just the
/// path to the running exe.
pub fn launcher_target() -> PathBuf {
    std::env::current_exe().expect("could not locate the running executable")
}

/// Return (the Launch Options string to paste into Steam, whether it's verified).
///
/// The target is always where this launcher itself lives, not the active campaign's root,
/// which can be a subfolder underneath it. That's what makes it possible to paste a
/// *different* Steam entry's launch options (e.g. Fallout 1's, to force Et Tu) at the same
/// launcher: `force_campaign` adds the flag that picks the campaign on startup, overriding
/// "remember last campaign" for that one launch.
///
/// Linux and macOS both run Launch Options through a shell that treats `#` as a comment, so
/// `<target> # %command%` drops Steam's own command entirely. cmd.exe (Windows) has no
/// equivalent for that mid-line, so the Windows line is unverified best-effort: Steam may run
/// the target and then still launch the game afterward, rather than replacing it.
pub fn steam_launch_option(force_campaign: Option<&str>) -> (String, bool) {
    format_launch_option(&launcher_target(), force_campaign, cfg!(windows))
}

/// Pure formatting logic, separated from `steam_launch_option` only so tests can exercise
/// both the Windows and non-Windows line shapes regardless of which OS is running them.
fn format_launch_option(target: &Path, force_campaign: Option<&str>, windows: bool) -> (String, bool) {
    let args = force_campaign.map(|c| format!(" --campaign={c}")).unwrap_or_default();
    let target = target.display();
    if windows {
        (format!("\"{target}\"{args} %command%"), false)
    } else {
        (format!("\"{target}\"{args} # %command%"), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_line_comments_out_steams_own_command_and_is_verified() {
        let (line, verified) = format_launch_option(Path::new("/game/f2launcher"), None, false);
        assert_eq!(line, "\"/game/f2launcher\" # %command%");
        assert!(verified);
    }

    #[test]
    fn windows_line_cannot_drop_steams_command_and_is_unverified() {
        let (line, verified) = format_launch_option(Path::new(r"C:\game\f2launcher.exe"), None, true);
        assert_eq!(line, "\"C:\\game\\f2launcher.exe\" %command%");
        assert!(!verified);
    }

    #[test]
    fn force_campaign_adds_the_flag_before_the_rest_of_the_line() {
        let (line, _) = format_launch_option(Path::new("/game/f2launcher"), Some("ettu"), false);
        assert_eq!(line, "\"/game/f2launcher\" --campaign=ettu # %command%");
    }
}
