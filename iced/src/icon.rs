//! The glyphs Fira Sans does not have, drawn rather than typed.
//!
//! Two things had to be worked around to put a shape on a row, and the second
//! one is only visible in a browser.
//!
//! **They cannot be characters.** iced embeds Fira Sans, which is a text face:
//! it has no U+25B6 play, U+275A pause or U+2582 block, and had no U+2665
//! heart either back when there was one. A missing glyph lays out fine and
//! draws a `?`, or nothing at all, so the button looks broken rather than
//! unfontable — which is how the transport shipped with a `?` on its play
//! button while the heart beside it was correct, because only the heart had
//! been through this once already.
//!
//! The rule, learned twice: **anything outside Latin-1 is a drawing.** `«`,
//! `»` and `·` are in the font; `▶`, `❚`, `♥` and `▂` are not.
//!
//! **It cannot be a `canvas` either, once there is one per row.** A canvas
//! inside a `scrollable` is not translated to the row it belongs to under the
//! WebGL renderer: every one in the list is drawn at very nearly the same
//! place, so twenty of them stack into what looks like one stray shape near
//! the bottom of the list, and scrolling moves the pile rather than the rows.
//! It looks like "it does not render", which is why it survived a demo — one
//! *was* rendering, and it was all twenty.
//!
//! So these are SVGs. An image is positioned by the widget that holds it
//! rather than by geometry in a shared layer, which is exactly the part that
//! was broken, and it needs no font and no icon asset.
//!
//! Their color is the theme's, applied through the `svg` style's color filter
//! rather than written into the file. A shape with a color baked in is the
//! same color on a white row, a dark row and the accent-colored row under the
//! cursor — three different backgrounds, and one color that was only ever
//! chosen against the first.

use iced::widget::{svg, Svg};
use iced::Theme;

/// How big a transport button is drawn in the now-playing bar.
pub const TRANSPORT: f32 = 15.0;

/// A tick, for a playlist this track is already on.
///
/// Drawn rather than typed for the reason at the top of this file: U+2713 is
/// outside Latin-1, so Fira Sans has nothing for it and the row would say `?`
/// where it meant yes.
const TICK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M9.2 16.6 4.8 12.2l1.6-1.6 2.8 2.8 7.2-7.2 1.6 1.6z" fill="#000"/></svg>"##;

/// Three dots, for the menu a row hides behind them. U+22EE is outside
/// Latin-1 like everything else here, so it is drawn.
const MORE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
<path d="M12 4.2a2.1 2.1 0 1 1 0 4.2 2.1 2.1 0 0 1 0-4.2zm0 5.7a2.1 2.1 0 1 1 0 4.2 2.1 2.1 0 0 1 0-4.2zm0 5.7a2.1 2.1 0 1 1 0 4.2 2.1 2.1 0 0 1 0-4.2z" fill="#000"/></svg>"##;

/// The transport glyphs, as paths in a 24-unit box.
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
            let text = crate::palette::of(theme).background.base.text;
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

/// The transport, on the row that is playing.
///
/// The only shape in the table, which is what makes it the thing the eye
/// finds: a list of a hundred near-identical names has one row with a mark
/// beside it, and that row is the one making a sound. It is a button as well
/// as a mark, because the place you look to see what is playing is the place
/// you reach to stop it.
///
/// `on_cursor` is the case the color has to answer: that row is painted in
/// the accent, so the mark takes the one color that background was paired
/// with. Off it, the mark *is* the accent, the same as the title beside it.
pub fn playing<'a>(paused: bool, on_cursor: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(if paused { PLAY } else { PAUSE }))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(if on_cursor {
                    palette.primary.base.text
                } else {
                    palette.primary.base.color
                }),
            }
        })
}

/// The tick beside a playlist this track is on.
///
/// The accent, the same as the title of the row that is playing: both mean
/// "this one", and a second color for a second kind of yes would be a color
/// nobody chose. On the cursor's own row it is the one color that background
/// was paired with.
pub fn tick<'a>(on_cursor: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(TICK))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(if on_cursor {
                    palette.primary.base.text
                } else {
                    palette.primary.base.color
                }),
            }
        })
}

/// The three dots that open a row's menu.
///
/// Drawn faintly: it is on every row, and something on every row that is as
/// loud as the title is something that competes with a hundred titles.
pub fn more<'a>(on_cursor: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(MORE))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(if on_cursor {
                    palette.primary.base.text.scale_alpha(0.8)
                } else {
                    palette.background.base.text.scale_alpha(0.45)
                }),
            }
        })
}
