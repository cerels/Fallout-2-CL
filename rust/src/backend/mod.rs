//! Ported from the Python `backend/` package - see `launcher/docs/adr/0004-rust-rewrite.md`
//! for why, and `0005-boring-rust.md` for how (simple ownership over clever ownership).
//!
//! Each module here is a 1:1 behavioral port of its Python counterpart of the same name;
//! only the implementation idioms change to fit Rust.

pub mod campaigns;
pub mod ce_install;
pub mod dat;
pub mod ettu_install;
pub mod http;
pub mod ini_file;
pub mod keys;
pub mod launch;
pub mod launch_option;
pub mod mod_catalog;
pub mod nevada_install;
pub mod rpu_install;
pub mod paths;
pub mod saves;
pub mod schema;
pub mod shared_settings;
pub mod sonora_install;
pub mod steam_shortcuts;
