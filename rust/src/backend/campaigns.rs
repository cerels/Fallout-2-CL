//! Campaigns: the games this launcher can run and manage. Vanilla/CE is rooted at the
//! launcher's own install folder; every other campaign is a self-contained subfolder
//! underneath it (see `GLOSSARY.md` and `docs/adr/0001-campaign-root-is-a-subfolder.md`).
//! Installing/managing each campaign's own files is a separate module per campaign; this one
//! only knows where they live, which is installed, and which one is active right now.

use std::io;
use std::path::PathBuf;

use super::ini_file::IniFile;
use super::paths::Paths;

pub const DEFAULT_ID: &str = "ce";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Campaign {
    pub id: &'static str,
    pub name: &'static str,
    /// "" for Vanilla/CE itself; relative to the launcher directory otherwise.
    pub subdir: &'static str,
    /// Candidate engine binary filenames, checked in order.
    pub bin_names: &'static [&'static str],
    /// Short bracket-text code shown everywhere in the terminal chrome (sidebar switcher, nav) -
    /// see `GLOSSARY.md`'s "Badge" entry and `docs/adr/0008-terminal-skin-escalation.md`.
    pub badge: &'static str,
    /// Badge color (hex).
    pub accent: &'static str,
    /// This campaign's cover art (600x900 PNG bytes, embedded at build time) and the URI
    /// `egui_extras`'s image loader caches its decoded texture under - see `GLOSSARY.md`'s
    /// "Cover art" entry and `docs/adr/0010-campaign-cover-art.md`. Each cover is the campaign's
    /// own official promotional art, not something this project drew.
    pub cover: &'static [u8],
    pub cover_uri: &'static str,
    /// Whether this campaign's config uses the same file/section/key format as Vanilla/CE
    /// (see GLOSSARY.md's "Shareable campaign" and `docs/adr/0003-shared-settings-sync-on-save.md`),
    /// so a `Setting::shared` value can sync straight into it. False only for Sonora, whose
    /// FISSION fork uses an unrelated config format with no equivalent keys.
    pub shareable: bool,
}

impl Campaign {
    pub fn root(&self, paths: &Paths) -> PathBuf {
        if self.subdir.is_empty() {
            paths.launcher_dir().to_path_buf()
        } else {
            paths.launcher_dir().join(self.subdir)
        }
    }

    pub fn is_installed(&self, paths: &Paths) -> bool {
        let root = self.root(paths);
        self.bin_names.iter().any(|name| root.join(name).is_file())
    }
}

pub const CAMPAIGNS: &[Campaign] = &[
    Campaign {
        id: "ce",
        name: "Fallout 2",
        subdir: "",
        bin_names: &["fallout2-ce", "fallout2-ce.exe"],
        badge: "F2",
        accent: "#5dfc8a",
        cover: include_bytes!("../../assets/covers/ce.png"),
        cover_uri: "bytes://covers/ce.png",
        shareable: true,
    },
    Campaign {
        id: "ettu",
        name: "Fallout et Tu",
        subdir: "Fallout1in2",
        bin_names: &["fallout2-ce", "fallout2-ce.exe", "Fallout2.exe"],
        badge: "ET",
        accent: "#5d9dfc",
        cover: include_bytes!("../../assets/covers/ettu.png"),
        cover_uri: "bytes://covers/ettu.png",
        shareable: true,
    },
    Campaign {
        id: "sonora",
        name: "Fallout: Sonora",
        subdir: "Sonora",
        bin_names: &["fallout-fission", "fallout-fission.exe"],
        badge: "SO",
        accent: "#ffb84d",
        cover: include_bytes!("../../assets/covers/sonora.png"),
        cover_uri: "bytes://covers/sonora.png",
        shareable: false,
    },
    Campaign {
        id: "nevada",
        name: "Fallout 2: Nevada",
        subdir: "Nevada",
        bin_names: &["fallout2-ce", "fallout2-ce.exe"],
        badge: "NV",
        accent: "#ff7a5d",
        cover: include_bytes!("../../assets/covers/nevada.png"),
        cover_uri: "bytes://covers/nevada.png",
        shareable: true,
    },
];

pub fn by_id(campaign_id: &str) -> Option<&'static Campaign> {
    CAMPAIGNS.iter().find(|c| c.id == campaign_id)
}

pub fn installed_campaigns(paths: &Paths) -> Vec<&'static Campaign> {
    CAMPAIGNS.iter().filter(|c| c.is_installed(paths)).collect()
}

/// Installed campaigns that share Vanilla/CE's config format, in `CAMPAIGNS` order - the
/// set a `Setting::shared` value can sync across. See `Campaign::shareable`.
pub fn installed_shareable_campaigns(paths: &Paths) -> Vec<&'static Campaign> {
    installed_campaigns(paths).into_iter().filter(|c| c.shareable).collect()
}

/// Which already-installed shareable campaign a freshly-installed one should seed its
/// shared settings from: Vanilla/CE if it's installed, else the first installed shareable
/// campaign in `CAMPAIGNS` order. `None` if `new_campaign` is the only one installed so far.
pub fn shared_settings_seed_source(
    paths: &Paths,
    new_campaign: &Campaign,
) -> Option<&'static Campaign> {
    let candidates: Vec<&'static Campaign> =
        installed_shareable_campaigns(paths).into_iter().filter(|c| c.id != new_campaign.id).collect();
    candidates.iter().find(|c| c.id == DEFAULT_ID).or_else(|| candidates.first()).copied()
}

pub fn active_campaign(paths: &Paths) -> &'static Campaign {
    let root = paths.game_dir();
    CAMPAIGNS
        .iter()
        .find(|c| c.root(paths) == root)
        .unwrap_or_else(|| by_id(DEFAULT_ID).expect("DEFAULT_ID always names a real campaign"))
}

/// `--campaign=<id>` on the command line - how a Steam Launch Options line (e.g. one pasted
/// into Fallout 1's entry to force Et Tu) picks a campaign without going through "remember
/// last campaign".
pub fn forced_campaign_id(argv: &[String]) -> Option<String> {
    argv.iter().find_map(|arg| arg.strip_prefix("--campaign=").map(str::to_string))
}

pub fn load_last_campaign_id(prefs: &IniFile) -> String {
    prefs.get("launcher", "last_campaign").unwrap_or_else(|| DEFAULT_ID.to_string())
}

pub fn save_last_campaign_id(prefs: &mut IniFile, campaign_id: &str) -> io::Result<()> {
    prefs.set("launcher", "last_campaign", campaign_id);
    prefs.save()
}

pub fn activate(paths: &Paths, campaign: &Campaign) {
    paths.set_campaign_root(campaign.root(paths));
    paths.set_campaign_bin_names(campaign.bin_names);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_id_finds_known_campaigns_and_rejects_unknown_ones() {
        assert_eq!(by_id("ettu").unwrap().name, "Fallout et Tu");
        assert!(by_id("olympus").is_none());
    }

    #[test]
    fn ce_root_is_the_launcher_dir_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let ce = by_id("ce").unwrap();
        assert_eq!(ce.root(&paths), tmp.path());
    }

    #[test]
    fn total_conversion_root_is_a_subfolder() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let ettu = by_id("ettu").unwrap();
        assert_eq!(ettu.root(&paths), tmp.path().join("Fallout1in2"));
    }

    #[test]
    fn is_installed_checks_for_any_known_binary_name() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let ce = by_id("ce").unwrap();
        assert!(!ce.is_installed(&paths));

        std::fs::write(tmp.path().join("fallout2-ce.exe"), b"").unwrap();
        assert!(ce.is_installed(&paths));
    }

    #[test]
    fn installed_campaigns_lists_only_installed_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::create_dir_all(tmp.path().join("Nevada")).unwrap();
        std::fs::write(tmp.path().join("Nevada").join("fallout2-ce"), b"").unwrap();

        let installed: Vec<_> = installed_campaigns(&paths).iter().map(|c| c.id).collect();
        assert_eq!(installed, vec!["ce", "nevada"]);
    }

    #[test]
    fn active_campaign_matches_the_active_root_or_falls_back_to_default() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(active_campaign(&paths).id, "ce");

        activate(&paths, by_id("sonora").unwrap());
        assert_eq!(active_campaign(&paths).id, "sonora");
    }

    #[test]
    fn forced_campaign_id_reads_the_flag_from_argv() {
        let argv = vec!["f2launcher".to_string(), "--campaign=ettu".to_string()];
        assert_eq!(forced_campaign_id(&argv), Some("ettu".to_string()));
        assert_eq!(forced_campaign_id(&["f2launcher".to_string()]), None);
    }

    #[test]
    fn forced_campaign_id_takes_the_first_flag_when_repeated() {
        let argv = vec!["--campaign=ettu".to_string(), "--campaign=nevada".to_string()];
        assert_eq!(forced_campaign_id(&argv), Some("ettu".to_string()));
    }

    #[test]
    fn forced_campaign_id_with_empty_value_is_empty_string_not_none() {
        // Matches Python's `arg.split("=", 1)[1]`: a bare "--campaign=" still produces a
        // (empty) value, it's not treated as "no flag at all".
        let argv = vec!["--campaign=".to_string()];
        assert_eq!(forced_campaign_id(&argv), Some(String::new()));
    }

    #[test]
    fn active_campaign_falls_back_to_default_for_an_unrecognized_root() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        paths.set_campaign_root(tmp.path().join("SomeUnrelatedFolder"));
        assert_eq!(active_campaign(&paths).id, DEFAULT_ID);
    }

    #[test]
    fn only_sonora_is_not_shareable() {
        for c in CAMPAIGNS {
            assert_eq!(c.shareable, c.id != "sonora", "{}", c.id);
        }
    }

    #[test]
    fn installed_shareable_campaigns_excludes_sonora_even_when_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::create_dir_all(tmp.path().join("Sonora")).unwrap();
        std::fs::write(tmp.path().join("Sonora").join("fallout-fission"), b"").unwrap();

        let ids: Vec<_> = installed_shareable_campaigns(&paths).iter().map(|c| c.id).collect();
        assert_eq!(ids, vec!["ce"]);
    }

    #[test]
    fn seed_source_prefers_ce_when_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::create_dir_all(tmp.path().join("Nevada")).unwrap();
        std::fs::write(tmp.path().join("Nevada").join("fallout2-ce"), b"").unwrap();

        let source = shared_settings_seed_source(&paths, by_id("nevada").unwrap());
        assert_eq!(source.map(|c| c.id), Some("ce"));
    }

    #[test]
    fn seed_source_falls_back_to_first_installed_shareable_campaign_when_ce_is_not_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("Fallout1in2")).unwrap();
        std::fs::write(tmp.path().join("Fallout1in2").join("fallout2-ce"), b"").unwrap();
        std::fs::create_dir_all(tmp.path().join("Nevada")).unwrap();
        std::fs::write(tmp.path().join("Nevada").join("fallout2-ce"), b"").unwrap();

        let source = shared_settings_seed_source(&paths, by_id("nevada").unwrap());
        assert_eq!(source.map(|c| c.id), Some("ettu"));
    }

    #[test]
    fn seed_source_is_none_when_no_other_shareable_campaign_is_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();

        let source = shared_settings_seed_source(&paths, by_id("ce").unwrap());
        assert!(source.is_none());
    }

    #[test]
    fn last_campaign_id_round_trips_through_prefs() {
        let tmp = tempfile::tempdir().unwrap();
        let ini_path = tmp.path().join("launcher.ini");
        let mut prefs = IniFile::open(ini_path.clone()).unwrap();
        assert_eq!(load_last_campaign_id(&prefs), DEFAULT_ID);

        save_last_campaign_id(&mut prefs, "nevada").unwrap();
        let reloaded = IniFile::open(ini_path).unwrap();
        assert_eq!(load_last_campaign_id(&reloaded), "nevada");
    }
}
