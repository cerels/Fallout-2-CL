//! DIK (DirectInput scan code) keyboard tables: party orders / sfall use these, not Linux
//! evdev codes or egui key codes, so captured key presses need translating.

use std::collections::HashMap;
use std::sync::OnceLock;

/// DIK code -> display name, built once and reused. A `OnceLock` is the "boring" way to get
/// a lazily-built, read-only global table in Rust without `unsafe` or an external crate.
fn dik_names() -> &'static HashMap<i32, String> {
    static TABLE: OnceLock<HashMap<i32, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut names: HashMap<i32, String> = HashMap::new();
        for (code, name) in [
            (1, "Esc"), (12, "-"), (13, "="), (14, "Backspace"), (15, "Tab"),
            (26, "["), (27, "]"), (28, "Enter"), (29, "Left Ctrl"), (39, ";"),
            (40, "'"), (41, "`"), (42, "Left Shift"), (43, "\\"), (51, ","),
            (52, "."), (53, "/"), (54, "Right Shift"), (55, "Num *"), (56, "Left Alt"),
            (57, "Space"), (58, "Caps Lock"), (69, "Num Lock"), (70, "Scroll Lock"),
            (71, "Num 7"), (72, "Num 8"), (73, "Num 9"), (74, "Num -"), (75, "Num 4"),
            (76, "Num 5"), (77, "Num 6"), (78, "Num +"), (79, "Num 1"), (80, "Num 2"),
            (81, "Num 3"), (82, "Num 0"), (83, "Num ."), (87, "F11"), (88, "F12"),
            (156, "Num Enter"), (157, "Right Ctrl"), (181, "Num /"), (184, "Right Alt"),
            (199, "Home"), (200, "Up"), (201, "Page Up"), (203, "Left"), (205, "Right"),
            (207, "End"), (208, "Down"), (209, "Page Down"), (210, "Insert"), (211, "Delete"),
        ] {
            names.insert(code, name.to_string());
        }

        for (i, c) in "1234567890".chars().enumerate() {
            names.insert(2 + i as i32, c.to_string());
        }
        for (row, start) in [("QWERTYUIOP", 16), ("ASDFGHJKL", 30), ("ZXCVBNM", 44)] {
            for (i, c) in row.chars().enumerate() {
                names.insert(start + i as i32, c.to_string());
            }
        }
        for i in 0..10 {
            names.insert(59 + i, format!("F{}", i + 1));
        }
        names
    })
}

/// Linux evdev codes that differ from DIK codes (extended keys).
pub fn evdev_to_dik(evdev_code: i32) -> Option<i32> {
    match evdev_code {
        96 => Some(156),
        97 => Some(157),
        98 => Some(181),
        100 => Some(184),
        102 => Some(199),
        103 => Some(200),
        104 => Some(201),
        105 => Some(203),
        106 => Some(205),
        107 => Some(207),
        108 => Some(208),
        109 => Some(209),
        110 => Some(210),
        111 => Some(211),
        _ => None,
    }
}

/// Render a stored key value (e.g. `"42+5"` for Left Shift + B) the way the settings UI
/// shows it to a player, e.g. `"Left Ctrl + B"`. Modifier is shown first, matching Python.
pub fn dik_label(value: &str) -> String {
    let mut parts = Vec::new();
    for raw in value.split('+') {
        let raw = raw.trim();
        let Ok(code) = raw.parse::<i32>() else {
            return value.to_string();
        };
        parts.push(if code == 0 {
            "None".to_string()
        } else {
            dik_names().get(&code).cloned().unwrap_or_else(|| format!("Key {code}"))
        });
    }
    if parts.len() == 2 {
        parts.reverse();
    }
    parts.join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_letter_key() {
        assert_eq!(dik_label("30"), "A");
    }

    #[test]
    fn digit_row_key() {
        assert_eq!(dik_label("2"), "1");
    }

    #[test]
    fn function_key() {
        assert_eq!(dik_label("68"), "F10");
        assert_eq!(dik_label("87"), "F11");
    }

    #[test]
    fn modifier_combo_shows_modifier_first() {
        // Python: dik_label("30+42") == "Left Shift + A" (modifier first, value second).
        assert_eq!(dik_label("30+42"), "Left Shift + A");
    }

    #[test]
    fn zero_means_none() {
        assert_eq!(dik_label("0"), "None");
    }

    #[test]
    fn unknown_code_falls_back_to_key_n() {
        assert_eq!(dik_label("999"), "Key 999");
    }

    #[test]
    fn non_numeric_value_passes_through_unchanged() {
        assert_eq!(dik_label("Already Labeled"), "Already Labeled");
    }

    #[test]
    fn evdev_extended_keys_map_to_dik() {
        assert_eq!(evdev_to_dik(103), Some(200)); // Up
        assert_eq!(evdev_to_dik(1), None);
    }
}
