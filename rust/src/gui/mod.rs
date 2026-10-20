//! The egui/eframe desktop frontend, built on top of the finished backend (see
//! `src/backend/`). A terminal-inspired skin: heavy true-dark backgrounds, monospace chrome,
//! and a per-campaign accent color that re-tints the active state - see
//! `docs/adr/0004-rust-rewrite.md` and `docs/adr/0008-terminal-skin-escalation.md`. Boring Rust
//! throughout (`docs/adr/0005`): plain structs, `Rc`/clones over lifetimes, no generics or
//! trait-object machinery beyond what egui itself asks for.

pub(crate) mod app;
pub(crate) mod errors;
pub(crate) mod jobs;
mod keys_egui;
pub(crate) mod open;
mod pages;
pub(crate) mod store;
pub(crate) mod theme;
pub(crate) mod widgets;

pub use app::App;
