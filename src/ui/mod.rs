use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label,
    MenuButton, Notebook, Orientation, Paned, PolicyType, PopoverMenu, ScrolledWindow,
};
use gtk4::gio::SimpleAction;
use std::cell::RefCell;
use std::rc::Rc;

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::config::ThemeMode;
use crate::editor::Editor;
use crate::find_replace::FindReplaceDialog;

pub fn build_ui(app: &Application) {
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();
    
    // Start with dark theme by default
    let current_theme_mode = Rc::new(RefCell::new(ThemeMode::Dark));
    let theme = Rc::new(ts.themes["base16-ocean.dark"].clone());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Fikby IDE")
        .default_width(1000)
        .default_height(700)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 0);

    // Custom menu bar
    let menubar = GtkBox::new(Orientation::Horizontal, 0);
    menubar.style_context().add_class("menubar");

    let file_menu = create_file_menu();
    let edit_menu = create_edit_menu();
    
    // Pass the theme mode to view menu for theme toggle
    let view_menu = create_view_menu();

    menubar.append(&file_menu);
    menubar.append(&edit_menu);
    menubar.append(&view_menu);

    // Right-aligned buttons
    let right_box = GtkBox::new(Orientation::Horizontal, 0);
    right_box.set_hexpand(true);
    right_box.set_halign(gtk4::Align::End);

    let settings_btn = Button::new();
    settings_btn.set_icon_name("emblem-system-symbolic");
    settings_btn.style_context().add_class("right-button");
    right_box.append(&settings_btn);

    menubar.append(&right_box);

    vbox.append(&menubar);

    // Main paned (sidebar + editor area)
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);

    // Sidebar placeholder
    let sidebar = GtkBox::new(Orientation::Vertical, 0);
    sidebar.set_width_request(200);
    let sidebar_label = Label::new(Some("File Explorer\n(Coming Soon)"));
    sidebar.append(&sidebar_label);

    let sidebar_scrolled = ScrolledWindow::builder()
        .child(&sidebar)
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .build();

    paned.set_start_child(Some(&sidebar_scrolled));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);

    // Editor area (Notebook for tabs)
    let notebook = Notebook::new();
    notebook.set_scrollable(true);
    notebook.set_vexpand(true);
    notebook.set_hexpand(true);

    // Status bar
    let status_bar = GtkBox::new(Orientation::Horizontal, 10);
    status_bar.style_context().add_class("status");

    let status_label = Label::new(Some("Ln 1, Col 1"));
    let status_info_label = Label::new(Some("Ready"));
    status_info_label.set_hexpand(true);
    status_info_label.set_halign(gtk4::Align::Start);

    status_bar.append(&status_label);
    status_bar.append(&status_info_label);

    paned.set_end_child(Some(&notebook));
    vbox.append(&paned);
    vbox.append(&status_bar);

    window.set_child(Some(&vbox));

    // Store references in Rc<RefCell<>> for sharing
    let editors: Rc<RefCell<Vec<Rc<Editor>>>> = Rc::new(RefCell::new(Vec::new()));
    let current_editor: Rc<RefCell<Option<Rc<Editor>>>> = Rc::new(RefCell::new(None));

    // TOGGLE THEME ACTION
    {
        let action = SimpleAction::new("toggle-theme", None);
        let theme_mode_clone = current_theme_mode.clone();

        action.connect_activate(move |_, _| {
            let mut mode = theme_mode_clone.borrow_mut();
            *mode = if *mode == ThemeMode::Light {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            
            // Apply new theme
            crate::load_css(*mode);
        });

        app.add_action(&action);
    }

    // NEW FILE ACTION
    {
        let action = SimpleAction::new("new", None);
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let current_editor_clone = current_editor.clone();
        let ss_clone = ss.clone();
        let theme_clone = theme.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();

        action.connect_activate(move |_, _| {
            let editor = Editor::new("Untitled", None, None, ss_clone.clone(), theme_clone.clone());
            
            let page_index = notebook_clone.append_page(
                &editor.content_row(),
                Some(&editor.header),
            );
            
            notebook_clone.set_current_page(Some(page_index));
            
            editor.update(&status_label_clone, &status_info_label_clone);
            
            editors_clone.borrow_mut().push(editor.clone());
            *current_editor_clone.borrow_mut() = Some(editor.clone());

            // Connect close button
            let notebook_clone2 = notebook_clone.clone();
            let editors_clone2 = editors_clone.clone();
            let editor_clone = editor.clone();
            editor.close_button.connect_clicked(move |_| {
                if let Some(page_num) = notebook_clone2.page_num(&editor_clone.content_row()) {
                    notebook_clone2.remove_page(Some(page_num));
                    editors_clone2.borrow_mut().retain(|e| !Rc::ptr_eq(e, &editor_clone));
                }
            });
        });

        app.add_action(&action);
    }

    // OPEN FILE ACTION
    {
        let action = SimpleAction::new("open", None);
        let window_clone = window.clone();
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let current_editor_clone = current_editor.clone();
        let ss_clone = ss.clone();
        let theme_clone = theme.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();

        action.connect_activate(move |_, _| {
            let dialog = gtk4::FileChooserDialog::new(
                Some("Open File"),
                Some(&window_clone),
                gtk4::FileChooserAction::Open,
                &[("Cancel", gtk4::ResponseType::Cancel), ("Open", gtk4::ResponseType::Accept)],
            );

            let notebook_clone2 = notebook_clone.clone();
            let editors_clone2 = editors_clone.clone();
            let current_editor_clone2 = current_editor_clone.clone();
            let ss_clone2 = ss_clone.clone();
            let theme_clone2 = theme_clone.clone();
            let status_label_clone2 = status_label_clone.clone();
            let status_info_label_clone2 = status_info_label_clone.clone();

            dialog.connect_response(move |dialog, response| {
                if response == gtk4::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let editor = Editor::new(
                                    "File",
                                    Some(content),
                                    Some(path.clone()),
                                    ss_clone2.clone(),
                                    theme_clone2.clone(),
                                );

                                let page_index = notebook_clone2.append_page(
                                    &editor.content_row(),
                                    Some(&editor.header),
                                );

                                notebook_clone2.set_current_page(Some(page_index));
                                editor.update(&status_label_clone2, &status_info_label_clone2);
                                
                                editors_clone2.borrow_mut().push(editor.clone());
                                *current_editor_clone2.borrow_mut() = Some(editor.clone());

                                // Connect close button
                                let notebook_clone3 = notebook_clone2.clone();
                                let editors_clone3 = editors_clone2.clone();
                                let editor_clone = editor.clone();
                                editor.close_button.connect_clicked(move |_| {
                                    if let Some(page_num) = notebook_clone3.page_num(&editor_clone.content_row()) {
                                        notebook_clone3.remove_page(Some(page_num));
                                        editors_clone3.borrow_mut().retain(|e| !Rc::ptr_eq(e, &editor_clone));
                                    }
                                });
                            }
                        }
                    }
                }
                dialog.close();
            });

            dialog.show();
        });

        app.add_action(&action);
    }

    // SAVE ACTION
    {
        let action = SimpleAction::new("save", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                let current_file = editor.current_file.borrow().clone();

                if let Some(path) = current_file {
                    // Save to existing file
                    let _ = editor.save_to_path(&path);
                } else {
                    // Show save dialog
                    let dialog = gtk4::FileChooserDialog::new(
                        Some("Save File"),
                        Some(&window_clone),
                        gtk4::FileChooserAction::Save,
                        &[("Cancel", gtk4::ResponseType::Cancel), ("Save", gtk4::ResponseType::Accept)],
                    );

                    let editor_clone = editor.clone();
                    dialog.connect_response(move |dialog, response| {
                        if response == gtk4::ResponseType::Accept {
                            if let Some(file) = dialog.file() {
                                if let Some(path) = file.path() {
                                    let _ = editor_clone.save_to_path(&path);
                                }
                            }
                        }
                        dialog.close();
                    });

                    dialog.show();
                }
            }
        });

        app.add_action(&action);
    }

    // SAVE AS ACTION
    {
        let action = SimpleAction::new("save-as", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                let dialog = gtk4::FileChooserDialog::new(
                    Some("Save File As"),
                    Some(&window_clone),
                    gtk4::FileChooserAction::Save,
                    &[("Cancel", gtk4::ResponseType::Cancel), ("Save", gtk4::ResponseType::Accept)],
                );

                let editor_clone = editor.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == gtk4::ResponseType::Accept {
                        if let Some(file) = dialog.file() {
                            if let Some(path) = file.path() {
                                let _ = editor_clone.save_to_path(&path);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        app.add_action(&action);
    }

    // QUIT ACTION
    {
        let action = SimpleAction::new("quit", None);
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            window_clone.close();
        });

        app.add_action(&action);
    }

    // UNDO ACTION
    {
        let action = SimpleAction::new("undo", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.undo();
            }
        });

        app.add_action(&action);
    }

    // REDO ACTION
    {
        let action = SimpleAction::new("redo", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.redo();
            }
        });

        app.add_action(&action);
    }

    // CUT ACTION
    {
        let action = SimpleAction::new("cut", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.cut();
            }
        });

        app.add_action(&action);
    }

    // COPY ACTION
    {
        let action = SimpleAction::new("copy", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                if let Some(display) = gtk4::gdk::Display::default() {
                    let clipboard = display.clipboard();
                    editor.main_buffer.copy_clipboard(&clipboard);
                }
            }
        });

        app.add_action(&action);
    }

    // PASTE ACTION
    {
        let action = SimpleAction::new("paste", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.paste();
            }
        });

        app.add_action(&action);
    }

    // FIND ACTION
    {
        let action = SimpleAction::new("find", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                // Upcast ApplicationWindow to Window
                let window_ref: &gtk4::Window = window_clone.upcast_ref();
                let find_dialog = FindReplaceDialog::new(window_ref, editor.clone());
                
                // If there's selected text, use it as the initial find text
                if let Some((start, end)) = editor.main_buffer.selection_bounds() {
                    let selected = editor.main_buffer.text(&start, &end, false);
                    if !selected.is_empty() && !selected.contains('\n') {
                        find_dialog.set_find_text(&selected);
                    }
                }
                
                find_dialog.show();
            }
        });

        app.add_action(&action);
    }

    // REPLACE ACTION (same as find, but with replace tab focused)
    {
        let action = SimpleAction::new("replace", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                // Upcast ApplicationWindow to Window
                let window_ref: &gtk4::Window = window_clone.upcast_ref();
                let find_dialog = FindReplaceDialog::new(window_ref, editor.clone());
                
                // If there's selected text, use it as the initial find text
                if let Some((start, end)) = editor.main_buffer.selection_bounds() {
                    let selected = editor.main_buffer.text(&start, &end, false);
                    if !selected.is_empty() && !selected.contains('\n') {
                        find_dialog.set_find_text(&selected);
                    }
                }
                
                find_dialog.show();
            }
        });

        app.add_action(&action);
    }

    // TOGGLE WRAP ACTION
    {
        let action = SimpleAction::new("toggle-wrap", None);
        let current_editor_clone = current_editor.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.toggle_wrap();
            }
        });

        app.add_action(&action);
    }

    // Update current editor when switching tabs
    {
        let current_editor_clone = current_editor.clone();
        let editors_clone = editors.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();

        notebook.connect_switch_page(move |_notebook, page, _page_num| {
            let editors = editors_clone.borrow();
            for editor in editors.iter() {
                if editor.content_row().upcast_ref::<gtk4::Widget>() == page {
                    *current_editor_clone.borrow_mut() = Some(editor.clone());
                    editor.update(&status_label_clone, &status_info_label_clone);
                    break;
                }
            }
        });
    }

    // Set up keyboard shortcuts
    app.set_accels_for_action("app.new", &["<Ctrl>N"]);
    app.set_accels_for_action("app.open", &["<Ctrl>O"]);
    app.set_accels_for_action("app.save", &["<Ctrl>S"]);
    app.set_accels_for_action("app.save-as", &["<Ctrl><Shift>S"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    app.set_accels_for_action("app.undo", &["<Ctrl>Z"]);
    app.set_accels_for_action("app.redo", &["<Ctrl><Shift>Z"]);
    app.set_accels_for_action("app.cut", &["<Ctrl>X"]);
    app.set_accels_for_action("app.copy", &["<Ctrl>C"]);
    app.set_accels_for_action("app.paste", &["<Ctrl>V"]);
    app.set_accels_for_action("app.find", &["<Ctrl>F"]);
    app.set_accels_for_action("app.replace", &["<Ctrl>H"]);

    // Create initial empty tab
    let initial_editor = Editor::new("Untitled", None, None, ss.clone(), theme.clone());
    notebook.append_page(&initial_editor.content_row(), Some(&initial_editor.header));
    initial_editor.update(&status_label, &status_info_label);
    
    editors.borrow_mut().push(initial_editor.clone());
    *current_editor.borrow_mut() = Some(initial_editor.clone());

    // Connect close button for initial tab
    {
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let editor_clone = initial_editor.clone();
        initial_editor.close_button.connect_clicked(move |_| {
            if let Some(page_num) = notebook_clone.page_num(&editor_clone.content_row()) {
                notebook_clone.remove_page(Some(page_num));
                editors_clone.borrow_mut().retain(|e| !Rc::ptr_eq(e, &editor_clone));
            }
        });
    }

    window.present();
}

fn create_file_menu() -> MenuButton {
    let menu_button = MenuButton::new();
    menu_button.set_label("File");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("New"), Some("app.new"));
    menu.append(Some("Open"), Some("app.open"));
    menu.append(Some("Save"), Some("app.save"));
    menu.append(Some("Save As"), Some("app.save-as"));
    menu.append(Some("Quit"), Some("app.quit"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}

fn create_edit_menu() -> MenuButton {
    let menu_button = MenuButton::new();
    menu_button.set_label("Edit");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("Undo"), Some("app.undo"));
    menu.append(Some("Redo"), Some("app.redo"));
    menu.append(Some("Cut"), Some("app.cut"));
    menu.append(Some("Copy"), Some("app.copy"));
    menu.append(Some("Paste"), Some("app.paste"));
    menu.append(Some("Find"), Some("app.find"));
    menu.append(Some("Replace"), Some("app.replace"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}

fn create_view_menu() -> MenuButton {
    let menu_button = MenuButton::new();
    menu_button.set_label("View");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("Toggle Word Wrap"), Some("app.toggle-wrap"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}