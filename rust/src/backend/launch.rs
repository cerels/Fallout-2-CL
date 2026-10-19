//! Reading launcher.ini, building fallout2-ce's argv, and replacing this process with it.

use std::io;
use std::path::Path;
use std::process::Command;

use super::ini_file::IniFile;
use super::paths::Paths;
use super::saves::read_saves;

pub fn launcher_prefs(paths: &Paths) -> io::Result<IniFile> {
    IniFile::open(paths.launcher_ini())
}

/// Command-line arguments for fallout2-ce for a launch mode.
pub fn game_args(paths: &Paths, mode: &str, slot: Option<u32>) -> Vec<String> {
    if mode == "slot" {
        if let Some(slot) = slot {
            return vec![format!("--dev-load-game={slot}")];
        }
    }
    if mode == "continue" {
        if let Some(save) = read_saves(paths).first() {
            return vec![format!("--dev-load-game={}", save.slot)];
        }
    }
    Vec::new()
}

/// Replace this process with the game binary. On Unix this is a true `exec` (same as
/// Python's `os.execv`): this process becomes the game, nothing of this launcher survives
/// to return to. Not unit-tested here for exactly that reason - calling it from a test would
/// replace the test runner itself with whatever "binary" the test pointed at.
pub fn exec_game(paths: &Paths, args: &[String]) -> io::Result<()> {
    std::env::set_current_dir(paths.game_dir())?;
    run(&paths.game_bin(), args)
}

#[cfg(unix)]
fn run(binary: &Path, args: &[String]) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    // exec() only returns if it failed to replace the process - success never reaches here.
    Err(Command::new(binary).args(args).exec())
}

#[cfg(not(unix))]
fn run(binary: &Path, args: &[String]) -> io::Result<()> {
    // Windows has no process-replacement syscall; spawn-and-exit is what Python's own
    // os.execv falls back to there too.
    let status = Command::new(binary).args(args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_games_dir(paths: &Paths) -> std::path::PathBuf {
        paths.game_dir().join("data").join("SAVEGAME")
    }

    fn write_minimal_save(paths: &Paths, slot: u32) {
        let dir = save_games_dir(paths).join(format!("slot{slot}"));
        std::fs::create_dir_all(&dir).unwrap();
        let mut head = vec![0u8; 131];
        head[0..17].copy_from_slice(b"FALLOUT SAVE FILE");
        std::fs::write(dir.join("SAVE.DAT"), head).unwrap();
    }

    #[test]
    fn slot_mode_with_a_slot_number() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(game_args(&paths, "slot", Some(3)), vec!["--dev-load-game=3"]);
    }

    #[test]
    fn slot_mode_without_a_slot_number_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(game_args(&paths, "slot", None), Vec::<String>::new());
    }

    #[test]
    fn continue_mode_picks_the_most_recent_save() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        write_minimal_save(&paths, 2);
        assert_eq!(game_args(&paths, "continue", None), vec!["--dev-load-game=2"]);
    }

    #[test]
    fn slot_mode_with_slot_zero_is_not_treated_as_absent() {
        // slot 0 is a legitimate save slot - `Option::Some(0)`, not "no slot given".
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(game_args(&paths, "slot", Some(0)), vec!["--dev-load-game=0"]);
    }

    #[test]
    fn continue_mode_picks_the_most_recently_modified_save_among_several() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        write_minimal_save(&paths, 1);
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_minimal_save(&paths, 5);

        assert_eq!(game_args(&paths, "continue", None), vec!["--dev-load-game=5"]);
    }

    #[test]
    fn continue_mode_with_no_saves_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(game_args(&paths, "continue", None), Vec::<String>::new());
    }

    #[test]
    fn unknown_mode_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(game_args(&paths, "menu", None), Vec::<String>::new());
    }

    #[test]
    fn launcher_prefs_opens_launcher_ini_under_the_launcher_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let prefs = launcher_prefs(&paths).unwrap();
        assert!(!prefs.exists());
    }
}
