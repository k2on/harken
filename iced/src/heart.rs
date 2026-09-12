//! A heart, drawn rather than typed.
//!
//! The obvious way to put a heart on a button is the character — and it does
//! not work here. iced embeds Fira Sans, whose cmap has no U+2665, no U+2661
//! and no U+2764, so the glyph silently draws nothing at all: widgets lay out,
//! input works, and the button is blank. That is the same failure mode
//! `CLAUDE.md` records for a browser build with no font at all, and it
//! is hard to recognise as a font problem when you meet it.
//!
//! So it is a path. Two cubics down each side, filled when the song is on the
//! playlist and stroked when it is not, which reads at a glance and needs no
//! font, no icon asset and no second colour to explain it.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{mouse, Color, Point, Rectangle, Renderer, Size, Theme};

/// The heart, at whatever size it is given.
pub struct Heart {
    pub filled: bool,
}

impl Heart {
    pub const SIZE: f32 = 22.0;
}

impl<Message> canvas::Program<Message> for Heart {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let Size {
            width: w,
            height: h,
        } = bounds.size();
        // Proportions of a plain valentine heart: the dip at the top centre,
        // a lobe either side, and the two curves meeting at the point.
        let at = |x: f32, y: f32| Point::new(x * w, y * h);
        let heart = Path::new(|p| {
            p.move_to(at(0.5, 0.90));
            p.bezier_curve_to(at(0.08, 0.60), at(0.02, 0.34), at(0.17, 0.20));
            p.bezier_curve_to(at(0.30, 0.08), at(0.43, 0.14), at(0.5, 0.28));
            p.bezier_curve_to(at(0.57, 0.14), at(0.70, 0.08), at(0.83, 0.20));
            p.bezier_curve_to(at(0.98, 0.34), at(0.92, 0.60), at(0.5, 0.90));
            p.close();
        });

        let red = Color::from_rgb(0.85, 0.21, 0.33);
        if self.filled {
            frame.fill(&heart, red);
        } else {
            frame.stroke(
                &heart,
                Stroke::default()
                    .with_color(Color::from_rgb(0.55, 0.55, 0.58))
                    .with_width(1.6),
            );
        }
        vec![frame.into_geometry()]
    }
}
