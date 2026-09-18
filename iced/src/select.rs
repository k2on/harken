//! Text you can drag across, and copy.
//!
//! iced's `text` draws a string and hears nothing: it has no `update`, so a
//! pointer dragged across a track title is a pointer dragged across a
//! picture. Upstream is fixing that — iced-rs/iced#3315 puts `.selectable()`
//! on `text` and `rich_text`, with a `selectable_group` that carries one drag
//! across sibling widgets — and this is the same idea against the 0.14 that
//! `Cargo.toml` pins: a widget of our own, over the two things a released
//! `Paragraph` already answers.
//!
//! Those two are what decide its shape. `hit_test` turns a point into a byte
//! offset, which is the selection; `grapheme_position` turns an offset back
//! into an x, which is the highlight. Both take a *line*, and the second one
//! is only ever asked about line 0 — so **this is for text on one line**, which
//! is what the table, its headings and the page headers are. Something that
//! wraps would select correctly and highlight only its first line, so it is
//! not used on anything that does.
//!
//! What it is not is a second `text`. It carries the four builders its call
//! sites use and no more; anything that wants centring or a font is drawing
//! something else.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::text::paragraph::Plain;
use iced::advanced::text::{self, Paragraph as _};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{clipboard, layout, mouse, renderer, Clipboard, Layout, Shell};
use iced::widget::text::{draw as draw_text, layout as layout_text, Format, IntoFragment, Style};
use iced::{keyboard, Element, Event, Length, Pixels, Point, Rectangle, Size};

type Renderer = iced::Renderer;
type Theme = iced::Theme;
type Paragraph = <Renderer as text::Renderer>::Paragraph;

/// How much of the text's own color is washed behind the selection.
///
/// A number rather than a palette entry, because the one ground this cannot
/// know is the one it is drawn on: the same cell is on the zebra, on the gold
/// cursor row and on the row that is playing, and a selection color chosen
/// against any one of those is wrong on the other two. The text is legible on
/// all three by construction — that is what `palette::of` already decided — so
/// a wash of *that* color is legible on all three too, and needs no second
/// decision per call site.
const WASH: f32 = 0.3;

/// Text that can be selected with the pointer and copied.
pub struct Selectable<'a> {
    fragment: text::Fragment<'a>,
    format: Format<iced::Font>,
    style: Box<dyn Fn(&Theme) -> Style + 'a>,
}

/// Text that can be selected with the pointer and copied.
pub fn selectable<'a>(fragment: impl IntoFragment<'a>) -> Selectable<'a> {
    Selectable {
        fragment: fragment.into_fragment(),
        format: Format::default(),
        style: Box::new(|_theme| Style::default()),
    }
}

impl<'a> Selectable<'a> {
    /// Sets the size of the text.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.format.size = Some(size.into());
        self
    }

    /// Sets the width of the text.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.format.width = width.into();
        self
    }

    /// Sets the wrapping strategy of the text.
    pub fn wrapping(mut self, wrapping: text::Wrapping) -> Self {
        self.format.wrapping = wrapping;
        self
    }

    /// Sets the style of the text, the way `text` takes one.
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self {
        self.style = Box::new(style);
        self
    }
}

/// What a `Selectable` remembers between frames.
#[derive(Default)]
struct State {
    paragraph: Plain<Paragraph>,
    /// Where the drag began and where it has got to, as byte offsets into the
    /// content. In that order rather than sorted, because a drag leftwards is
    /// an anchor with the head *before* it and the anchor is what stays put.
    span: Option<(usize, usize)>,
    /// The pointer is down and this is the widget it went down on.
    dragging: bool,
    /// This is the widget a copy would come from. Set by a press on it and
    /// dropped by a press anywhere else, which every widget hears — so the
    /// one selection on screen needs nothing to coordinate it.
    focused: bool,
    /// The last press, chained so that two in a row are a double click.
    clicked: Option<mouse::Click>,
}

impl State {
    /// Forget the selection, and that this was where it was.
    fn forget(&mut self) {
        self.span = None;
        self.dragging = false;
        self.focused = false;
        self.clicked = None;
    }

    /// The selection, in the order a string can be sliced with.
    fn range(&self) -> Option<(usize, usize)> {
        let (anchor, head) = self.span?;
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        (start < end).then_some((start, end))
    }

    /// Where the text is drawn, which is not where the widget is: a paragraph
    /// is anchored inside its bounds by its own alignment.
    fn anchor(&self, bounds: Rectangle) -> Point {
        let paragraph = self.paragraph.raw();
        bounds.anchor(
            paragraph.min_bounds(),
            paragraph.align_x(),
            paragraph.align_y(),
        )
    }

    /// The byte offset a point lands on.
    ///
    /// Clamped into the paragraph rather than refused outside it, because a
    /// drag that has left the cell still means something: past the end of the
    /// line is the end of the line, which is what dragging off the right of a
    /// title is asking for.
    fn offset_at(&self, bounds: Rectangle, point: Point) -> usize {
        let paragraph = self.paragraph.raw();
        let min = paragraph.min_bounds();
        let anchor = self.anchor(bounds);
        let x = (point.x - anchor.x).clamp(0.0, min.width);
        let y = (point.y - anchor.y).clamp(0.0, (min.height - 1.0).max(0.0));
        paragraph
            .hit_test(Point::new(x, y))
            .map(text::Hit::cursor)
            .unwrap_or_default()
    }

    /// Where the highlight goes, on the one line this widget is for.
    fn highlight(&self, bounds: Rectangle) -> Option<Rectangle> {
        let (start, end) = self.range()?;
        let paragraph = self.paragraph.raw();
        let content = self.paragraph.content();
        // `hit_test` answers in bytes and `grapheme_position` asks in
        // graphemes, so the two have to be bridged. Counting `chars` is that
        // bridge and is exact for everything without a combining mark on it;
        // a decomposed é would put the highlight's edge a fraction of a
        // character out, and nothing else.
        let at = |byte: usize| {
            let graphemes = content.get(..byte)?.chars().count();
            paragraph.grapheme_position(0, graphemes).map(|p| p.x)
        };
        let from = at(start)?;
        let to = at(end)?;
        let anchor = self.anchor(bounds);
        Some(Rectangle {
            x: anchor.x + from,
            y: anchor.y,
            width: to - from,
            height: paragraph.min_bounds().height,
        })
    }
}

/// The word around a byte offset, for a double click.
///
/// A word is a run of alphanumerics; anything else is one character of its
/// own, which is what makes a double click on a `,` select the `,` rather
/// than the sentence it is in.
fn word_at(content: &str, at: usize) -> (usize, usize) {
    let word = |c: char| c.is_alphanumeric();
    match content[at..].chars().next() {
        Some(c) if !word(c) => (at, at + c.len_utf8()),
        // Past the last glyph — which is what `hit_test` answers when the
        // pointer is off the end of the line — there is no character to be
        // on, so the word is the one that ends there.
        _ => {
            let start = content[..at]
                .char_indices()
                .rev()
                .take_while(|(_, c)| word(*c))
                .last()
                .map_or(at, |(i, _)| i);
            let end = content[at..]
                .char_indices()
                .find(|(_, c)| !word(*c))
                .map_or(content.len(), |(i, _)| at + i);
            (start, end)
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Selectable<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.format.width,
            height: self.format.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        // A row is a position in the tree and its state outlives whatever
        // string happens to be in it, so a list scrolled or a page opened
        // hands this widget somebody else's text. The offsets are bytes into
        // the old one: kept, they would highlight a span of a title nobody
        // selected.
        if state.paragraph.content() != &*self.fragment {
            state.forget();
        }
        layout_text(
            &mut state.paragraph,
            renderer,
            limits,
            &self.fragment,
            self.format,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(point) = cursor.position() else {
                    return;
                };
                if !cursor.is_over(bounds) {
                    // Every widget hears every press, so a press somewhere
                    // else is how this one learns it is no longer the
                    // selection — which is what keeps there being one.
                    if state.span.is_some() || state.focused {
                        state.forget();
                        shell.request_redraw();
                    }
                    return;
                }
                let at = state.offset_at(bounds, point);
                let click = mouse::Click::new(point, mouse::Button::Left, state.clicked);
                match click.kind() {
                    mouse::click::Kind::Single => {
                        state.span = Some((at, at));
                        state.dragging = true;
                    }
                    mouse::click::Kind::Double => {
                        state.span = Some(word_at(state.paragraph.content(), at));
                        state.dragging = false;
                    }
                    mouse::click::Kind::Triple => {
                        state.span = Some((0, state.paragraph.content().len()));
                        state.dragging = false;
                    }
                }
                state.clicked = Some(click);
                state.focused = true;
                // The press is ours, so nothing above it opens or plays on
                // one. What a click on a row still does it does on the
                // *release*, which is the half a drag does not reach.
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (Some(point), Some((anchor, head))) = (cursor.position(), state.span) else {
                    return;
                };
                if !state.dragging {
                    return;
                }
                let at = state.offset_at(bounds, point);
                if at != head {
                    state.span = Some((anchor, at));
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if !state.dragging {
                    return;
                }
                state.dragging = false;
                if state.range().is_some() {
                    // A drag that selected something is not a click, and the
                    // row under it must not take it as one.
                    shell.capture_event();
                } else {
                    state.span = None;
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if state.focused && modifiers.command() =>
            {
                let keyboard::Key::Character(c) = key else {
                    return;
                };
                match c.as_str() {
                    "c" | "C" => {
                        let Some((start, end)) = state.range() else {
                            return;
                        };
                        let Some(selected) = state.paragraph.content().get(start..end) else {
                            return;
                        };
                        clipboard.write(clipboard::Kind::Standard, selected.to_owned());
                        shell.capture_event();
                    }
                    "a" | "A" => {
                        let all = state.paragraph.content().len();
                        if all > 0 {
                            state.span = Some((0, all));
                            shell.capture_event();
                            shell.request_redraw();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let style = (self.style)(theme);
        let bounds = layout.bounds();
        // The one color the text is known to be legible on is its own, so the
        // wash is that — and the same line answers what `Style::color` leaves
        // to whatever is inherited, the way `text` itself does.
        let color = style.color.unwrap_or(defaults.text_color);
        if let Some(highlight) = state.highlight(bounds) {
            // Clipped to the viewport for the reason the text is: a cell in a
            // scrollable is drawn whether or not it is on screen.
            if let Some(visible) = highlight.intersection(viewport) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: visible,
                        ..renderer::Quad::default()
                    },
                    color.scale_alpha(WASH),
                );
            }
        }
        draw_text(
            renderer,
            defaults,
            bounds,
            state.paragraph.raw(),
            style,
            viewport,
        );
    }
}

impl<'a, Message: 'a> From<Selectable<'a>> for Element<'a, Message> {
    fn from(text: Selectable<'a>) -> Self {
        Element::new(text)
    }
}

#[cfg(test)]
mod tests {
    use super::word_at;

    #[test]
    fn a_double_click_takes_the_word_under_it() {
        let line = "Brandenburg Concerto No. 3, BWV 1048";
        assert_eq!(&line[range(word_at(line, 0))], "Brandenburg");
        assert_eq!(&line[range(word_at(line, 5))], "Brandenburg");
        assert_eq!(&line[range(word_at(line, 12))], "Concerto");
        assert_eq!(&line[range(word_at(line, 32))], "1048");
    }

    #[test]
    fn what_is_not_a_word_is_one_character() {
        // The space between two words, and the comma after a number: a double
        // click on either is asking about that character, not about the
        // sentence it happens to sit in.
        let line = "No. 3, BWV";
        assert_eq!(&line[range(word_at(line, 3))], " ");
        assert_eq!(&line[range(word_at(line, 5))], ",");
    }

    #[test]
    fn the_end_of_the_line_is_a_word_too() {
        // `hit_test` answers with the length itself when the pointer is past
        // the last glyph, which is an offset no `char_indices` ever yields —
        // so the walk has to end somewhere rather than run off.
        let line = "Water Music";
        assert_eq!(&line[range(word_at(line, line.len()))], "Music");
    }

    fn range((start, end): (usize, usize)) -> std::ops::Range<usize> {
        start..end
    }
}
