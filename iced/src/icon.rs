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
//! **The drawings are not in this file any more.** They are Lucide's, vendored
//! once in `branding/icons/` and generated into [`crate::glyphs`] and the
//! phone's `mobile/src/ui/glyphs.ts` from the same files — because the two
//! clients were drawing two icon sets and calling them one program. What is
//! left here is the half that does not generalise: which colour each glyph
//! earns, how big it is drawn, and what it means.
//!
//! Their color is the theme's, applied through the `svg` style's color filter
//! rather than written into the file. A shape with a color baked in is the
//! same color on a white row, a dark row and the accent-colored row under the
//! cursor — three different backgrounds, and one color that was only ever
//! chosen against the first.

use crate::glyphs;
use iced::widget::{svg, Svg};
use iced::Theme;

/// How big a transport button is drawn in the now-playing bar.
pub const TRANSPORT: f32 = 15.0;

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
    transport(glyphs::PLAY, false)
}

pub fn pause<'a>() -> Svg<'a> {
    transport(glyphs::PAUSE, false)
}

pub fn previous<'a>() -> Svg<'a> {
    transport(glyphs::PREVIOUS, true)
}

pub fn next<'a>() -> Svg<'a> {
    transport(glyphs::NEXT, true)
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
    svg(svg::Handle::from_memory(if paused {
        glyphs::PLAY
    } else {
        glyphs::PAUSE
    }))
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
    svg(svg::Handle::from_memory(glyphs::TICK))
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
    svg(svg::Handle::from_memory(glyphs::MORE))
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

/// The mark on a menu entry that has more behind it.
///
/// What a submenu looks like everywhere, and the thing an ellipsis could not
/// say: `Add to playlist…` and `Rename…` are the same three dots, and one of
/// them opens a panel beside the entry while the other replaces what is under
/// it. A chevron points at where the panel is about to appear.
///
/// Dimmer than the label it sits beside: it says *how* this entry behaves, not
/// what it does.
pub fn chevron<'a>(lit: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(glyphs::CHEVRON))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(match lit {
                    true => palette.primary.base.text.scale_alpha(0.8),
                    false => palette.background.base.text.scale_alpha(0.55),
                }),
            }
        })
}

/// A glyph beside a line of text — a sidebar row, a menu entry.
///
/// Dimmer than the words it sits beside, on both sides of the highlight: the
/// icon is *which kind of thing this row is*, and a shape as loud as the name
/// would compete with four names for the same glance. `lit` is the row under
/// the cursor, where the one legible colour is the one the accent was paired
/// with.
pub fn line<'a>(shape: &'static [u8], lit: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(shape))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(match lit {
                    true => palette.primary.base.text.scale_alpha(0.9),
                    false => palette.background.base.text.scale_alpha(0.6),
                }),
            }
        })
}

/// Where the sound is coming from, in the now-playing bar.
///
/// The accent when it is *this* device and plain text when it is another,
/// which is the one thing the label beside it cannot say at a glance: "this
/// device" and "Phone" are both just words, and the color is what makes the
/// common case need no reading.
pub fn devices<'a>(here: bool) -> Svg<'a> {
    svg(svg::Handle::from_memory(glyphs::DEVICES))
        .width(TRANSPORT)
        .height(TRANSPORT)
        .style(move |theme: &Theme, _| {
            let palette = crate::palette::of(theme);
            svg::Style {
                color: Some(if here {
                    palette.primary.base.color
                } else {
                    palette.background.base.text.scale_alpha(0.75)
                }),
            }
        })
}

/// **Every shape in the table can actually draw.**
///
/// The pause button was invisible for as long as Lucide had been vendored, and
/// nothing said so. `<rect x="14" y="3" rx="1"/>` is well-formed SVG, resvg
/// renders it without complaint, and a rect with no width is nothing — so the
/// button laid out at the right size, took its click, and drew empty space.
/// Four glyphs were like it: `pause`, `library-big`, `smartphone` and
/// `circle-stop`, every one of them the ones built out of a `<rect>`.
///
/// The cause is the normalising that happens on the way *in*. `branding/` keeps
/// each icon stripped of the root `<svg>`'s `width` and `height`, because the
/// widget sets those — and the strip took the `width` and `height` off the
/// `<rect>`s inside as well, where they are the shape itself. `stroke-width`
/// survived because somebody had already been bitten by that one and written
/// the rule to spare it; a bare `width` on a child element is the same mistake
/// wearing the attribute's real name.
///
/// This is the same shape as every other trap in this program's drawing:
/// **something that fails to draw lays out perfectly.** A missing glyph in a
/// font, a canvas in a scrollable, a `container` gradient that never painted —
/// each looked like a rendering fault rather than the thing it was. So the
/// check is structural rather than visual: read the generated table as text and
/// assert that every element in it carries the attributes without which it is
/// not a shape.
///
/// It reads `glyphs.rs` with `include_str!` rather than a list of the constants
/// on purpose. The table is the program's vocabulary and half of it is drawn
/// only by the phone, so a list here would be a second table to keep in step —
/// which is the thing generating one file for both clients replaced.
#[cfg(test)]
mod tests {
    /// What each element needs before it is a shape rather than a no-op.
    const NEEDS: &[(&str, &[&str])] = &[
        ("rect", &["width", "height"]),
        ("circle", &["r"]),
        ("ellipse", &["rx", "ry"]),
        ("path", &["d"]),
        ("line", &["x1", "y1", "x2", "y2"]),
        ("polyline", &["points"]),
        ("polygon", &["points"]),
    ];

    /// Falsify it by deleting ` width="5"` from `PAUSE` in `glyphs.rs`: the
    /// failure names the glyph, the element and the attribute.
    #[test]
    fn every_glyph_can_draw() {
        let table = include_str!("glyphs.rs");
        let mut checked = 0;
        for line in table.lines() {
            let Some((name, body)) = line.split_once(": &[u8] = br##\"") else {
                continue;
            };
            let name = name.trim_start_matches("pub const ");
            // Past the root `<svg …>`, whose own width and height the widget
            // sets and the vendoring therefore strips.
            let inner = &body[body.find('>').expect("a root element") + 1..];
            for el in inner.split('<').skip(1) {
                let tag = el
                    .split([' ', '/', '>'])
                    .next()
                    .expect("a tag name after the angle bracket");
                let Some((_, needs)) = NEEDS.iter().find(|(t, _)| *t == tag) else {
                    // `</svg>` and the closing quote, and nothing else: an
                    // element this test has never seen is one to add above
                    // rather than to wave through.
                    assert!(
                        tag.starts_with('/') || tag.is_empty(),
                        "{name} draws a <{tag}>, which `NEEDS` does not describe"
                    );
                    continue;
                };
                for attr in *needs {
                    assert!(
                        el.contains(&format!("{attr}=\"")),
                        "{name}'s <{tag}> has no {attr}, so it draws nothing"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 60, "this has to be reading the table: {checked}");
    }
}
