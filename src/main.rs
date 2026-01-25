use gtk4::prelude::*;
use gtk4::Application;

mod ui;
mod editor;
mod highlight;
mod config;

fn main() {
    let app = Application::builder()
        .application_id(config::APP_ID)
        .build();

    app.connect_activate(|app| {
        ui::build_ui(app);
    });
    app.run();
}