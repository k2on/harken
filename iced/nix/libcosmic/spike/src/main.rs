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
    LeftPress,
    RightPress,
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
    left: u32,
    right: u32,
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
        (App { core, opened: 0, left: 0, right: 0, tracks }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Drawn below, so the screenshot says whether the widget saw the
        // right-click at all — "no menu" and "no event" look identical.
        match message {
            Message::Opened => self.opened += 1,
            Message::LeftPress => self.left += 1,
            Message::RightPress => self.right += 1,
            Message::Play => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        // Three levels, so one run says where a right-click stops: a plain
        // button proves button events arrive at all, a `mouse_area` proves the
        // *right* button arrives, and `on_open` is the context menu itself.
        let body = cosmic::widget::column::with_children(vec![
            cosmic::widget::text(format!(
                "button(left)={}  mouse_area(right)={}  context_menu(on_open)={}",
                self.left, self.right, self.opened
            ))
            .into(),
            cosmic::widget::button::standard("press me")
                .on_press(Message::LeftPress)
                .into(),
            cosmic::widget::mouse_area(
                cosmic::widget::container(cosmic::widget::text("right-click this box"))
                    .padding(20)
                    .width(cosmic::iced::Length::Fill),
            )
            .on_right_press(Message::RightPress)
            .into(),
            table::table(&self.tracks).into(),
        ]);
        cosmic::widget::context_menu(
            body,
            Some(menu::items(
                &std::collections::HashMap::new(),
                vec![menu::Item::Button(String::from("Go to Goldberg Variations, BWV 988"), None, Action::Play)],
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
