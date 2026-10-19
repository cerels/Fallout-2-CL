//! The egui/eframe desktop frontend, built on top of the finished backend (see
//! `src/backend/`). Pip-Boy-inspired skin: dark, modern layout with amber/green CRT accents
//! kept subtle - see `docs/adr/0004-rust-rewrite.md`. Boring Rust throughout (`docs/adr/0005`):
//! plain structs, `Rc`/clones over lifetimes, no generics or trait-object machinery beyond
//! what egui itself asks for.

pub(crate) mod app;
pub(crate) mod jobs;
mod keys_egui;
pub(crate) mod open;
mod pages;
pub(crate) mod store;
pub(crate) mod theme;
pub(crate) mod widgets;

pub use app::App;
