use std::sync::OnceLock;

use ratatui::style::{Color, Style};

const CRUST: Color = Color::Rgb(0x11, 0x11, 0x1b);
const MANTLE: Color = Color::Rgb(0x18, 0x18, 0x25);
const SURFACE0: Color = Color::Rgb(0x31, 0x32, 0x44);
const OVERLAY: Color = Color::Rgb(0x6c, 0x70, 0x86);
const SUBTEXT: Color = Color::Rgb(0xa6, 0xad, 0xc8);
const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
const MAUVE: Color = Color::Rgb(0xcb, 0xa6, 0xf7);
const BLUE: Color = Color::Rgb(0x89, 0xb4, 0xfa);
const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);

/// Catppuccin Mocha palette, or a monochrome fallback for `NO_COLOR`.
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub surface: Color,
    pub overlay: Color,
    pub text: Color,
    pub subtext: Color,
    pub mauve: Color,
    pub blue: Color,
    pub teal: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub red: Color,
}

impl Theme {
    const fn mocha() -> Self {
        Self {
            bg: CRUST,
            panel: MANTLE,
            surface: SURFACE0,
            overlay: OVERLAY,
            text: TEXT,
            subtext: SUBTEXT,
            mauve: MAUVE,
            blue: BLUE,
            teal: TEAL,
            green: GREEN,
            yellow: YELLOW,
            peach: PEACH,
            red: RED,
        }
    }

    const fn mono() -> Self {
        Self {
            bg: Color::Reset,
            panel: Color::Reset,
            surface: Color::Reset,
            overlay: Color::Reset,
            text: Color::Reset,
            subtext: Color::Reset,
            mauve: Color::Reset,
            blue: Color::Reset,
            teal: Color::Reset,
            green: Color::Reset,
            yellow: Color::Reset,
            peach: Color::Reset,
            red: Color::Reset,
        }
    }

    #[must_use]
    pub fn border(self) -> Style {
        Style::default().fg(self.surface)
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// Returns the active palette, honoring `NO_COLOR` and `TERM=dumb`.
#[must_use]
pub fn theme() -> Theme {
    *THEME.get_or_init(|| {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        if no_color || dumb {
            Theme::mono()
        } else {
            Theme::mocha()
        }
    })
}
