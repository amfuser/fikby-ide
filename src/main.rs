use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, AboutDialog, Box as GtkBox, Button, CssProvider, FileChooserAction,
    FileChooserNative, Label, MenuButton, Orientation, ResponseType, ScrolledWindow, TextView,
    WrapMode, MessageDialog, ButtonsType, MessageType,
};
use gtk4::{gdk, gio, glib, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "org.gtk_rs.Fikby";

const CSS: &str = r#"
.menubar {
    background: #f5f5f5;
    padding: 4px;
}
.menubutton {
    font-weight: 600;
    padding: 6px 10px;
    border-radius: 4px;
}
.menubutton:hover {
    background: #e8e8e8;
}
.right-button {
    padding: 4px 8px;
    margin-right: 6px;
}
"#;

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    // Load CSS
    let provider = CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // --- File menu model ---
    let file_menu = gio::Menu::new();

    file_menu.append_item(&gio::MenuItem::new(Some("New"), Some("app.new")));
    // example recent submenu
    let recent_submenu = gio::Menu::new();
    recent_submenu.append(Some("Recent 1"), Some("app.open_recent_1"));
    recent_submenu.append(Some("Recent 2"), Some("app.open_recent_2"));
    let mi_open_recent = gio::MenuItem::new(Some("Open Recent"), None);
    mi_open_recent.set_submenu(Some(&recent_submenu));
    file_menu.append_item(&mi_open_recent);
    file_menu.append_item(&gio::MenuItem::new(Some("Open..."), Some("app.open")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save"), Some("app.save")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save As..."), Some("app.saveas")));
    // quit section
    let section = gio::Menu::new();
    section.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));
    file_menu.append_section(None, &section);

    // --- Single menubar box (contains menu labels + right-side controls) ---
    let menubar = GtkBox::new(Orientation::Horizontal, 0);
    menubar.style_context().add_class("menubar");

    // File MenuButton — we'll keep a handle to it so an action can pop it for Alt+F
    let file_menu_button = MenuButton::builder()
        .label("File")
        .menu_model(&file_menu)
        .margin_start(6)
        .margin_end(6)
        .build();
    file_menu_button.style_context().add_class("menubutton");
    menubar.append(&file_menu_button);

    // Other top-level labels
    let edit_button = MenuButton::builder().label("Edit").build();
    edit_button.style_context().add_class("menubutton");
    menubar.append(&edit_button);

    let view_button = MenuButton::builder().label("View").build();
    view_button.style_context().add_class("menubutton");
    menubar.append(&view_button);

    let help_button = MenuButton::builder().label("Help").build();
    help_button.style_context().add_class("menubutton");
    menubar.append(&help_button);

    // spacer + right-side buttons
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    menubar.append(&spacer);

    let about_button = Button::with_label("About");
    about_button.style_context().add_class("right-button");
    menubar.append(&about_button);

    let quit_button = Button::with_label("Quit");
    quit_button.style_context().add_class("right-button");
    menubar.append(&quit_button);

    // --- Text editor area ---
    let text_view = TextView::new();
    text_view.set_wrap_mode(WrapMode::None);
    let text_buffer = text_view.buffer();
    let scrolled = ScrolledWindow::builder()
        .child(&text_view)
        .min_content_height(400)
        .build();

    // application state
    let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let dirty: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    // mark buffer changed => dirty
    {
        let dirty = dirty.clone();
        text_buffer.connect_changed(move |_| {
            *dirty.borrow_mut() = true;
        });
    }

    // layout
    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.append(&menubar);
    vbox.append(&scrolled);

    // Builder title expects a string-like value (not Option)
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Fikby")
        .default_width(900)
        .default_height(600)
        .child(&vbox)
        .build();

    // --- Actions ---

    // 1) New
    {
        let buffer = text_buffer.clone();
        let current_file = current_file.clone();
        let window = window.clone();
        let dirty = dirty.clone();
        let new_act = gio::SimpleAction::new("new", None);
        new_act.connect_activate(move |_, _| {
            buffer.set_text("");
            *current_file.borrow_mut() = None;
            *dirty.borrow_mut() = false;
            window.set_title(Some("Fikby"));
        });
        app.add_action(&new_act);
        app.set_accels_for_action("app.new", &["<Ctrl>N"]);
    }

    // Helper to load file contents into buffer
    let load_file_into_buffer = {
        let buffer = text_buffer.clone();
        let current_file = current_file.clone();
        let window = window.clone();
        let dirty = dirty.clone();
        move |path: PathBuf| {
            match fs::read_to_string(&path) {
                Ok(text) => {
                    buffer.set_text(&text);
                    *current_file.borrow_mut() = Some(path.clone());
                    *dirty.borrow_mut() = false;
                    window.set_title(Some(&format!("Fikby - {}", path.display())));
                }
                Err(err) => {
                    eprintln!("Failed to read file: {}", err);
                }
            }
        }
    };

    // 2) Open (non-blocking FileChooserNative)
    {
        let load = load_file_into_buffer.clone();
        let window = window.clone();
        let open_act = gio::SimpleAction::new("open", None);
        open_act.connect_activate(move |_, _| {
            let dlg = FileChooserNative::new(
                Some("Open File"),
                Some(&window),
                FileChooserAction::Open,
                Some("Open"),
                Some("Cancel"),
            );
            //let window_cloned = window.clone();
            //dlg.connect_response(move |dlg, resp| {
            //    if resp == ResponseType::Accept {
            //        if let Some(file) = dlg.file() {
            //            if let Some(path) = file.path() {
            //                load(path);
            //            }
            //        }
            //    }
            //    let _ = dlg;
            //    drop(window_cloned);
            //});
            // use non-blocking show()
            //dlg.show();
        });
        app.add_action(&open_act);
        app.set_accels_for_action("app.open", &["<Ctrl>O"]);
    }

    // 3) Save (synchronous write; if no file => forward to Save As dialog)
    {
        let buffer = text_buffer.clone();
        let current_file = current_file.clone();
        let window = window.clone();
        let dirty = dirty.clone();
        let save_act = gio::SimpleAction::new("save", None);
        save_act.connect_activate(move |_, _| {
            // extract whole text (include_hidden_chars=false)
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            let text = buffer.text(&start, &end, false);

            if let Some(path) = &*current_file.borrow() {
                if let Err(err) = fs::write(path, text.as_str()) {
                    eprintln!("Failed to save file {}: {}", path.display(), err);
                } else {
                    *dirty.borrow_mut() = false;
                    window.set_title(Some(&format!("Fikby - {}", path.display())));
                }
            } else {
                // open Save As dialog non-blocking and write file on accept
                let dlg = FileChooserNative::new(
                    Some("Save File"),
                    Some(&window),
                    FileChooserAction::Save,
                    Some("Save"),
                    Some("Cancel"),
                );
                let buffer = buffer.clone();
                let current_file = current_file.clone();
                let window2 = window.clone();
                let dirty2 = dirty.clone();
                dlg.connect_response(move |dlg, resp| {
                    if resp == ResponseType::Accept {
                        if let Some(file) = dlg.file() {
                            if let Some(path) = file.path() {
                                let start = buffer.start_iter();
                                let end = buffer.end_iter();
                                let text = buffer.text(&start, &end, false);
                                if let Err(err) = fs::write(&path, text.as_str()) {
                                    eprintln!("Failed to save file {}: {}", path.display(), err);
                                } else {
                                    *current_file.borrow_mut() = Some(path.clone());
                                    *dirty2.borrow_mut() = false;
                                    window2.set_title(Some(&format!("Fikby - {}", path.display())));
                                }
                            }
                        }
                    }
                    let _ = dlg;
                });
                dlg.show();
            }
        });
        app.add_action(&save_act);
        app.set_accels_for_action("app.save", &["<Ctrl>S"]);
    }

    // 4) Save As
    {
        let buffer = text_buffer.clone();
        let current_file = current_file.clone();
        let window = window.clone();
        let dirty = dirty.clone();
        let saveas_act = gio::SimpleAction::new("saveas", None);
        saveas_act.connect_activate(move |_, _| {
            let dlg = FileChooserNative::new(
                Some("Save File As..."),
                Some(&window),
                FileChooserAction::Save,
                Some("Save"),
                Some("Cancel"),
            );
            let buffer = buffer.clone();
            let current_file = current_file.clone();
            let window2 = window.clone();
            let dirty2 = dirty.clone();
            dlg.connect_response(move |dlg, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dlg.file() {
                        if let Some(path) = file.path() {
                            let start = buffer.start_iter();
                            let end = buffer.end_iter();
                            let text = buffer.text(&start, &end, false);
                            if let Err(err) = fs::write(&path, text.as_str()) {
                                eprintln!("Failed to save file {}: {}", path.display(), err);
                            } else {
                                *current_file.borrow_mut() = Some(path.clone());
                                *dirty2.borrow_mut() = false;
                                window2.set_title(Some(&format!("Fikby - {}", path.display())));
                            }
                        }
                    }
                }
                let _ = dlg;
            });
            dlg.show();
        });
        app.add_action(&saveas_act);
        // bind Shift+Ctrl+S as accelerator (gtk uses <Ctrl><Shift>S)
        app.set_accels_for_action("app.saveas", &["<Ctrl><Shift>S"]);
    }

    // Open recent example actions
    {
        let or1 = gio::SimpleAction::new("open_recent_1", None);
        or1.connect_activate(|_, _| {
            println!("Open Recent -> Recent 1");
        });
        app.add_action(&or1);

        let or2 = gio::SimpleAction::new("open_recent_2", None);
        or2.connect_activate(|_, _| {
            println!("Open Recent -> Recent 2");
        });
        app.add_action(&or2);
    }

    // AboutDialog action (F1)
    {
        let window_for_about = window.clone();
        let about_act = gio::SimpleAction::new("about", None);
        about_act.connect_activate(move |_, _| {
            let about = AboutDialog::new();
            about.set_transient_for(Some(&window_for_about));
            about.set_program_name(Some("Fikby"));
            about.set_version(Some("0.1"));
            about.set_comments(Some("A small text editor example"));
            about.set_authors(&["Your Name"]);
            about.present();
        });
        app.add_action(&about_act);
        app.set_accels_for_action("app.about", &["<F1>"]);
    }

    // Quit action (checks dirty flag with confirmation dialog)
    {
        let window = window.clone();
        let current_file = current_file.clone();
        let dirty = dirty.clone();
        let quit_act = gio::SimpleAction::new("quit", None);
        quit_act.connect_activate(move |_, _| {
            // if dirty, prompt Save / Don't Save / Cancel
            if *dirty.borrow() {
                let dlg = MessageDialog::builder()
                    .transient_for(&window)
                    .modal(true)
                    .message_type(MessageType::Question)
                    .buttons(ButtonsType::None)
                    .text("You have unsaved changes. Save before quitting?")
                    .build();
                // add buttons manually: Save, Don't Save, Cancel
                dlg.add_buttons(&[
                    ("Save", ResponseType::Yes),
                    ("Don't Save", ResponseType::No),
                    ("Cancel", ResponseType::Cancel),
                ]);
                let window_clone = window.clone();
                let current_file = current_file.clone();
                let dirty_clone = dirty.clone();
                dlg.connect_response(move |dlg, resp| {
                    match resp {
                        ResponseType::Yes => {
                            // activate save action and then quit the application
                            if let Some(app) = window_clone.application() {
                                if let Some(action) = app.lookup_action("save") {
                                    action.activate(None);
                                }
                                app.quit();
                            }
                        }
                        ResponseType::No => {
                            // discard changes and quit
                            if let Some(app) = window_clone.application() {
                                app.quit();
                            }
                        }
                        _ => {
                            // Cancel -> do nothing
                            dlg.close();
                        }
                    }
                    let _ = dlg;
                    let _ = window_clone;
                    let _ = current_file;
                    let _ = dirty_clone;
                });
                dlg.present();
            } else {
                if let Some(app) = window.application() {
                    app.quit();
                }
            }
        });
        app.add_action(&quit_act);
        app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    }

    // Wire About / Quit right-side buttons to app actions
    {
        let app_clone = app.clone();
        about_button.connect_clicked(move |_| {
            if let Some(action) = app_clone.lookup_action("about") {
                action.activate(None);
            }
        });
    }
    {
        let app_clone = app.clone();
        quit_button.connect_clicked(move |_| {
            if let Some(action) = app_clone.lookup_action("quit") {
                action.activate(None);
            }
        });
    }

    // Add action to pop the File menu when pressing Alt+F (gives 'mnemonic-like' behavior)
    {
        let file_button = file_menu_button.clone();
        let filemenu_act = gio::SimpleAction::new("filemenu", None);
        filemenu_act.connect_activate(move |_, _| {
            // popup the file menu (MenuButton provides popup)
            file_button.popup();
        });
        app.add_action(&filemenu_act);
        app.set_accels_for_action("app.filemenu", &["<Alt>F"]);
    }

    // Finally present the window
    window.present();
}