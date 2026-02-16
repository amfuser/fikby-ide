mod config;
mod editor;
mod file_explorer;
mod highlight;
mod ui;
mod find_replace;

#[cfg(test)]
mod tests;

use gtk4::prelude::*;
use gtk4::{gdk, Application, CssProvider};
use config::ThemeMode;

fn main() {
    let app = Application::builder()
        .application_id(config::APP_ID)
        .build();

    app.connect_startup(|_| {
        // Load default CSS on startup (dark theme by default)
        load_css(ThemeMode::Dark);
    });

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}

pub fn load_css(theme: ThemeMode) {
    let provider = CssProvider::new();
    provider.load_from_data(theme.css());
    
    // Add the provider to the default screen
    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}