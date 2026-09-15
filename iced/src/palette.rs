//! The palette, generated from `branding/nix/palette.nix`.
//!
//! Do not edit: `nix run .#write-files` writes this and `nix flake
//! check` fails while it differs. The colors live next door to the
//! icon that uses the same gold, and reach the phone as
//! `mobile/src/palette.ts` from the same description.
//!
//! iced picks Light or Dark from the system and hands every style
//! closure the theme it picked; [`of`] reads which of the two that
//! was and answers with ours. So the rule that makes dark mode work
//! is unchanged — nothing writes a color down, it asks — and what
//! it asks is this file rather than iced's own.
use iced::theme::palette::Extended;
use iced::theme::Palette;
use iced::{Color, Theme};
use std::sync::OnceLock;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

/// Black, white text, gold.
pub const DARK: Palette = Palette {
    background: rgb(0x0A, 0x0A, 0x0A),
    text: rgb(0xFF, 0xFF, 0xFF),
    primary: rgb(0xE9, 0xBB, 0x45),
    success: rgb(0x5F, 0xBE, 0x88),
    warning: rgb(0xE9, 0xBB, 0x45),
    danger: rgb(0xE4, 0x68, 0x5C),
};

/// …and the same, against white. The gold is darker here because
/// text has to be legible *on* it.
pub const LIGHT: Palette = Palette {
    background: rgb(0xFF, 0xFF, 0xFF),
    text: rgb(0x0A, 0x0A, 0x09),
    primary: rgb(0x9A, 0x74, 0x1A),
    success: rgb(0x2E, 0x7C, 0x51),
    warning: rgb(0x9A, 0x74, 0x1A),
    danger: rgb(0xB2, 0x35, 0x2A),
};

/// Ours, for whichever of the two iced picked.
///
/// Generated once each rather than per widget per frame: `Extended`
/// is forty-odd colors derived from six, and deriving them at
/// sixty hertz would be forty-odd divisions nobody asked for.
pub fn of(theme: &Theme) -> &'static Extended {
    static DARK_EXT: OnceLock<Extended> = OnceLock::new();
    static LIGHT_EXT: OnceLock<Extended> = OnceLock::new();

    if theme.extended_palette().is_dark {
        DARK_EXT.get_or_init(|| Extended::generate(DARK))
    } else {
        LIGHT_EXT.get_or_init(|| Extended::generate(LIGHT))
    }
}
