//! The square that stands in for a cover.
//!
//! There is no artwork in the log and there should not be: `media` carries a
//! title, a creator, a length and the name of a file, and a picture would be a
//! column every kind pays for so that one kind can have one. So this draws one
//! from the name instead — which is most of what a cover is actually doing in
//! a list, and the part that survives having no picture.
//!
//! **Deterministic, and the same on both clients.** The phone's
//! `mobile/src/ui/artwork.tsx` does this too, and the two have to agree or one
//! album is two squares depending on which screen you are looking at. That is
//! why the six gradients are generated into `palette.rs` and `palette.ts` from
//! one description in `branding/nix/palette.nix` — and why the hash below is
//! specified down to the width of what it walks, rather than being "some hash".

use iced::widget::svg;
use iced::{Color, Element, Length, Theme};

use crate::palette;

/// FNV-1a, over UTF-16 code units.
///
/// The width is the load-bearing part. The phone's is JavaScript's
/// `charCodeAt`, which yields UTF-16 code units, so a name outside the Basic
/// Multilingual Plane is two units there and would be one `char` here —
/// `encode_utf16` is what keeps "Für Elise" and anything further out hashing
/// the same on both. `wrapping_mul` for the same reason: JavaScript's
/// `Math.imul` wraps at 32 bits and Rust would otherwise panic in debug.
fn hash(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for unit in text.encode_utf16() {
        h ^= u32::from(unit);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The two stops this name draws as, in whichever theme iced picked.
pub fn of(theme: &Theme, seed: &str) -> [Color; 2] {
    let set = if theme.extended_palette().is_dark {
        palette::DARK_ART
    } else {
        palette::LIGHT_ART
    };
    set[hash(seed) as usize % set.len()]
}

/// The square itself: a rounded rectangle in the name's own colour.
///
/// **An SVG tinted by the style closure, not a `container` with a gradient
/// background.** The container was the first version and it drew nothing at
/// all — it laid out at the right size, the glyph inside it appeared, and the
/// background simply never painted. This program already knows the answer to
/// that shape of problem: `icon.rs` draws the transport and the heart as SVG
/// because an image widget positions itself from its own bounds and nothing
/// else, and the `svg` feature was already on for exactly that. Rounded
/// corners, and the artist page's circle, come free with it.
///
/// The consequence is that the *colour* cannot be in the file. `view` never
/// sees the theme — iced resolves Light or Dark internally and hands it only
/// to style closures — so a two-colour gradient chosen per theme is not
/// something a handle can carry. `svg`'s style filter replaces every colour in
/// the drawing with one, which is the same rule the heart follows and the same
/// reason.
///
/// So the depth is in *alpha* rather than in a second colour: the gradient
/// runs from the tint at full strength to the tint at a third, which survives
/// the filter because the filter replaces colour and leaves opacity alone. On
/// a dark page it fades into the page and on a light one it fades out of it,
/// which is the same shape the phone's two-stop gradient has.
pub fn square<'a, Message: 'a>(seed: &str, size: f32, corner: f32) -> Element<'a, Message> {
    // Drawn in a 100-unit box and scaled by the widget, so the same string
    // serves the 132px card and the 116px header.
    let radius = (corner / size * 100.0).clamp(0.0, 50.0);
    let seed = seed.to_string();
    svg(svg::Handle::from_memory(drawing(radius).into_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |theme: &Theme, _| svg::Style {
            // The first stop of the pair: the light end, which is the one the
            // square should read as.
            color: Some(of(theme, &seed)[0]),
        })
        .into()
}

/// The SVG source for a square of one corner radius.
///
/// Every colour in it is a placeholder that the style filter replaces, which
/// is why there is only one and why it is white: what matters is the
/// *opacity* at each stop, and white is the colour that says "I was replaced"
/// most loudly if the filter ever stops being applied.
fn drawing(radius: f32) -> String {
    format!(
        concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">"##,
            r##"<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">"##,
            r##"<stop offset="0" stop-color="#FFFFFF" stop-opacity="1"/>"##,
            r##"<stop offset="1" stop-color="#FFFFFF" stop-opacity="0.33"/>"##,
            r##"</linearGradient></defs>"##,
            r##"<rect width="100" height="100" rx="{radius}" ry="{radius}" fill="url(#g)"/>"##,
            // A note, knocked back rather than drawn in a second colour. Fira
            // Sans has no U+266A and U+00B7 is a four-pixel dot at any size,
            // which is what the first attempt at this actually put on screen.
            r##"<g fill="#FFFFFF" fill-opacity="0.35">"##,
            r##"<circle cx="44" cy="62" r="8"/><rect x="50" y="26" width="4" height="36"/>"##,
            r##"<path d="M50 26 L68 32 L68 40 L50 34 Z"/>"##,
            "</g></svg>"
        ),
        radius = radius,
    )
}

#[cfg(test)]
mod tests {
    use super::hash;

    /// The phone computes this with `Math.imul` over `charCodeAt`. If either
    /// side's hash moves, one album becomes two different squares depending on
    /// which screen it is drawn on — which is exactly the kind of drift the
    /// generated palette exists to prevent, and it would be invisible.
    #[test]
    fn matches_the_phone() {
        // Each of these is what `mobile/src/ui/artwork.tsx` answers.
        assert_eq!(hash(""), 0x811c_9dc5);
        assert_eq!(hash("a"), 0xe40c_292c);
        assert_eq!(hash("Water Music"), 0x7441_7273);
        assert_eq!(hash("Für Elise"), 0x939e_323b);
        assert_eq!(hash("Messiah"), 0x4602_085d);
    }

    /// The one case where a `char` loop and a UTF-16 loop disagree.
    ///
    /// Everything above is in the Basic Multilingual Plane, where one `char`
    /// is one code unit and the two are the same function — so none of it
    /// would catch iterating `chars()` instead. This would: U+1F3B5 is one
    /// `char` and the surrogate pair `D83C DFB5` to JavaScript, and the two
    /// walks give `0x442e75ca` and `0xb154da50`. Nothing in the library is
    /// named with an emoji today, which is the point of pinning it now.
    #[test]
    fn walks_utf16_code_units() {
        assert_eq!(hash("\u{1F3B5}"), 0x442e_75ca);
    }
}
