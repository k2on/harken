//! A real libcosmic application, using their context menu and their table,
//! compiled for the browser.
use cosmic::app::{Core, Settings, Task};
use cosmic::widget::{menu, table};
use cosmic::{Application, Element};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Action {
    Play,
}

impl menu::action::MenuAction for Action {
    type Message = Message;
    fn message(&self) -> Message {
        Message::Play
    }
}

#[derive(Clone, Debug)]
enum Message {
    Play,
    Opened,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Column {
    #[default]
    Name,
}

impl table::ItemCategory for Column {
    fn width(&self) -> cosmic::iced::Length {
        cosmic::iced::Length::Fill
    }
}

impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Name")
    }
}

struct Track {
    name: String,
}

impl table::ItemInterface<Column> for Track {
    fn get_icon(&self, _: Column) -> Option<cosmic::widget::Icon> {
        None
    }
    fn get_text(&self, _: Column) -> std::borrow::Cow<'static, str> {
        self.name.clone().into()
    }
    fn compare(&self, other: &Self, _: Column) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

struct App {
    core: Core,
    opened: u32,
    tracks: table::Model<table::SingleSelect, Track, Column>,
}

impl Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "us.koon.harken.spike";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let mut tracks = table::Model::new(vec![Column::Name]);
        tracks.insert(Track {
            name: "Goldberg Variations, BWV 988".into(),
        });
        (App { core, opened: 0, tracks }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Drawn below, so the screenshot says whether the widget saw the
        // right-click at all — "no menu" and "no event" look identical.
        if matches!(message, Message::Opened) {
            self.opened += 1;
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let body = cosmic::widget::column::with_children(vec![
            cosmic::widget::text(format!("on_open fired {} time(s)", self.opened)).into(),
            table::table(&self.tracks).into(),
        ]);
        cosmic::widget::context_menu(
            body,
            Some(menu::items(
                &std::collections::HashMap::new(),
                vec![menu::Item::Button(String::from("Play"), None, Action::Play)],
            )),
        )
        .on_open(Message::Opened)
        .into()
    }
}

fn main() -> cosmic::iced::Result {
    // Without this a Rust panic on wasm is `RuntimeError: unreachable` and
    // nothing else — the message never reaches the console.
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    cosmic::app::run::<App>(Settings::default(), ())
}
