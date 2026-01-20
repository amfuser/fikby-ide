use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, FileChooserAction,
    FileChooserNative, Label, MenuButton, Notebook, Orientation, ResponseType,
};
use gtk4::{gdk, gio, glib, STYLE_PROVIDER_PRIORITY_APPLICATION};
use glib::clone;

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use crate::config;
use crate::editor::Editor;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;

pub fn build_ui(app: &Application) {
    // Load CSS
    let provider = CssProvider::new();
    provider.load_from_data(config::CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // --- File menu ---
    let file_menu = gio::Menu::new();
    file_menu.append_item(&gio::MenuItem::new(Some("New"), Some("app.new")));
    file_menu.append_item(&gio::MenuItem::new(Some("Open..."), Some("app.open")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save"), Some("app.save")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save As..."), Some("app.saveas")));
    let section = gio::Menu::new();
    section.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));
    file_menu.append_section(None, &section);

    // Menubar
    let menubar = GtkBox::new(Orientation::Horizontal, 0);
    menubar.style_context().add_class("menubar");
    let file_menu_button = MenuButton::builder()
        .label("File")
        .menu_model(&file_menu)
        .margin_start(6)
        .margin_end(6)
        .build();
    file_menu_button.style_context().add_class("menubutton");
    menubar.append(&file_menu_button);

    let edit_button = MenuButton::builder().label("Edit").build();
    edit_button.style_context().add_class("menubutton");
    menubar.append(&edit_button);

    let view_button = MenuButton::builder().label("View").build();
    view_button.style_context().add_class("menubutton");
    menubar.append(&view_button);

    let help_button = MenuButton::builder().label("Help").build();
    help_button.style_context().add_class("menubutton");
    menubar.append(&help_button);

    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    menubar.append(&spacer);

    let about_button = Button::with_label("About");
    about_button.style_context().add_class("right-button");
    menubar.append(&about_button);

    let quit_button = Button::with_label("Quit");
    quit_button.style_context().add_class("right-button");
    menubar.append(&quit_button);

    // Notebook for tabs
    let notebook = Notebook::new();
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);

    // syntect resources
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();
    let theme = Rc::new(
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().unwrap().clone()),
    );

    // tabs vector
    let tabs: Rc<RefCell<Vec<Option<Rc<Editor>>>>> = Rc::new(RefCell::new(Vec::new()));

    // status bar
    let status_box = GtkBox::new(Orientation::Horizontal, 6);
    status_box.style_context().add_class("status");
    let status_left = Label::new(Some("Ln 1, Col 1"));
    status_left.set_halign(Align::Start);
    status_box.append(&status_left);
    let status_spacer = Label::new(None);
    status_spacer.set_hexpand(true);
    status_box.append(&status_spacer);

    // add_tab function uses Editor::new
    let add_tab = {
        let notebook = notebook.clone();
        let tabs = tabs.clone();
        let ss = ss.clone();
        let theme = theme.clone();
        move |title: &str, initial_text: Option<String>, path: Option<PathBuf>, status: &Label| {
            let editor = Editor::new(title, initial_text, path, ss.clone(), theme.clone());
            // schedule initial update
            editor.update(status);

            // append page
            let content_row = editor.content_row();
            let header = editor.header();
            let page = notebook.append_page(&content_row, Some(&header));

            // store editor
            {
                let mut v = tabs.borrow_mut();
                let idx = page as usize;
                if v.len() <= idx {
                    v.resize_with(idx + 1, || None);
                }
                v[idx] = Some(editor.clone());
            }

            // wire the close button that lives inside the editor header
            {
                let close_btn = editor.close_button.clone();
                let notebook_cl = notebook.clone();
                let tabs_cl = tabs.clone();
                let content_row_clone = content_row.clone();
                let editor_clone = editor.clone();

                close_btn.connect_clicked(move |_| {
                    // If not dirty, close immediately
                    if !*editor_clone.dirty.borrow() {
                        if let Some(idx) = notebook_cl.page_num(&content_row_clone) {
                            notebook_cl.remove_page(Some(idx));
                            let p_usize: usize = idx as usize;
                            if let Some(mut t) = tabs_cl.try_borrow_mut().ok() {
                                if p_usize < t.len() {
                                    t[p_usize] = None;
                                }
                            }
                        }
                        return;
                    }

                    // Dirty: prompt user using a Dialog with three buttons
                    let title = editor_clone
                        .current_file
                        .borrow()
                        .as_ref()
                        .and_then(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| "Untitled".to_string());

                    let dlg = Dialog::builder()
                        .modal(true)
                        .title("Save changes")
                        .build();

                    // message in content area
                    let content_area = dlg.content_area();
                    let label = Label::new(Some(&format!("Save changes to \"{}\"?", title)));
                    content_area.append(&label);

                    // add buttons: Save (Yes), Don't Save (No), Cancel
                    dlg.add_button("Save", ResponseType::Yes);
                    dlg.add_button("Don't Save", ResponseType::No);
                    dlg.add_button("Cancel", ResponseType::Cancel);

                    dlg.connect_response(clone!(@strong notebook_cl, @strong tabs_cl, @strong content_row_clone, @strong editor_clone => move |dialog, resp| {
                        match resp {
                            ResponseType::Yes => {
                                // Save, then close if successful
                                if let Some(path) = &*editor_clone.current_file.borrow() {
                                    if let Err(err) = editor_clone.save_to_path(path) {
                                        eprintln!("Failed to save: {}", err);
                                        // keep open (user can retry)
                                    } else {
                                        // saved OK -> close
                                        if let Some(idx) = notebook_cl.page_num(&content_row_clone) {
                                            notebook_cl.remove_page(Some(idx));
                                            let p_usize: usize = idx as usize;
                                            if let Some(mut t) = tabs_cl.try_borrow_mut().ok() {
                                                if p_usize < t.len() {
                                                    t[p_usize] = None;
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // No path -> show Save As dialog
                                    let save_dlg = FileChooserNative::new(
                                        Some("Save File As..."),
                                        None::<&gtk4::Window>,
                                        FileChooserAction::Save,
                                        Some("Save"),
                                        Some("Cancel"),
                                    );
                                    let buffer = editor_clone.main_buffer.clone();
                                    let ed_clone2 = editor_clone.clone();
                                    let notebook_inner = notebook_cl.clone();
                                    let tabs_inner = tabs_cl.clone();
                                    let content_inner = content_row_clone.clone();
                                    save_dlg.connect_response(move |sd, sresp| {
                                        if sresp == ResponseType::Accept {
                                            if let Some(file) = sd.file() {
                                                if let Some(path) = file.path() {
                                                    let s = buffer.start_iter();
                                                    let e = buffer.end_iter();
                                                    let content = buffer.text(&s, &e, false);
                                                    if let Err(err) = fs::write(&path, content.as_str()) {
                                                        eprintln!("Failed to save file {}: {}", path.display(), err);
                                                    } else {
                                                        *ed_clone2.current_file.borrow_mut() = Some(path.clone());
                                                        *ed_clone2.dirty.borrow_mut() = false;
                                                        // close after successful save
                                                        if let Some(idx) = notebook_inner.page_num(&content_inner) {
                                                            notebook_inner.remove_page(Some(idx));
                                                            let p_usize: usize = idx as usize;
                                                            if let Some(mut t) = tabs_inner.try_borrow_mut().ok() {
                                                                if p_usize < t.len() {
                                                                    t[p_usize] = None;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        let _ = sd;
                                    });
                                    save_dlg.show();
                                }
                            }
                            ResponseType::No => {
                                // Close without saving
                                if let Some(idx) = notebook_cl.page_num(&content_row_clone) {
                                    notebook_cl.remove_page(Some(idx));
                                    let p_usize: usize = idx as usize;
                                    if let Some(mut t) = tabs_cl.try_borrow_mut().ok() {
                                        if p_usize < t.len() {
                                            t[p_usize] = None;
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Cancel or other: do nothing
                            }
                        }
                        dialog.close();
                    }));

                    dlg.show();
                });
            }

            // when tab becomes active, refresh its layout+status
            {
                let tabs_cl = tabs.clone();
                let status_cl = status.clone();
                notebook.connect_switch_page(move |nb, _, _| {
                    if let Some(page_num) = nb.current_page() {
                        let idx = page_num as usize;
                        if let Some(Some(ed)) = tabs_cl.borrow().get(idx) {
                            ed.update(&status_cl);
                        }
                    }
                });
            }

            page
        }
    };

    // create initial tab
    add_tab("Untitled", Some(String::new()), None, &status_left);

    // main layout
    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.append(&menubar);
    vbox.append(&notebook);
    vbox.append(&status_box);

    // build window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Fikby")
        .default_width(1000)
        .default_height(700)
        .child(&vbox)
        .build();

    // Actions wiring (New / Open / Save / SaveAs / About / Quit)
    // New
    {
        let add_tab = add_tab.clone();
        let notebook_cl = notebook.clone();
        let status_cl = status_left.clone();
        let new_act = gio::SimpleAction::new("new", None);
        new_act.connect_activate(move |_, _| {
            let page = add_tab("Untitled", Some(String::new()), None, &status_cl);
            notebook_cl.set_current_page(Some(page));
        });
        app.add_action(&new_act);
        app.set_accels_for_action("app.new", &["<Ctrl>N"]);
    }

    // Open
    {
        let add_tab = add_tab.clone();
        let notebook_cl = notebook.clone();
        let status_cl = status_left.clone();
        let open_act = gio::SimpleAction::new("open", None);
        let window_for_dialog = window.clone();
        open_act.connect_activate(move |_, _| {
            let dlg = FileChooserNative::new(
                Some("Open File"),
                Some(&window_for_dialog),
                FileChooserAction::Open,
                Some("Open"),
                Some("Cancel"),
            );
            let add_tab = add_tab.clone();
            let notebook = notebook_cl.clone();
            let status_for_response = status_cl.clone();
            dlg.connect_response(move |dlg, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dlg.file() {
                        if let Some(path) = file.path() {
                            match fs::read_to_string(&path) {
                                Ok(text) => {
                                    let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
                                    let page = (add_tab)(title, Some(text), Some(path.clone()), &status_for_response);
                                    notebook.set_current_page(Some(page));
                                }
                                Err(err) => {
                                    eprintln!("Failed to read file: {}", err);
                                }
                            }
                        }
                    }
                }
                let _ = dlg;
            });
            dlg.show();
        });
        app.add_action(&open_act);
        app.set_accels_for_action("app.open", &["<Ctrl>O"]);
    }

    // Save
    {
        let notebook_cl = notebook.clone();
        let tabs_cl = tabs.clone();
        let save_act = gio::SimpleAction::new("save", None);
        save_act.connect_activate(move |_, _| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                if let Some(Some(ed)) = tabs_cl.borrow().get(idx) {
                    if let Some(path) = &*ed.current_file.borrow() {
                        if let Err(err) = ed.save_to_path(path) {
                            eprintln!("Failed to save: {}", err);
                        }
                    }
                }
            }
        });
        app.add_action(&save_act);
        app.set_accels_for_action("app.save", &["<Ctrl>S"]);
    }

    // Save As
    {
        let notebook_cl = notebook.clone();
        let tabs_cl = tabs.clone();
        let window_clone = window.clone();
        let saveas_act = gio::SimpleAction::new("saveas", None);
        saveas_act.connect_activate(move |_, _| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                if let Some(Some(ed)) = tabs_cl.borrow().get(idx) {
                    let buffer = ed.main_buffer.clone();
                    let dlg = FileChooserNative::new(
                        Some("Save File As..."),
                        Some(&window_clone),
                        FileChooserAction::Save,
                        Some("Save"),
                        Some("Cancel"),
                    );
                    let buffer2 = buffer.clone();
                    let ed_clone = ed.clone();
                    dlg.connect_response(move |dlg, resp| {
                        if resp == ResponseType::Accept {
                            if let Some(file) = dlg.file() {
                                if let Some(path) = file.path() {
                                    let s = buffer2.start_iter();
                                    let e = buffer2.end_iter();
                                    let content = buffer2.text(&s, &e, false);
                                    if let Err(err) = fs::write(&path, content.as_str()) {
                                        eprintln!("Failed to save file {}: {}", path.display(), err);
                                    } else {
                                        *ed_clone.current_file.borrow_mut() = Some(path.clone());
                                        *ed_clone.dirty.borrow_mut() = false;
                                    }
                                }
                            }
                        }
                        let _ = dlg;
                    });
                    dlg.show();
                }
            }
        });
        app.add_action(&saveas_act);
        app.set_accels_for_action("app.saveas", &["<Ctrl><Shift>S"]);
    }

    // About
    {
        let window_for_about = window.clone();
        let about_act = gio::SimpleAction::new("about", None);
        about_act.connect_activate(move |_, _| {
            let about = gtk4::AboutDialog::new();
            about.set_transient_for(Some(&window_for_about));
            about.set_program_name(Some("Fikby"));
            about.set_version(Some("0.1"));
            about.present();
        });
        app.add_action(&about_act);
        app.set_accels_for_action("app.about", &["F1"]);
    }

    // Wire right-side buttons
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

    // Quit
    {
        let quit_act = gio::SimpleAction::new("quit", None);
        let appl = app.clone();
        quit_act.connect_activate(move |_, _| {
            appl.quit();
        });
        app.add_action(&quit_act);
        app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    }

    window.present();
}