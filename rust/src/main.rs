//! Entry point: decide the active campaign and whether to show the launcher window (same
//! rules as the Python build's `f2launcher.py main()` - `--ui`/`--no-ui`/`--campaign=<id>`,
//! and the player's own "show this launcher" preference), then either start the game
//! directly or run the egui app and start it afterward with whatever the player chose.
//!
//! The GUI itself lives in `gui/` - see `docs/adr/0004-rust-rewrite.md` for why egui, and
//! `gui/mod.rs` for how it's organized.

// No console window behind the GUI on Windows (the Python build's PyInstaller spec used
// `console=False` for the same reason).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod backend;
mod gui;

use std::cell::RefCell;
use std::rc::Rc;

use backend::campaigns::{activate, by_id, forced_campaign_id, load_last_campaign_id, save_last_campaign_id, DEFAULT_ID};
use backend::ini_file::IniFile;
use backend::launch::{exec_game, game_args, launcher_prefs};
use backend::paths::Paths;

/// Pre-cropped, background-removed Vault Boy-on-a-nuke art, baked in as raw RGBA at build
/// time (not a PNG) so the window icon doesn't need an image-decoding dependency.
fn app_icon() -> egui::IconData {
    const RGBA: &[u8] = include_bytes!("../assets/icon/vaultboy-nuke-256.rgba");
    egui::IconData { rgba: RGBA.to_vec(), width: 256, height: 256 }
}

fn main() -> eframe::Result {
    let paths = Paths::from_current_exe();
    let argv: Vec<String> = std::env::args().collect();

    let mut prefs = launcher_prefs(&paths).unwrap_or_else(|_| {
        // `launcher_prefs` only fails if the file exists but becomes unreadable between the
        // `is_file()` check and the read (a race) - treat that the same as "no prefs yet".
        IniFile::open(paths.launcher_ini()).expect("constructing an IniFile for a path never fails")
    });

    let forced_id = forced_campaign_id(&argv);
    let campaign_id = forced_id.clone().unwrap_or_else(|| load_last_campaign_id(&prefs));
    let campaign = by_id(&campaign_id).unwrap_or_else(|| by_id(DEFAULT_ID).expect("DEFAULT_ID is registered"));
    activate(&paths, campaign);
    if forced_id.is_none() {
        let _ = save_last_campaign_id(&mut prefs, campaign.id);
    }

    let mut show = prefs.get("launcher", "show_launcher").map(|v| v != "0").unwrap_or(true);
    if argv.iter().any(|a| a == "--ui") {
        show = true;
    }
    if argv.iter().any(|a| a == "--no-ui") {
        show = false;
    }

    if !show {
        let mode = prefs.get("launcher", "on_start").unwrap_or_else(|| "continue".to_string());
        let _ = exec_game(&paths, &game_args(&paths, &mode, None));
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Fallout 2 Launcher")
            .with_icon(app_icon()),
        ..Default::default()
    };

    let launch_result = Rc::new(RefCell::new(None));
    let launch_result_for_app = launch_result.clone();
    let paths_for_app = paths.clone();
    let initial_accent = gui::theme::parse_hex(campaign.accent);

    eframe::run_native(
        "Fallout 2 Launcher",
        options,
        Box::new(move |cc| {
            gui::theme::install(&cc.egui_ctx, initial_accent);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(gui::App::new(paths_for_app, launch_result_for_app)))
        }),
    )?;

    if let Some((mode, slot)) = launch_result.borrow_mut().take() {
        let _ = exec_game(&paths, &game_args(&paths, &mode, slot));
    }
    Ok(())
}
