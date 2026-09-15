//! The glyphs Fira Sans does not have, drawn rather than typed.
//!
//! Two things had to be worked around to put a heart on a row, and the second
//! one is only visible in a browser.
//!
//! **They cannot be characters.** iced embeds Fira Sans, which is a text face:
//! it has no U+2665 heart, and no U+25B6 play, U+275A pause or U+2582 block
//! either. A missing glyph lays out fine and draws a `?`, or nothing at all,
//! so the button looks broken rather than unfontable — which is how the
//! transport shipped with a `?` on its play button while the heart beside it
//! was correct, because only the heart had been through this once already.
//!
//! The rule, learned twice: **anything outside Latin-1 is a drawing.** `«`,
//! `»` and `·` are in the font; `▶`, `❚`, `♥` and `▂` are not.
//!
//! **It cannot be a `canvas` either, once there is one per row.** A canvas
//! inside a `scrollable` is not translated to the row it belongs to under the
//! WebGL renderer: every heart in the list is drawn at very nearly the same
//! place, so twenty of them stack into what looks like one stray heart near
//! the bottom of the list, and scrolling moves the pile rather than the
//! hearts. It looks like "the heart does not render", which is why it survived
//! a demo — one heart *was* rendering, and it was all twenty.
//!
//! So it is an SVG. An image is positioned by the widget that holds it rather
//! than by geometry in a shared layer, which is exactly the part that was
//! broken, and it needs no font and no icon asset.
//!
//! Its color is the theme's, applied through the `svg` style's color filter
//! rather than written into the file. A heart with `fill="#d9364f"` baked in
//! is a heart that is the same red on a white row, a dark row and the accent
//! -colored row under the cursor — which is three different backgrounds and
//! one color that was only ever chosen against the first.

use iced::widget::{svg, Svg};
use iced::Theme;

/// Filled when the song is on the playlist, outlined when it is not. The same
/// path either way: two cubics down each side, meeting at the point.
///
/// Neither carries a color worth keeping — the `fill` and `stroke` below are
/// placeholders that iced's color filter replaces — because a hardcoded red
/// is a red that cannot follow the theme. See [`heart`].
const FILLED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="#000"/>
</svg>"##;

const OUTLINE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="none" stroke="#000" stroke-width="1.6"/>
</svg>"##;

/// How big a heart is drawn in a table row.
pub const SIZE: f32 = 15.0;

/// How big a transport button is drawn in the now-playing bar.
pub const TRANSPORT: f32 = 15.0;

/// The transport glyphs, as paths in the same 24-unit box as the heart.
const PLAY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M8 5l11 7-11 7z" fill="#000"/></svg>"##;

const PAUSE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="#000"/></svg>"##;

const PREVIOUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M6 5h2.5v14H6zM19 5l-9 7 9 7z" fill="#000"/></svg>"##;

const NEXT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M15.5 5H18v14h-2.5zM5 5l9 7-9 7z" fill="#000"/></svg>"##;

/// One of the transport buttons, in the theme's own text color.
///
/// `dim` is the pair either side of play/pause: they do the same kind of thing
/// and should not compete with it for the eye.
fn transport<'a>(shape: &'static [u8], dim: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(shape))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let text = theme.extended_palette().background.base.text;
            svg::Style {
                color: Some(if dim { text.scale_alpha(0.6) } else { text }),
            }
        })
}

pub fn play<'a>() -> Svg<'a> {
    transport(PLAY, false)
}

pub fn pause<'a>() -> Svg<'a> {
    transport(PAUSE, false)
}

pub fn previous<'a>() -> Svg<'a> {
    transport(PREVIOUS, true)
}

pub fn next<'a>() -> Svg<'a> {
    transport(NEXT, true)
}

/// The heart, for a row that is on the playlist or is not.
///
/// The color comes from the theme rather than from the file, through the
/// `svg` style's color filter — so the same two shapes serve light and dark,
/// and a hearted row on the cursor's own background is drawn in the color
/// that background was built to be read against. `on_cursor` is that case: the
/// row is painted in the accent color, so a red heart on it would be two
/// saturated colors fighting.
///
/// `from_memory` keys its cache on the bytes, and there are exactly two sets
/// of them, so rebuilding this every frame parses nothing.
pub fn heart<'a>(filled: bool, on_cursor: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(if filled {
        FILLED
    } else {
        OUTLINE
    }))
    .width(SIZE)
    .height(SIZE)
    .style(move |theme: &Theme, _| {
        let palette = theme.extended_palette();
        svg::Style {
            color: Some(match (filled, on_cursor) {
                // On the cursor's row everything is drawn in the one
                // color that row is guaranteed to be legible in.
                (true, true) => palette.primary.base.text,
                (false, true) => palette.primary.base.text.scale_alpha(0.55),
                // Off it, a hearted row earns the accent; an empty one is
                // an outline that should not compete with the title.
                (true, false) => palette.danger.base.color,
                (false, false) => palette.background.base.text.scale_alpha(0.35),
            }),
        }
    })
}
