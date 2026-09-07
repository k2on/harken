//! A to-do list on Exo, in iced, on the desktop (`just desktop`) and in a
//! browser (`just web`).
//!
//! The same [`exo::Client`] drives both. What differs between the two targets
//! is one line — where the database lives — and that is the point: the engine
//! is sans-io and has no idea which of the two it is in. How SQLite comes to
//! exist in a browser at all is in `docs/decisions.md`.

pub mod todo;

use exo::{AutoCtx, Client, Id};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Task};

use todo::{list, Item, Todo, TodoApp};

/// Where the database lives is the only difference between the two targets.
#[cfg(not(target_arch = "wasm32"))]
fn open() -> exo::Result<exo::Connection> {
    let dir = std::env::temp_dir();
    exo::open_path(dir.join("exo-iced-demo.db"))
}

/// In the browser, in memory: `sqlite-wasm-rs` registers a memory VFS by
/// default. It can also persist to OPFS, which needs an async handshake before
/// the first query — worth doing, and not what this example is for.
#[cfg(target_arch = "wasm32")]
fn open() -> exo::Result<exo::Connection> {
    exo::open_memory()
}

#[derive(Debug, Clone)]
pub enum Message {
    Typed(String),
    Add,
    Toggle(Id, bool),
    Remove(Id),
}

pub struct App {
    client: Client<TodoApp>,
    /// The materialised view, refreshed after every mutation.
    ///
    /// iced's `view` takes `&self` and Diesel needs `&mut` even to read, so the
    /// query cannot happen during rendering. Keeping the rows here is the right
    /// shape anyway — and it is exactly the seam that reactive queries would
    /// slot into later.
    items: Vec<Item>,
    /// Cached for the same reason as `items`: `view` gets `&self`, and asking
    /// Exo how much is unconfirmed needs `&mut`.
    pending: usize,
    input: String,
    note: String,
}

impl App {
    pub fn boot() -> Self {
        let client = Client::<TodoApp>::open(
            open().expect("open the database"),
            "iced",
            AutoCtx::system(),
        )
        .expect("open the exo client");
        let mut app = App {
            client,
            items: Vec::new(),
            pending: 0,
            input: String::new(),
            note: String::new(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.items = list(self.client.conn()).unwrap_or_default();
        self.pending = self.client.pending_len();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let outcome = match message {
            Message::Typed(text) => {
                self.input = text;
                Ok(())
            }
            Message::Add => {
                let text = std::mem::take(&mut self.input);
                self.client.mutate(Todo::add(&text)).map(|_| ())
            }
            Message::Toggle(id, done) => self.client.mutate(Todo::SetDone { id, done }).map(|_| ()),
            Message::Remove(id) => self.client.mutate(Todo::Remove { id }).map(|_| ()),
        };
        // A mutation the app itself refuses never reaches the pending queue.
        self.note = match outcome {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        };
        self.refresh();
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let rows = self.items.iter().fold(column![].spacing(6), |col, item| {
            col.push(
                row![
                    checkbox(item.done).on_toggle(move |done| Message::Toggle(item.id, done)),
                    text(item.text.clone()).width(Length::Fill),
                    button("remove").on_press(Message::Remove(item.id)),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            )
        });

        let entry = row![
            text_input("a new to-do…", &self.input)
                .on_input(Message::Typed)
                .on_submit(Message::Add)
                .width(Length::Fill),
            button("add").on_press(Message::Add),
        ]
        .spacing(12);

        // The status line is the engine showing through: `cursor` is how much of
        // the server's log has been applied, `pending` is what this client has
        // done that no server has confirmed. With no server, everything stays
        // pending and the list works anyway.
        let status = text(format!(
            "cursor {} · {} pending{}",
            self.client.cursor(),
            self.pending,
            if self.note.is_empty() {
                String::new()
            } else {
                format!("  ·  {}", self.note)
            }
        ))
        .size(13);

        container(
            column![
                text("exo · to-do").size(26),
                entry,
                scrollable(rows).height(Length::Fill),
                status,
            ]
            .spacing(16),
        )
        .padding(24)
        .into()
    }
}

/// Browser entry point. `wasm-bindgen` calls this when the module loads; iced
/// appends its canvas to `<body>` and takes it from there.
///
/// This does not build yet: `wasm32-unknown-unknown` has no SQLite. See
/// "The browser build is blocked on SQLite" in `docs/decisions.md`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    // Without this a panic is an unhelpful "unreachable executed".
    console_error_panic_hook::set_once();
    let _ = iced::application(App::boot, App::update, App::view)
        .title("exo · to-do")
        .run();
}

/// Exercises Exo end to end and reports what happened, so a browser can prove
/// the engine works there independently of whether anything renders.
///
/// Called from the page as `?selftest`; the desktop has the test suite instead.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn exo_self_test() -> String {
    console_error_panic_hook::set_once();
    let mut app = App::boot();
    app.update(Message::Typed("buy oat milk".into()));
    app.update(Message::Add);
    app.update(Message::Typed("book the ferry".into()));
    app.update(Message::Add);
    let first = app.items[0].id;
    app.update(Message::Toggle(first, true));
    // A mutation the app itself refuses must not reach the pending queue.
    app.update(Message::Typed("   ".into()));
    app.update(Message::Add);
    let refused = !app.note.is_empty();

    format!(
        "items={:?} done={:?} pending={} cursor={} refused_empty={}",
        app.items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>(),
        app.items.iter().map(|i| i.done).collect::<Vec<_>>(),
        app.pending,
        app.client.cursor(),
        refused,
    )
}
