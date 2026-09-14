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
//! broken, and it needs no font, no icon asset and no second colour to explain
//! it.

use iced::widget::{svg, Svg};

/// Filled when the song is on the playlist, outlined when it is not. The same
/// path either way: two cubics down each side, meeting at the point.
const FILLED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="#d9364f"/>
</svg>"##;

const OUTLINE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 21.4C2.4 14.3 1 8.3 4.3 5.1 7.3 2.2 10.4 3.7 12 6.9c1.6-3.2 4.7-4.7 7.7-1.8 3.3 3.2 1.9 9.2-7.7 16.3z" fill="none" stroke="#8c8c93" stroke-width="1.6"/>
</svg>"##;

/// How big a heart is drawn in a list row.
pub const SIZE: f32 = 18.0;

/// The heart, for a row that is on the playlist or is not.
///
/// `from_memory` keys its cache on the bytes, and there are exactly two
/// possible sets of them, so rebuilding this every frame parses nothing.
pub fn heart<'a>(filled: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(if filled {
        FILLED
    } else {
        OUTLINE
    }))
    .width(SIZE)
    .height(SIZE)
}
