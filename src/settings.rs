use gio::prelude::*;
use gio::{Settings, SettingsSchemaSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces,
    Tabs,
}

impl IndentStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            IndentStyle::Spaces => "spaces",
            IndentStyle::Tabs => "tabs",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "tabs" => IndentStyle::Tabs,
            _ => IndentStyle::Spaces,
        }
    }
}

#[derive(Debug, Clone)]
enum Backend {
    /// Uses real GSettings (persistent).
    GSettings(Settings),
    /// Fallback when schema is missing (non-persistent, but app keeps running).
    Fallback {
        indent_style: IndentStyle,
        indent_width: i32,
    },
}

#[derive(Debug, Clone)]
pub struct EditorSettings {
    backend: Backend,
}

impl EditorSettings {
    pub const SCHEMA_ID: &'static str = "io.github.amfuser.fikby_ide";

    pub fn new() -> Self {
        // IMPORTANT:
        // Settings::new(schema_id) will abort the process if the schema doesn't exist.
        // So we first check schema availability via SettingsSchemaSource.
        let schema_found = SettingsSchemaSource::default()
            .and_then(|src| src.lookup(Self::SCHEMA_ID, true))
            .is_some();

        if schema_found {
            Self {
                backend: Backend::GSettings(Settings::new(Self::SCHEMA_ID)),
            }
        } else {
            eprintln!(
                "Warning: GSettings schema '{}' not found. Using non-persistent defaults.",
                Self::SCHEMA_ID
            );
            Self {
                backend: Backend::Fallback {
                    indent_style: IndentStyle::Spaces,
                    indent_width: 4,
                },
            }
        }
    }

    pub fn indent_style(&self) -> IndentStyle {
        match &self.backend {
            Backend::GSettings(s) => IndentStyle::from_str(s.string("indent-style").as_str()),
            Backend::Fallback { indent_style, .. } => *indent_style,
        }
    }

    pub fn set_indent_style(&self, style: IndentStyle) {
        match &self.backend {
            Backend::GSettings(s) => {
                let _ = s.set_string("indent-style", style.as_str());
            }
            Backend::Fallback { .. } => {
                // No persistence in fallback mode.
                // If you want this to be mutable, we can switch to Rc<RefCell<...>>.
            }
        }
    }

    pub fn indent_width(&self) -> i32 {
        match &self.backend {
            Backend::GSettings(s) => s.int("indent-width"),
            Backend::Fallback { indent_width, .. } => *indent_width,
        }
    }

    pub fn set_indent_width(&self, width: i32) {
        let width = width.clamp(1, 8);
        match &self.backend {
            Backend::GSettings(s) => {
                let _ = s.set_int("indent-width", width);
            }
            Backend::Fallback { .. } => {
                // No persistence in fallback mode.
            }
        }
    }

    /// True if settings are backed by installed GSettings schema.
    pub fn is_persistent(&self) -> bool {
        matches!(self.backend, Backend::GSettings(_))
    }
}
