//! A heart, drawn rather than typed — and as an image rather than a canvas.
//!
//! Two things had to be worked around to put a heart on a row, and the second
//! one is only visible in a browser.
//!
//! **It cannot be a character.** iced embeds Fira Sans, whose cmap has no
//! U+2665, no U+2661 and no U+2764, so the glyph silently draws nothing at
//! all: widgets lay out, input works, and the button is blank. That is the
//! same failure mode `CLAUDE.md` records for a browser build with no font at
//! all, and it is hard to recognise as a font problem when you meet it.
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
//! Its colour is the theme's, applied through the `svg` style's colour filter
//! rather than written into the file. A heart with `fill="#d9364f"` baked in
//! is a heart that is the same red on a white row, a dark row and the accent
//! -coloured row under the cursor — which is three different backgrounds and
//! one colour that was only ever chosen against the first.

use iced::widget::{svg, Svg};
use iced::Theme;

/// Filled when the song is on the playlist, outlined when it is not. The same
/// path either way: two cubics down each side, meeting at the point.
///
/// Neither carries a colour worth keeping — the `fill` and `stroke` below are
/// placeholders that iced's colour filter replaces — because a hardcoded red
/// is a red that cannot follow the theme. See [`heart`].
const FILLED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="#000"/>
</svg>"##;

const OUTLINE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="none" stroke="#000" stroke-width="1.6"/>
</svg>"##;

/// How big a heart is drawn in a table row.
pub const SIZE: f32 = 15.0;

/// The heart, for a row that is on the playlist or is not.
///
/// The colour comes from the theme rather than from the file, through the
/// `svg` style's colour filter — so the same two shapes serve light and dark,
/// and a hearted row on the cursor's own background is drawn in the colour
/// that background was built to be read against. `on_cursor` is that case: the
/// row is painted in the accent colour, so a red heart on it would be two
/// saturated colours fighting.
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
                // colour that row is guaranteed to be legible in.
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
