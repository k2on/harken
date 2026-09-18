//! Frosted glass: a panel that shows a blurred copy of the page behind it.
//!
//! **This is the renderer's, not a compositor's, and that is the whole point.**
//! COSMIC frosts its own surfaces over `ext-background-effect-v1` — a Wayland
//! protocol, so it is nothing at all on X11, on macOS, on Windows, or in a
//! browser, where the program is one canvas with a page behind it that no
//! compositor is going to blur. And even where it works it blurs what is behind
//! the *window*, which for a menu drawn inside that window is the wallpaper
//! rather than the track list. See `iced/nix/libcosmic/` and the section in
//! CLAUDE.md.
//!
//! What is behind a panel here is something this renderer drew a frame's worth
//! of milliseconds ago, so it can be read back and blurred. `blur.rs` in the
//! patched `iced_wgpu` is that; this is the widget that asks for it.
//!
//! It is a *transparent* wrapper: `layout` returns the child's own node rather
//! than nesting one, so the frost occupies exactly the child's bounds and every
//! other method hands the layout straight down. A wrapper that padded or
//! centred would put the blur somewhere other than under the thing it is for.

use cosmic::blur;
use cosmic::iced::advanced::renderer;
use cosmic::iced::advanced::widget::{tree, Operation, Tree};
use cosmic::iced::advanced::{layout, mouse, overlay, Clipboard, Layout, Shell};
use cosmic::iced::{Color, Element, Event, Length, Rectangle, Size, Vector};

use crate::palette;

/// How far the blur reaches, in logical pixels.
///
/// About a third of a panel's width would be a wash; about a pixel would be a
/// smudge. Twelve is roughly what AppKit's `NSVisualEffectView` reads as at a
/// menu's size — enough that no letter behind it survives, which is the test
/// that matters, since a menu you can read the page through is worse than an
/// opaque one.
const SIGMA: f32 = 18.0;

/// How far the blurred copy is carried towards the panel's own ground.
///
/// Not zero, and this is the number that decides whether it looks like glass or
/// like a smear. A pure blur of a dark track list is a dark panel with no edge
/// to it; the tint is what puts the panel on a plane of its own and what keeps
/// its text legible against whatever happened to be underneath.
const TINT: f32 = 0.45;

pub fn frost<'a, Message: 'a>(
    content: impl Into<Element<'a, Message, cosmic::Theme, cosmic::Renderer>>,
    radius: f32,
) -> Element<'a, Message, cosmic::Theme, cosmic::Renderer> {
    Element::new(Frost {
        content: content.into(),
        radius: [radius; 4],
    })
}

struct Frost<'a, Message> {
    content: Element<'a, Message, cosmic::Theme, cosmic::Renderer>,
    radius: [f32; 4],
}

impl<Message> cosmic::iced::advanced::Widget<Message, cosmic::Theme, cosmic::Renderer>
    for Frost<'_, Message>
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.as_widget_mut().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        // Before the child, because the child is the panel's border and its
        // rows, and this is the ground they stand on.
        //
        // The tint is the theme's, asked for here rather than written down, for
        // the reason every other colour in this program is: `palette::of` is
        // what makes dark mode work, and a frosted panel that ignored it would
        // be the one surface with a colour chosen against one theme.
        let palette = palette::of(theme);
        let ground = palette.background.weak.color;

        blur::Renderer::draw_blur(
            renderer,
            layout.bounds(),
            self.radius,
            SIGMA,
            Color { a: TINT, ..ground },
        );

        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &cosmic::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
