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
        (App { core, tracks }, Task::none())
    }

    fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let body = table::table(&self.tracks);
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
    cosmic::app::run::<App>(Settings::default(), ())
}
