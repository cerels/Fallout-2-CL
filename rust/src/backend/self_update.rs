//! Checking whether a newer release of the launcher itself exists on GitHub. Notify-and-link
//! only - never downloads or replaces the running binary (`docs/adr/0012-launcher-self-update-
//! notify-only.md`).

use std::time::Duration;

const LAUNCHER_REPO: &str = "cerels/Fallout-2-CL";
pub const RELEASES_URL: &str = "https://github.com/cerels/Fallout-2-CL/releases/latest";

/// `latest_tag` (a GitHub release tag, optionally "v"-prefixed) if it differs from
/// `current_version` - this build's own `CARGO_PKG_VERSION` - else `None`. Split out from
/// `check_for_update` so it's testable with no network access needed. Unlike every other
/// `check_for_update` in this crate, there's no baseline file to seed on a first check: the
/// binary already knows its own exact version at compile time, so a straight inequality is
/// enough.
fn newer_version(latest_tag: &str, current_version: &str) -> Option<String> {
    let latest = latest_tag.strip_prefix('v').unwrap_or(latest_tag);
    (latest != current_version).then(|| latest.to_string())
}

/// The latest release's version if it's newer than what this binary was built as, else `None`.
/// Best-effort: any network/parse failure is treated the same as "nothing to report" rather
/// than surfaced as an error, since this check is never essential to using the launcher.
pub fn check_for_update() -> Option<String> {
    let api_url = format!("https://api.github.com/repos/{LAUNCHER_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(10)).ok()?;
    let tag = body.get("tag_name").and_then(|v| v.as_str())?;
    newer_version(tag, env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_reports_a_different_tag() {
        assert_eq!(newer_version("2026.10.26", "2026.10.25"), Some("2026.10.26".to_string()));
    }

    #[test]
    fn newer_version_strips_a_leading_v() {
        assert_eq!(newer_version("v2026.10.26", "2026.10.25"), Some("2026.10.26".to_string()));
    }

    #[test]
    fn newer_version_is_none_when_already_current() {
        assert_eq!(newer_version("2026.10.25", "2026.10.25"), None);
        assert_eq!(newer_version("v2026.10.25", "2026.10.25"), None);
    }
}
