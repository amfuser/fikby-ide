use glib::object::ObjectExt;
use gio::Settings;

pub struct EditorSettings {
    pub settings: Settings,
}

impl EditorSettings {
    pub fn new() -> Self {
        let settings = Settings::new("io.github.amfuser.fikby_ide");
        EditorSettings { settings }
    }

    pub fn indent_style(&self) -> String {
        self.settings.get_string("indent-style").unwrap_or_else(|_| String::from("space"))
    }

    pub fn indent_width(&self) -> i32 {
        self.settings.get_integer("indent-width").unwrap_or(4)
    }
}