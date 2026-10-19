//! egui key -> DIK (DirectInput scan code) translation for the key-binding widget. This is
//! purely a GUI concern (how *this toolkit* reports keypresses), the same way the old Qt
//! build had its own `nativeScanCode()`-based table in `gui/widgets.py` - `backend::keys`
//! only carries the DIK tables themselves (`dik_label`) plus a Linux-evdev table that has
//! nothing to do with egui's own key enum, so it doesn't belong there.

use eframe::egui;

/// DIK code for a plain key, or `None` for keys this binder doesn't support capturing
/// (numpad-specific keys, media keys, ...) - matches the DIK table in `backend::keys`.
pub fn egui_key_to_dik(key: egui::Key) -> Option<i32> {
    use egui::Key::*;
    Some(match key {
        Escape => 1,
        Num1 => 2,
        Num2 => 3,
        Num3 => 4,
        Num4 => 5,
        Num5 => 6,
        Num6 => 7,
        Num7 => 8,
        Num8 => 9,
        Num9 => 10,
        Num0 => 11,
        Minus => 12,
        Equals => 13,
        Backspace => 14,
        Tab => 15,
        Q => 16,
        W => 17,
        E => 18,
        R => 19,
        T => 20,
        Y => 21,
        U => 22,
        I => 23,
        O => 24,
        P => 25,
        OpenBracket => 26,
        CloseBracket => 27,
        Enter => 28,
        A => 30,
        S => 31,
        D => 32,
        F => 33,
        G => 34,
        H => 35,
        J => 36,
        K => 37,
        L => 38,
        Semicolon => 39,
        Quote => 40,
        Backtick => 41,
        Backslash => 43,
        Z => 44,
        X => 45,
        C => 46,
        V => 47,
        B => 48,
        N => 49,
        M => 50,
        Comma => 51,
        Period => 52,
        Slash => 53,
        Space => 57,
        F1 => 59,
        F2 => 60,
        F3 => 61,
        F4 => 62,
        F5 => 63,
        F6 => 64,
        F7 => 65,
        F8 => 66,
        F9 => 67,
        F10 => 68,
        F11 => 87,
        F12 => 88,
        Home => 199,
        ArrowUp => 200,
        PageUp => 201,
        ArrowLeft => 203,
        ArrowRight => 205,
        End => 207,
        ArrowDown => 208,
        PageDown => 209,
        Insert => 210,
        Delete => 211,
        _ => return None,
    })
}

/// DIK modifier code for whichever modifier is held, matching `KeyCapture`'s priority in the
/// old Qt build (Ctrl, then Alt, then Shift) - `None` when no modifier is held.
pub fn modifier_dik(modifiers: egui::Modifiers) -> Option<i32> {
    if modifiers.ctrl {
        Some(29)
    } else if modifiers.alt {
        Some(56)
    } else if modifiers.shift {
        Some(42)
    } else {
        None
    }
}
