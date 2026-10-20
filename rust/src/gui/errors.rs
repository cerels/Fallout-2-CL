//! Maps known raw error text from backends/external scripts (undat.sh, HTTP client errors,
//! etc. - see `docs/adr/0009-error-ux-revision.md`) to player-facing copy before it reaches an
//! `error_dialog`. Anything not recognized passes through unchanged rather than being hidden -
//! a confusing message a player can paste into a bug report beats a friendly one that loses the
//! actual cause.

/// Rewrite `raw` if it matches a known pattern, otherwise return it unchanged.
pub(crate) fn friendly(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("master.dat") || lower.contains("pass path") {
        return "That doesn't look like Fallout 1's MASTER.DAT - check the path and try again.".to_string();
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return "That took too long and timed out - check your connection and try again.".to_string();
    }
    if lower.contains("dns") || lower.contains("could not resolve") || lower.contains("connection refused") {
        return "Couldn't reach the internet - check your connection and try again.".to_string();
    }
    if lower.contains("404") {
        return "That release couldn't be found online - it may have moved or been renamed. \
                Check the source link for the current location."
            .to_string();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_patterns() {
        assert!(friendly("Please pass path to Fallout 1's master.dat to the script.").contains("MASTER.DAT"));
        assert!(friendly("request timed out after 20s").contains("timed out"));
        assert!(friendly("HTTP error: 404 Not Found").contains("couldn't be found"));
    }

    #[test]
    fn leaves_unrecognized_text_alone() {
        assert_eq!(friendly("disk full"), "disk full");
    }
}
