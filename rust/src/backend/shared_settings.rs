//! Install-time half of shared settings (see `docs/adr/0003-shared-settings-sync-on-save.md`
//! and `gui::store`'s save-time half): seeding a freshly installed campaign's own config from
//! whichever shareable campaign is already installed, so it doesn't start out mismatched
//! until the player happens to touch a shared setting from some other campaign's page.

use super::campaigns::{self, Campaign};
use super::ini_file::IniFile;
use super::paths::Paths;
use super::schema::{self, Setting};

fn read_value(f: &IniFile, s: &Setting) -> Option<String> {
    if s.kind == "resolution" {
        let (kx, ky) = s.key.split_once('|')?;
        let (x, y) = (f.get(&s.section, kx), f.get(&s.section, ky));
        return match (x, y) {
            (Some(x), Some(y)) => Some(format!("{x}x{y}")),
            _ => None,
        };
    }
    s.normalize(f.get(&s.section, &s.key).as_deref())
}

fn write_value(f: &mut IniFile, s: &Setting, value: &str) {
    if s.kind == "resolution" {
        if let Some((kx, ky)) = s.key.split_once('|') {
            if let Some((x, y)) = value.to_lowercase().split_once('x') {
                f.set(&s.section, kx, x);
                f.set(&s.section, ky, y);
            }
        }
    } else {
        f.set(&s.section, &s.key, value);
    }
}

/// Seeds `new_campaign`'s own config from whichever already-installed shareable campaign
/// `campaigns::shared_settings_seed_source` picks - a no-op for a non-shareable campaign
/// (Sonora) or when no other shareable campaign is installed yet. Best-effort per file, like
/// save-time sync: a file that can't be read or written is logged and skipped rather than
/// failing the install that already succeeded.
pub fn seed_new_campaign(paths: &Paths, new_campaign: &Campaign) {
    if !new_campaign.shareable {
        return;
    }
    let Some(source) = campaigns::shared_settings_seed_source(paths, new_campaign) else { return };

    let catalog = schema::load_catalog(paths);
    let source_root = source.root(paths);
    let dest_root = new_campaign.root(paths);

    let mut by_file: std::collections::HashMap<&str, Vec<&Setting>> = std::collections::HashMap::new();
    for page in &catalog.pages {
        for (_, settings) in &page.groups {
            for s in settings {
                if s.shared {
                    by_file.entry(s.file.as_str()).or_default().push(s);
                }
            }
        }
    }

    for (file_id, settings) in by_file {
        let Some(rel) = catalog.files.get(file_id) else { continue };
        let source_path = paths.ci_path(rel, Some(&source_root));
        let dest_path = paths.ci_path(rel, Some(&dest_root));
        if !source_path.is_file() || !dest_path.is_file() {
            continue;
        }
        let source_file = match IniFile::open(source_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("shared settings: couldn't read {rel} from {}: {e}", source.name);
                continue;
            }
        };
        let mut dest_file = match IniFile::open(dest_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("shared settings: couldn't open {rel} for {}: {e}", new_campaign.name);
                continue;
            }
        };
        for s in settings {
            if let Some(value) = read_value(&source_file, s) {
                write_value(&mut dest_file, s, &value);
            }
        }
        if let Err(e) = dest_file.save() {
            eprintln!("shared settings: couldn't seed {rel} for {}: {e}", new_campaign.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_a_freshly_installed_campaigns_shared_settings_from_an_existing_one() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::write(tmp.path().join("fallout2.cfg"), "[sound]\nmaster_volume=500\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("Nevada")).unwrap();
        std::fs::write(tmp.path().join("Nevada").join("fallout2-ce"), b"").unwrap();
        std::fs::write(tmp.path().join("Nevada").join("fallout2.cfg"), "[sound]\nmaster_volume=100\n").unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());

        seed_new_campaign(&paths, campaigns::by_id("nevada").unwrap());

        let nevada_cfg = std::fs::read_to_string(tmp.path().join("Nevada").join("fallout2.cfg")).unwrap();
        assert!(nevada_cfg.contains("master_volume=500"), "{nevada_cfg}");
    }

    #[test]
    fn does_not_seed_a_non_shareable_campaign() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::write(tmp.path().join("fallout2.cfg"), "[sound]\nmaster_volume=500\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("Sonora")).unwrap();
        std::fs::write(tmp.path().join("Sonora").join("fallout-fission"), b"").unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());

        // Should be a complete no-op: Sonora has no fallout2.cfg at all, and nothing should
        // be fabricated for it.
        seed_new_campaign(&paths, campaigns::by_id("sonora").unwrap());

        assert!(!tmp.path().join("Sonora").join("fallout2.cfg").exists());
    }

    #[test]
    fn is_a_no_op_when_no_other_shareable_campaign_is_installed_yet() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout2-ce"), b"").unwrap();
        std::fs::write(tmp.path().join("fallout2.cfg"), "[sound]\nmaster_volume=500\n").unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());

        // Must not panic or alter CE's own config when it's the only shareable campaign
        // installed (it is itself the "new" one here).
        seed_new_campaign(&paths, campaigns::by_id("ce").unwrap());

        let ce_cfg = std::fs::read_to_string(tmp.path().join("fallout2.cfg")).unwrap();
        assert_eq!(ce_cfg, "[sound]\nmaster_volume=500\n");
    }
}
