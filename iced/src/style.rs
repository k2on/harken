//! What the widgets iced draws for us look like.
//!
//! [`crate::palette`] answers what a color *is*; this is where the handful of
//! widgets that would otherwise style themselves are told to use it. The
//! distinction matters because iced's own defaults are not neutral — a
//! `slider` with no style is drawn in the *theme's* primary, and the theme is
//! still iced's Dark, so the seek bar came out the blue-violet that ships with
//! iced in the middle of a gold-and-black player.
//!
//! That is the one thing asking `palette::of(theme)` in our own closures
//! cannot fix: those closures only run for widgets we have styled. Anything
//! left on a default reaches for `theme.extended_palette()` inside iced and
//! gets iced's answer. So every widget this program draws is styled here or
//! at its call site, and "it looked fine" is not evidence — the blue only
//! showed up once there was gold beside it.
use iced::widget::{button, scrollable, slider, text};
use iced::{Background, Border, Color, Theme};

use crate::palette;

/// Secondary text: a clock, a count, a hint.
///
/// `text::secondary` in iced's place, which read the theme's own palette and
/// so was a grey chosen against iced's background rather than ours.
pub fn dim(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(palette::of(theme).background.base.text.scale_alpha(0.6)),
    }
}

/// The seek bar: gold behind the handle, the track's own step of grey ahead
/// of it.
///
/// The two rail backgrounds are *played* and *remaining* in that order, which
/// is the whole reason the accent belongs on the first: what has gone by is
/// the part worth coloring, and a gold rail all the way across would say the
/// track was over.
pub fn seek(theme: &Theme, status: slider::Status) -> slider::Style {
    let palette = palette::of(theme);
    let gold = match status {
        slider::Status::Active => palette.primary.base.color,
        slider::Status::Hovered => palette.primary.strong.color,
        slider::Status::Dragged => palette.primary.weak.color,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (gold.into(), palette.background.strong.color.into()),
            width: 4.0,
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 7.0 },
            background: gold.into(),
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        },
    }
}

/// A button somebody is meant to press: signing in, going offline.
///
/// Filled with the accent, and its text is the one color that accent was
/// paired with — the same rule the cursor's row follows.
pub fn action(theme: &Theme, status: button::Status) -> button::Style {
    let palette = palette::of(theme);
    let (background, text_color) = match status {
        button::Status::Active => (palette.primary.base.color, palette.primary.base.text),
        button::Status::Hovered => (palette.primary.strong.color, palette.primary.strong.text),
        button::Status::Pressed => (palette.primary.weak.color, palette.primary.weak.text),
        button::Status::Disabled => (
            palette.background.weak.color,
            palette.background.base.text.scale_alpha(0.4),
        ),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The scrollbars.
///
/// Two things were wrong with the default and only one of them was the
/// palette: it draws a hovered scroller in `primary.strong`, so the bar went
/// iced's blue when you reached for it — and would have gone *gold* once it
/// was reading ours, which is no better. Gold means "this is playing, this is
/// hearted, press this"; a scrollbar is none of those. It is the text color
/// instead, which is white on a dark theme and near-black on a light one from
/// the same line.
pub fn bars(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let palette = palette::of(theme);
    let rail = |hovered: bool| scrollable::Rail {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            radius: 2.0.into(),
            ..Border::default()
        },
        scroller: scrollable::Scroller {
            background: Background::Color(if hovered {
                palette.background.base.text.scale_alpha(0.85)
            } else {
                palette.background.strongest.color
            }),
            border: Border {
                radius: 2.0.into(),
                ..Border::default()
            },
        },
    };
    let (v, h) = match status {
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered,
            is_horizontal_scrollbar_hovered,
            ..
        } => (
            rail(is_vertical_scrollbar_hovered),
            rail(is_horizontal_scrollbar_hovered),
        ),
        scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged,
            is_horizontal_scrollbar_dragged,
            ..
        } => (
            rail(is_vertical_scrollbar_dragged),
            rail(is_horizontal_scrollbar_dragged),
        ),
        _ => (rail(false), rail(false)),
    };
    scrollable::Style {
        vertical_rail: v,
        horizontal_rail: h,
        ..scrollable::default(theme, status)
    }
}
