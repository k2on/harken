//! Vim's grammar, and the one hook a component implements to get it.
//!
//! Three things are kept apart on purpose, because each is useful without the
//! others and mixing them is what makes keyboard code untestable:
//!
//! 1. **[`Keys`] turns key presses into an [`Action`].** It knows `5j` is five
//!    downs and `gg` is the top, and nothing about what is on screen. It is a
//!    state machine over characters and can be tested by pressing letters at
//!    it.
//! 2. **[`Navigate`] turns a [`Motion`] into a new cursor.** This is the hook.
//!    A component says how many cells it has and where a motion lands; it gets
//!    counts, `gg`, `G` and `{count}G` for free, and never sees a key.
//! 3. **The application does the rest** — which pane has the cursor, what
//!    `Activate` means, what the search text matches against. None of that
//!    generalises, so none of it is here.
//!
//! The part worth explaining is what [`Navigate::step`] returning `None`
//! means: *not mine*. A vertical [`List`] refuses `h` and `l` because it has no
//! horizontal axis at all, and the caller is then free to read that as "leave
//! this pane" — which is how the sidebar and the track list are moved between
//! without either of them knowing the other exists. A [`Grid`] accepts all
//! four and clamps at its edges, because it *does* have both axes: there the
//! motion was mine and there was simply nowhere further to go. Refusing and
//! clamping are different answers to different questions, and keeping them
//! apart is what lets one grammar serve a list, a row and a grid.

use iced::keyboard::{key::Named, Key, Modifiers};

/// How far a page moves. Vim's `^D` is half a window; a component here does not
/// know how tall its window is, so this is a fixed step and says so rather than
/// pretending to measure.
pub const PAGE: usize = 10;

/// Where a motion wants to go, with no idea what it is moving through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Down(usize),
    Up(usize),
    Left(usize),
    Right(usize),
    /// `gg`
    First,
    /// `G`
    Last,
    /// `{count}G` — a cell number, counting from one the way vim counts lines.
    To(usize),
}

/// A finished command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Move(Motion),
    /// `<Enter>` or `o` — open what the cursor is on.
    Activate,
    /// `<Space>` — the row's own toggle. A heart, here.
    Toggle,
    /// A `/` search was typed and accepted.
    Search(String),
    /// `n` forwards, `N` back.
    Match(isize),
    /// `<Tab>` — the next pane, whatever the panes are.
    Cycle,
    /// `<Esc>` — drop whatever was half-typed.
    Cancel,
    /// Keys an application binds for itself: `p` for play/pause here. Held
    /// apart from the motions so that adding one cannot change the grammar.
    Key(char),
}

/// What [`Keys`] is in the middle of.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    /// `/` was pressed; this is what has been typed since.
    Search(String),
}

/// The keys pressed so far, and what they have not yet added up to.
#[derive(Debug, Default)]
pub struct Keys {
    count: Option<usize>,
    /// A prefix waiting for its second key — only `g` today.
    pending: Option<char>,
    mode: Mode,
}

impl Keys {
    pub fn new() -> Keys {
        Keys::default()
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// What to show so the typist can see what the editor thinks it has: `5`,
    /// `2g`, `/chop`. Empty when nothing is half-typed, which is most of the
    /// time.
    pub fn pending(&self) -> String {
        match &self.mode {
            Mode::Search(q) => format!("/{q}"),
            Mode::Normal => {
                let mut s = String::new();
                if let Some(n) = self.count {
                    s.push_str(&n.to_string());
                }
                if let Some(c) = self.pending {
                    s.push(c);
                }
                s
            }
        }
    }

    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    /// Feed one key press. `None` means "still typing".
    pub fn press(&mut self, key: &Key, mods: Modifiers) -> Option<Action> {
        // A search swallows everything until it is accepted or abandoned, so
        // that `/` then `j` searches for a j rather than moving.
        if let Mode::Search(query) = &mut self.mode {
            return match key {
                Key::Named(Named::Enter) => {
                    let q = std::mem::take(query);
                    self.mode = Mode::Normal;
                    (!q.is_empty()).then_some(Action::Search(q))
                }
                Key::Named(Named::Escape) => {
                    self.mode = Mode::Normal;
                    Some(Action::Cancel)
                }
                Key::Named(Named::Backspace) => {
                    query.pop();
                    None
                }
                Key::Named(Named::Space) => {
                    query.push(' ');
                    None
                }
                Key::Character(c) => {
                    query.push_str(c);
                    None
                }
                _ => None,
            };
        }

        // `^D` and `^U`, which are the only chorded keys in the grammar.
        if mods.control() {
            if let Key::Character(c) = key {
                return match c.as_str() {
                    "d" => Some(Action::Move(Motion::Down(PAGE))),
                    "u" => Some(Action::Move(Motion::Up(PAGE))),
                    _ => None,
                };
            }
            return None;
        }

        match key {
            Key::Named(Named::Escape) => {
                self.count = None;
                self.pending = None;
                Some(Action::Cancel)
            }
            Key::Named(Named::Enter) => {
                self.count = None;
                self.pending = None;
                Some(Action::Activate)
            }
            Key::Named(Named::Space) => {
                self.count = None;
                self.pending = None;
                Some(Action::Toggle)
            }
            Key::Named(Named::Tab) => Some(Action::Cycle),
            Key::Named(Named::ArrowDown) => Some(Action::Move(Motion::Down(self.take_count()))),
            Key::Named(Named::ArrowUp) => Some(Action::Move(Motion::Up(self.take_count()))),
            Key::Named(Named::ArrowLeft) => Some(Action::Move(Motion::Left(self.take_count()))),
            Key::Named(Named::ArrowRight) => Some(Action::Move(Motion::Right(self.take_count()))),
            Key::Character(c) => self.character(c.as_str()),
            _ => None,
        }
    }

    fn character(&mut self, c: &str) -> Option<Action> {
        // A count is built up digit by digit, and a leading zero is not a count
        // — in vim `0` is a motion. There is no `0` motion here, so it is
        // simply ignored rather than starting a count of nothing.
        if let Ok(d) = c.parse::<usize>() {
            if d == 0 && self.count.is_none() {
                return None;
            }
            self.count = Some(self.count.unwrap_or(0) * 10 + d);
            return None;
        }

        if self.pending == Some('g') {
            self.pending = None;
            let count = self.take_count();
            return match c {
                // `gg` is the top, and `{count}gg` is that cell, like vim.
                "g" => Some(Action::Move(if count > 1 {
                    Motion::To(count)
                } else {
                    Motion::First
                })),
                _ => None,
            };
        }

        match c {
            "g" => {
                self.pending = Some('g');
                None
            }
            "j" => Some(Action::Move(Motion::Down(self.take_count()))),
            "k" => Some(Action::Move(Motion::Up(self.take_count()))),
            "h" => Some(Action::Move(Motion::Left(self.take_count()))),
            "l" => Some(Action::Move(Motion::Right(self.take_count()))),
            // `{count}G` is a cell number; a bare `G` is the end.
            "G" => Some(Action::Move(match self.count.take() {
                Some(n) => Motion::To(n),
                None => Motion::Last,
            })),
            "o" => Some(Action::Activate),
            "/" => {
                self.count = None;
                self.mode = Mode::Search(String::new());
                None
            }
            "n" => Some(Action::Match(1)),
            "N" => Some(Action::Match(-1)),
            other => other.chars().next().map(Action::Key),
        }
    }
}

/// How a component answers a motion — the whole extension point.
///
/// Implement it and the component has vim's motions, counts and jumps without
/// writing any of them. `step` returns `None` for a motion the shape has no
/// axis for, which the caller may read as "this belongs to whatever contains
/// me"; it clamps instead when the axis exists and the cursor is already at the
/// end of it.
pub trait Navigate {
    /// How many cells the cursor can be on.
    fn cells(&self) -> usize;

    /// Where `motion` lands, starting from `from`.
    fn step(&self, from: usize, motion: Motion) -> Option<usize>;
}

/// Clamp a cell number to a shape that has `cells` of them.
fn clamp(at: usize, cells: usize) -> usize {
    at.min(cells.saturating_sub(1))
}

/// A cell number as vim writes it — counting from one — as an index.
fn nth(n: usize, cells: usize) -> usize {
    clamp(n.saturating_sub(1), cells)
}

/// One column, top to bottom. `h` and `l` are refused: a list has no
/// horizontal axis, so those keys are free to mean something to whatever holds
/// it.
#[derive(Debug, Clone, Copy)]
pub struct List {
    pub cells: usize,
}

impl Navigate for List {
    fn cells(&self) -> usize {
        self.cells
    }

    fn step(&self, from: usize, motion: Motion) -> Option<usize> {
        match motion {
            Motion::Down(n) => Some(clamp(from.saturating_add(n), self.cells)),
            Motion::Up(n) => Some(from.saturating_sub(n)),
            Motion::Left(_) | Motion::Right(_) => None,
            Motion::First => Some(0),
            Motion::Last => Some(clamp(usize::MAX, self.cells)),
            Motion::To(n) => Some(nth(n, self.cells)),
        }
    }
}

/// One row, left to right. The mirror of [`List`]: `j` and `k` are refused.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub cells: usize,
}

impl Navigate for Row {
    fn cells(&self) -> usize {
        self.cells
    }

    fn step(&self, from: usize, motion: Motion) -> Option<usize> {
        match motion {
            Motion::Right(n) => Some(clamp(from.saturating_add(n), self.cells)),
            Motion::Left(n) => Some(from.saturating_sub(n)),
            Motion::Down(_) | Motion::Up(_) => None,
            Motion::First => Some(0),
            Motion::Last => Some(clamp(usize::MAX, self.cells)),
            Motion::To(n) => Some(nth(n, self.cells)),
        }
    }
}

/// `columns` wide, filled left to right and then down — so cell `n` is at row
/// `n / columns` and column `n % columns`.
///
/// Nothing in this application is a grid yet. It is here because it is the case
/// that proves [`Navigate`] is not shaped around the one caller: a grid accepts
/// every motion and clamps at its edges, where a list refuses two of them
/// outright, and the same `Keys` drives both without knowing which it has.
// Nothing in this window is a grid, so nothing constructs one. It is kept —
// and tested — because a hook with one implementation is not a hook: `List`
// alone could not tell you whether `Navigate` was a general shape or a
// description of the track list. `Grid` is what makes the difference between
// refusing a motion and clamping it visible, and it is twenty lines.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub cells: usize,
    pub columns: usize,
}

impl Navigate for Grid {
    fn cells(&self) -> usize {
        self.cells
    }

    fn step(&self, from: usize, motion: Motion) -> Option<usize> {
        let columns = self.columns.max(1);
        let (row, column) = (from / columns, from % columns);
        let landing = match motion {
            // Down and up keep the column, which is what makes a grid feel like
            // a grid: the cursor travels in a straight line rather than
            // wrapping through the end of a row.
            Motion::Down(n) => (row.saturating_add(n)) * columns + column,
            Motion::Up(n) => (row.saturating_sub(n)) * columns + column,
            // Left and right stay on their row, so `l` at the right-hand edge
            // stops rather than dropping to the next row.
            Motion::Right(n) => row * columns + (column.saturating_add(n)).min(columns - 1),
            Motion::Left(n) => row * columns + column.saturating_sub(n),
            Motion::First => 0,
            Motion::Last => self.cells.saturating_sub(1),
            Motion::To(n) => nth(n, self.cells),
        };
        // A short last row means a column can exist on one row and not the
        // next, so every landing is clamped to what is actually there.
        Some(clamp(landing, self.cells))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keys: &mut Keys, s: &str) -> Option<Action> {
        let key = match s {
            "<Enter>" => Key::Named(Named::Enter),
            "<Esc>" => Key::Named(Named::Escape),
            "<Space>" => Key::Named(Named::Space),
            "<Tab>" => Key::Named(Named::Tab),
            c => Key::Character(c.into()),
        };
        keys.press(&key, Modifiers::default())
    }

    /// The count is the part that is easy to get wrong: it accumulates across
    /// presses and has to be spent exactly once.
    #[test]
    fn a_count_is_built_up_and_spent_once() {
        let mut keys = Keys::new();
        assert_eq!(press(&mut keys, "1"), None);
        assert_eq!(press(&mut keys, "2"), None);
        assert_eq!(keys.pending(), "12");
        assert_eq!(press(&mut keys, "j"), Some(Action::Move(Motion::Down(12))));
        // Spent: the next motion is a single step, not another twelve.
        assert_eq!(press(&mut keys, "j"), Some(Action::Move(Motion::Down(1))));
        assert_eq!(keys.pending(), "");
    }

    #[test]
    fn gg_is_the_top_and_g_is_the_end() {
        let mut keys = Keys::new();
        assert_eq!(press(&mut keys, "g"), None, "g waits for its second key");
        assert_eq!(keys.pending(), "g");
        assert_eq!(press(&mut keys, "g"), Some(Action::Move(Motion::First)));
        assert_eq!(press(&mut keys, "G"), Some(Action::Move(Motion::Last)));
        // A count turns either of them into "go to that cell".
        assert_eq!(press(&mut keys, "7"), None);
        assert_eq!(press(&mut keys, "G"), Some(Action::Move(Motion::To(7))));
    }

    #[test]
    fn escape_drops_what_was_half_typed() {
        let mut keys = Keys::new();
        press(&mut keys, "4");
        press(&mut keys, "g");
        assert_eq!(keys.pending(), "4g");
        assert_eq!(press(&mut keys, "<Esc>"), Some(Action::Cancel));
        assert_eq!(keys.pending(), "");
        assert_eq!(press(&mut keys, "j"), Some(Action::Move(Motion::Down(1))));
    }

    /// A search has to swallow the motion keys, or typing a composer's name
    /// walks the cursor down the list instead.
    #[test]
    fn a_search_swallows_everything_until_it_is_accepted() {
        let mut keys = Keys::new();
        assert_eq!(press(&mut keys, "/"), None);
        for c in ["j", "o", "h", "n"] {
            assert_eq!(press(&mut keys, c), None, "{c} is text, not a motion");
        }
        assert_eq!(keys.pending(), "/john");
        assert_eq!(
            press(&mut keys, "<Enter>"),
            Some(Action::Search("john".into()))
        );
        assert_eq!(*keys.mode(), Mode::Normal);
        // And afterwards the same keys are motions again.
        assert_eq!(press(&mut keys, "j"), Some(Action::Move(Motion::Down(1))));
    }

    #[test]
    fn ctrl_d_and_ctrl_u_are_pages() {
        let mut keys = Keys::new();
        let ctrl = Modifiers::CTRL;
        let d = Key::Character("d".into());
        assert_eq!(keys.press(&d, ctrl), Some(Action::Move(Motion::Down(PAGE))));
    }

    #[test]
    fn a_list_refuses_the_horizontal_and_clamps_the_vertical() {
        let list = List { cells: 5 };
        assert_eq!(list.step(0, Motion::Down(2)), Some(2));
        assert_eq!(list.step(4, Motion::Down(9)), Some(4), "clamped at the end");
        assert_eq!(list.step(0, Motion::Up(9)), Some(0), "clamped at the top");
        assert_eq!(list.step(2, Motion::First), Some(0));
        assert_eq!(list.step(2, Motion::Last), Some(4));
        assert_eq!(list.step(0, Motion::To(3)), Some(2), "cells count from one");
        // The refusal is the whole pane mechanism: `None` is not "nowhere to
        // go", it is "not mine".
        assert_eq!(list.step(2, Motion::Left(1)), None);
        assert_eq!(list.step(2, Motion::Right(1)), None);
    }

    #[test]
    fn a_row_is_a_list_on_its_side() {
        let row = Row { cells: 3 };
        assert_eq!(row.step(0, Motion::Right(2)), Some(2));
        assert_eq!(row.step(2, Motion::Right(1)), Some(2));
        assert_eq!(row.step(0, Motion::Down(1)), None);
        assert_eq!(row.step(0, Motion::Up(1)), None);
    }

    /// A grid takes all four and clamps, which is the difference from a list
    /// that the caller depends on to decide whether to change pane.
    #[test]
    fn a_grid_keeps_its_column_going_down_and_its_row_going_across() {
        //  0 1 2
        //  3 4 5
        //  6 7
        let grid = Grid {
            cells: 8,
            columns: 3,
        };
        assert_eq!(grid.step(1, Motion::Down(1)), Some(4), "same column");
        assert_eq!(grid.step(4, Motion::Up(1)), Some(1));
        assert_eq!(grid.step(3, Motion::Right(1)), Some(4));
        assert_eq!(grid.step(5, Motion::Right(1)), Some(5), "the row ends here");
        assert_eq!(grid.step(3, Motion::Left(1)), Some(3), "and starts here");
        // Down the short column: cell 2 has nothing under 5, so it clamps to
        // the last cell rather than landing outside the grid.
        assert_eq!(grid.step(5, Motion::Down(1)), Some(7));
        assert_eq!(grid.step(0, Motion::Last), Some(7));
        // Every motion is answered, so nothing bubbles out of a grid.
        for m in [
            Motion::Left(1),
            Motion::Right(1),
            Motion::Up(1),
            Motion::Down(1),
        ] {
            assert!(grid.step(4, m).is_some(), "{m:?} is a grid's own");
        }
    }

    #[test]
    fn an_empty_shape_has_nowhere_to_go() {
        let list = List { cells: 0 };
        assert_eq!(list.step(0, Motion::Down(1)), Some(0));
        assert_eq!(list.step(0, Motion::Last), Some(0));
    }
}
