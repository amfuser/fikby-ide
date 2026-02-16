mod config;
mod editor;
mod highlight;
mod ui;
mod find_replace;

use gtk4::prelude::*;
use gtk4::{gdk, Application, CssProvider};

fn main() {
    let app = Application::builder()
        .application_id(config::APP_ID)
        .build();

    app.connect_startup(|_| {
        // Load CSS on startup
        load_css();
    });

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(config::CSS);
    
    // Add the provider to the default screen
    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}