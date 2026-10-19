//! One module per nav page. Each owns whatever per-page state egui can't derive fresh every
//! frame from the backend alone (selected save slot, pending background jobs, ...);
//! everything else is just read straight out of `backend`/`gui::store` each frame, immediate-
//! mode style.

pub mod campaigns;
pub mod launcher_info;
pub mod mods;
pub mod play;
pub mod rpu;
pub mod settings;
