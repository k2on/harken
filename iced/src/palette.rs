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
    background: rgb(0x1E, 0x1E, 0x1E),
    text: rgb(0xDD, 0xDD, 0xDD),
    primary: rgb(0xE9, 0xBB, 0x45),
    success: rgb(0x32, 0xD7, 0x4B),
    warning: rgb(0xE9, 0xBB, 0x45),
    danger: rgb(0xFF, 0x45, 0x3A),
};

/// …and the same, against white. The gold is darker here because
/// text has to be legible *on* it.
pub const LIGHT: Palette = Palette {
    background: rgb(0xFF, 0xFF, 0xFF),
    text: rgb(0x26, 0x26, 0x26),
    primary: rgb(0x9A, 0x74, 0x1A),
    success: rgb(0x28, 0xCD, 0x41),
    warning: rgb(0x9A, 0x74, 0x1A),
    danger: rgb(0xFF, 0x3B, 0x30),
};

/// Stand-in artwork, as the two stops of a gradient.
///
/// Nothing in the log carries a cover, and nothing should: `media`
/// has a title, a creator, a length and the name of a file, and a
/// picture would be a column every kind pays for so that one kind
/// can have one. So a record's art is *derived* from its name, in
/// [`crate::art`] — deterministically, so the same album is the same
/// square on every device, which is most of what a cover does in a
/// list.
pub const DARK_ART: [[Color; 2]; 6] = [
    [rgb(0x7A, 0x5C, 0x15), rgb(0x20, 0x19, 0x07)],
    [rgb(0x8A, 0x6B, 0x22), rgb(0x1B, 0x15, 0x09)],
    [rgb(0x6B, 0x5A, 0x2A), rgb(0x17, 0x14, 0x0B)],
    [rgb(0x8F, 0x73, 0x27), rgb(0x22, 0x1B, 0x09)],
    [rgb(0x5E, 0x4A, 0x18), rgb(0x14, 0x10, 0x07)],
    [rgb(0xA0, 0x81, 0x37), rgb(0x26, 0x1E, 0x08)],
];

/// …and against white, where the pair has to stay dark enough that
/// the note drawn on it reads.
pub const LIGHT_ART: [[Color; 2]; 6] = [
    [rgb(0xE8, 0xD1, 0x9A), rgb(0xC9, 0xA8, 0x5F)],
    [rgb(0xEF, 0xDC, 0xAE), rgb(0xD2, 0xB3, 0x70)],
    [rgb(0xE2, 0xD2, 0xAC), rgb(0xBF, 0xA6, 0x71)],
    [rgb(0xF0, 0xDF, 0xA8), rgb(0xCB, 0xAA, 0x5C)],
    [rgb(0xE6, 0xD0, 0xA0), rgb(0xC3, 0xA0, 0x5A)],
    [rgb(0xF3, 0xE6, 0xBE), rgb(0xD6, 0xB8, 0x77)],
];

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
