//! Desktop entry point.

fn main() -> iced::Result {
    iced::application(
        exo_iced_demo::App::boot,
        exo_iced_demo::App::update,
        exo_iced_demo::App::view,
    )
    .title("exo · to-do")
    .window_size((820.0, 560.0))
    .run()
}
