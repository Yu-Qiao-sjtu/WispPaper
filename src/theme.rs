//! Design tokens. Colours are semantic (`Theme::text()`, `Theme::line()`);
//! the accent follows the open library, so each library looks like itself.

use gpui::{BoxShadow, Hsla, hsla, point, px, rgb};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const UI_FONT: &str = if cfg!(target_os = "linux") { "Noto Sans" } else { ".SystemUIFont" };

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// A library colour: the strong tone for marks and buttons, the soft one for fills.
#[derive(Clone, Copy)]
pub struct Accent {
    pub key: &'static str,
    pub strong: u32,
    pub soft: u32,
}

/// Same keys and order as `wisp_core::libraries::COLORS`.
pub const ACCENTS: [Accent; 7] = [
    Accent { key: "blue", strong: 0x2F6BDE, soft: 0xE8F0FE },
    Accent { key: "green", strong: 0x2E8B57, soft: 0xE4F4EA },
    Accent { key: "amber", strong: 0xB7791F, soft: 0xFBF1DC },
    Accent { key: "rose", strong: 0xC8456B, soft: 0xFCE8EE },
    Accent { key: "violet", strong: 0x7052C8, soft: 0xEEE9FB },
    Accent { key: "teal", strong: 0x1F8A8A, soft: 0xDFF3F2 },
    Accent { key: "slate", strong: 0x4A5568, soft: 0xE9ECF0 },
];

static ACCENT: AtomicUsize = AtomicUsize::new(0);

pub fn accent_of(key: &str) -> Accent {
    ACCENTS.iter().copied().find(|a| a.key == key).unwrap_or(ACCENTS[0])
}

/// Take the colour of the library being opened (or the default on the home page).
pub fn set_accent(key: &str) {
    ACCENT.store(ACCENTS.iter().position(|a| a.key == key).unwrap_or(0), Ordering::Relaxed);
}

pub struct Theme;

impl Theme {
    /// The window surface the sidebar sits on.
    pub fn bg_window() -> Hsla { c(0xF5F4F1) }
    /// The white working plane: tables, panels, dialogs.
    pub fn bg_body() -> Hsla { c(0xFFFFFF) }
    /// Insets on the working plane: inputs at rest, table header, code.
    pub fn bg_sunken() -> Hsla { c(0xF7F7F5) }
    pub fn hover() -> Hsla { hsla(0., 0., 0., 0.045) }
    pub fn pressed() -> Hsla { hsla(0., 0., 0., 0.08) }
    pub fn text() -> Hsla { c(0x1F2329) }
    pub fn text_secondary() -> Hsla { c(0x5C636E) }
    pub fn text_tertiary() -> Hsla { c(0x8F959E) }
    pub fn text_placeholder() -> Hsla { c(0xB0B5BD) }
    pub fn line() -> Hsla { c(0xE7E7E3) }
    pub fn line_strong() -> Hsla { c(0xD5D6D2) }
    pub fn scrim() -> Hsla { hsla(0., 0., 0., 0.28) }
    pub fn danger() -> Hsla { c(0xD4483B) }
    pub fn danger_soft() -> Hsla { c(0xFDECEA) }
    pub fn success() -> Hsla { c(0x2E8B57) }
    pub fn star() -> Hsla { c(0xE0A100) }
    pub fn accent() -> Hsla { c(ACCENTS[ACCENT.load(Ordering::Relaxed)].strong) }
    pub fn accent_soft() -> Hsla { c(ACCENTS[ACCENT.load(Ordering::Relaxed)].soft) }
    pub fn selection() -> Hsla { Self::accent().opacity(0.22) }
}

/// The (text, fill) of a status chip.
pub fn status_colors(value: &str) -> (Hsla, Hsla) {
    match value {
        "included" | "read" | "stored" | "published" => (c(0x23764A), c(0xE4F4EA)),
        "excluded" | "skimmed" | "linked" => (c(0x2A5FC4), c(0xE8F0FE)),
        "uncertain" | "preprint" => (c(0x9A6410), c(0xFBF1DC)),
        "irrelevant" => (c(0xB23B30), c(0xFDECEA)),
        _ => (c(0x5C636E), c(0xEFEFEC)),
    }
}

pub fn shadow_panel() -> Vec<BoxShadow> {
    vec![
        BoxShadow { color: hsla(0., 0., 0., 0.10), offset: point(px(0.), px(8.)), blur_radius: px(28.), spread_radius: px(0.) },
        BoxShadow { color: hsla(0., 0., 0., 0.06), offset: point(px(0.), px(1.)), blur_radius: px(3.), spread_radius: px(0.) },
    ]
}

pub fn shadow_card() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(0., 0., 0., 0.05), offset: point(px(0.), px(1.)), blur_radius: px(3.), spread_radius: px(0.) }]
}
