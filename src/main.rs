use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Application, CssProvider, StyleContext};
use std::cell::RefCell;

pub mod config;
pub mod editor;
pub mod file_explorer;
pub mod find_replace;
pub mod highlight;
pub mod settings;
pub mod ui;

// Full embedded themes (no external CSS files).
// This includes fixes for PopoverMenu items appearing too light/washed out.
const CSS_DARK: &str = r#"
/* Base */
window, .background {
  background-color: #1e1e1e;
  color: #e6e6e6;
}

label { color: #e6e6e6; }

separator { background: #3a3a3a; }

/* Menubar row */
.menubar {
  background-color: #252526;
  border-bottom: 1px solid #3a3a3a;
  padding: 4px;
}

/* Menu buttons */
.menubutton { margin: 0 2px; }

.menubutton button,
.menubutton > button {
  background: transparent;
  color: #e6e6e6;
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 4px 10px;
}

.menubutton button:hover,
.menubutton > button:hover {
  background: #333333;
  border-color: #444444;
}

/* Generic buttons */
button {
  background-color: #2d2d2d;
  color: #e6e6e6;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 6px 10px;
}

button:hover { background-color: #3a3a3a; }
button:active { background-color: #444444; }

/* Right-side icon button (settings) */
.right-button {
  margin-right: 6px;
  padding: 4px;
}

/* Inputs */
entry, spinbutton, combobox, textview {
  background-color: #1b1b1b;
  color: #e6e6e6;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
}

entry:focus, spinbutton:focus, combobox:focus {
  border-color: #6a9fb5;
  box-shadow: none;
}

/* Notebook + tabs */
notebook { background-color: #1e1e1e; }

notebook > header {
  background-color: #252526;
  border-bottom: 1px solid #3a3a3a;
}

notebook > header tab {
  background-color: #2d2d2d;
  border: 1px solid #3a3a3a;
  border-bottom: none;
  border-radius: 6px 6px 0 0;
  margin: 2px 2px 0 2px;
  padding: 4px 8px;
}

notebook > header tab:hover { background-color: #3a3a3a; }

notebook > header tab:checked {
  background-color: #1e1e1e;
  border-color: #6a9fb5;
}

/* Status bar */
.status {
  background-color: #252526;
  border-top: 1px solid #3a3a3a;
  padding: 6px;
}

/* Gutter + editor view */
.gutter {
  background-color: #252526;
  border-right: 1px solid #3a3a3a;
}

.editor-view {
  background-color: #1e1e1e;
  color: #e6e6e6;
}

/* Popovers / dialogs */
popover, dialog {
  background-color: #252526;
  color: #e6e6e6;
  border: 1px solid #3a3a3a;
  border-radius: 8px;
}

/* --- PopoverMenu (File/Edit/View) readability fixes --- */
popover,
popover.background {
  background-color: #252526;
  color: #f2f2f2;
}

/* PopoverMenu uses modelbutton rows */
popover modelbutton {
  color: #f2f2f2;
  opacity: 1;
}

popover modelbutton label,
popover modelbutton accelerator,
popover label {
  color: #f2f2f2;
  opacity: 1;
}

popover modelbutton:hover { background-color: #3a3a3a; }
popover modelbutton:active { background-color: #444444; }

popover separator { background-color: #3a3a3a; }

/* If GTK marks items as insensitive/disabled, they often become too dim on dark backgrounds. */
popover modelbutton:disabled,
popover modelbutton:disabled label,
popover modelbutton:disabled accelerator,
popover modelbutton:disabled image,
popover modelbutton:insensitive,
popover modelbutton:insensitive label,
popover modelbutton:insensitive accelerator,
popover label.dim-label {
  color: #cfcfcf;
  opacity: 1;
}

/* If GTK uses :backdrop or other dimming, keep it readable */
popover:backdrop modelbutton,
popover:backdrop modelbutton label {
  opacity: 1;
  color: #cfcfcf;
}

/* --- File chooser (Open/Save dialog) list readability fixes (GTK4) --- */
filechooser,
filechooser dialog,
filechooser .background {
  background-color: #252526;
  color: #f2f2f2;
}

filechooser columnview,
filechooser listview,
filechooser columnview row,
filechooser listview row,
filechooser columnview row label,
filechooser listview row label,
filechooser columnview label,
filechooser listview label {
  color: #f2f2f2;
  opacity: 1;
}

filechooser placessidebar,
filechooser placessidebar label {
  color: #f2f2f2;
  opacity: 1;
}

filechooser columnview row:hover,
filechooser listview row:hover {
  background-color: #333333;
}

filechooser columnview row:selected,
filechooser listview row:selected,
filechooser columnview row:selected label,
filechooser listview row:selected label {
  background-color: #2b5b77;
  color: #ffffff;
  opacity: 1;
}
"#;

const CSS_LIGHT: &str = r#"
/* Base */
window, .background {
  background-color: #f6f6f6;
  color: #1a1a1a;
}

label { color: #1a1a1a; }
separator { background: #d0d0d0; }

/* Menubar row */
.menubar {
  background-color: #eeeeee;
  border-bottom: 1px solid #d0d0d0;
  padding: 4px;
}

/* Menu buttons */
.menubutton { margin: 0 2px; }

.menubutton button,
.menubutton > button {
  background: transparent;
  color: #1a1a1a;
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 4px 10px;
}

.menubutton button:hover,
.menubutton > button:hover {
  background: #dddddd;
  border-color: #c8c8c8;
}

/* Generic buttons */
button {
  background-color: #ffffff;
  color: #1a1a1a;
  border: 1px solid #c8c8c8;
  border-radius: 6px;
  padding: 6px 10px;
}

button:hover { background-color: #f0f0f0; }
button:active { background-color: #e6e6e6; }

/* Right-side icon button (settings) */
.right-button {
  margin-right: 6px;
  padding: 4px;
}

/* Inputs */
entry, spinbutton, combobox, textview {
  background-color: #ffffff;
  color: #1a1a1a;
  border: 1px solid #c8c8c8;
  border-radius: 6px;
}

entry:focus, spinbutton:focus, combobox:focus {
  border-color: #3b82f6;
  box-shadow: none;
}

/* Notebook + tabs */
notebook { background-color: #f6f6f6; }

notebook > header {
  background-color: #eeeeee;
  border-bottom: 1px solid #d0d0d0;
}

notebook > header tab {
  background-color: #ffffff;
  border: 1px solid #c8c8c8;
  border-bottom: none;
  border-radius: 6px 6px 0 0;
  margin: 2px 2px 0 2px;
  padding: 4px 8px;
}

notebook > header tab:hover { background-color: #f0f0f0; }

notebook > header tab:checked {
  background-color: #f6f6f6;
  border-color: #3b82f6;
}

/* Status bar */
.status {
  background-color: #eeeeee;
  border-top: 1px solid #d0d0d0;
  padding: 6px;
}

/* Gutter + editor view */
.gutter {
  background-color: #eeeeee;
  border-right: 1px solid #d0d0d0;
}

.editor-view {
  background-color: #ffffff;
  color: #1a1a1a;
}

/* Popovers / dialogs */
popover, dialog {
  background-color: #ffffff;
  color: #1a1a1a;
  border: 1px solid #c8c8c8;
  border-radius: 8px;
}
"#;

// One provider reused across theme changes.
thread_local! {
    static CSS_PROVIDER: RefCell<Option<CssProvider>> = RefCell::new(None);
}

pub fn load_css(mode: crate::config::ThemeMode) {
    let css = match mode {
        crate::config::ThemeMode::Dark => CSS_DARK,
        crate::config::ThemeMode::Light => CSS_LIGHT,
    };

    if let Some(display) = gdk::Display::default() {
        CSS_PROVIDER.with(|cell| {
            let mut slot = cell.borrow_mut();

            // Register provider once.
            let provider = slot.get_or_insert_with(|| {
                let p = CssProvider::new();
                // Deprecated warning is expected on newer GTK bindings; OK for gtk4 0.6.x.
                StyleContext::add_provider_for_display(
                    &display,
                    &p,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                p
            });

            // gtk4 0.6 expects &str here
            provider.load_from_data(css);
        });
    }
}

fn main() {
    let app = Application::builder()
        .application_id("io.github.amfuser.fikby_ide")
        .build();

    app.connect_activate(|app| {
        // Must match the initial mode used in ui/mod.rs at startup.
        load_css(crate::config::ThemeMode::Dark);
        crate::ui::build_ui(app);
    });

    app.run();
}
