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

use iced::widget::{center, container, text};
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

/// The square itself: the gradient, with a note drawn faintly on it.
///
/// Faint because it is a placeholder and not a logo — it should read as "a
/// record" at a glance and stop asking for attention on the second look.
pub fn square<'a, Message: 'a>(seed: &str, size: f32, corner: f32) -> Element<'a, Message> {
    let seed = seed.to_string();
    // The glyph is sized from the square rather than fixed, so one function
    // serves the 34px thumbnail in a grid and the 180px one on an album page.
    let note = text(icon_char()).size(size * 0.34).style({
        let seed = seed.clone();
        move |theme: &Theme| text::Style {
            // Against the gradient rather than against the page: the light
            // theme's pairs are dark, so the note is light on both.
            color: Some(on(of(theme, &seed))),
        }
    });

    container(center(note))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |theme: &Theme| {
            let [from, to] = of(theme, &seed);
            container::Style {
                // Down-right, the way both clients' gradients run.
                background: Some(
                    iced::gradient::Linear::new(std::f32::consts::FRAC_PI_4 * 3.0)
                        .add_stop(0.0, from)
                        .add_stop(1.0, to)
                        .into(),
                ),
                border: iced::Border {
                    radius: corner.into(),
                    ..Default::default()
                },
                ..container::Style::default()
            }
        })
        .into()
}

/// Which of white or black is legible on a pair.
///
/// Read off the *first* stop, which is the light end of every pair in the
/// branding — so the answer is the same for the whole square rather than
/// changing halfway down it.
fn on([from, _]: [Color; 2]) -> Color {
    // Rec. 601 luma, which is what "is this dark" means to an eye.
    let luma = 0.299 * from.r + 0.587 * from.g + 0.114 * from.b;
    if luma > 0.55 {
        Color::from_rgba(0.0, 0.0, 0.0, 0.5)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.45)
    }
}

/// The note, which the embedded font does not have.
///
/// Fira Sans is a text face: `♪` U+266A is outside Latin-1 and draws as a `?`
/// or as nothing, which is the trap this repository has already paid for
/// twice. So the placeholder is a character the font *does* have.
fn icon_char() -> &'static str {
    "·"
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
