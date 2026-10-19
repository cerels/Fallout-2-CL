//! Temporary CLI stub: the backend port is driven by its own test suite (`cargo test`), not
//! by this binary. This will become the egui app once the backend port is done - see
//! `launcher/docs/adr/0004-rust-rewrite.md` for the plan (backend first, GUI last).

mod backend;

fn main() {
    let paths = backend::paths::Paths::from_current_exe();
    println!("f2launcher (Rust rewrite, work in progress)");
    println!("launcher_dir = {}", paths.launcher_dir().display());
}
