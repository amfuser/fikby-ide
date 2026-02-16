use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label, Entry, Dialog, ResponseType,
    MenuButton, Notebook, Orientation, Paned, PopoverMenu,
    MessageDialog, MessageType, ButtonsType,
};
use gtk4::gio::SimpleAction;
use std::cell::RefCell;
use std::rc::Rc;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::config::ThemeMode;
use crate::editor::Editor;
use crate::file_explorer::FileExplorer;
use crate::find_replace::FindReplaceDialog;

pub fn build_ui(app: &Application) {
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();
    
    // Start with dark theme by default
    let current_theme_mode = Rc::new(RefCell::new(ThemeMode::Dark));
    let theme = Rc::new(ts.themes["base16-ocean.dark"].clone());
    let current_theme: Rc<RefCell<Rc<Theme>>> = Rc::new(RefCell::new(theme.clone()));

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

    // Create file explorer
    let file_explorer = FileExplorer::new();
    
    // Set current directory as root (or fallback to home)
    let root_dir = std::env::current_dir().unwrap_or_else(|_| {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
    });
    
    // We need to make it mutable, so we'll use Rc<RefCell<>>
    let file_explorer_rc = Rc::new(std::cell::RefCell::new(file_explorer));
    file_explorer_rc.borrow_mut().set_root_directory(root_dir);
    
    // Setup context menu (will be connected to actions later)
    file_explorer_rc.borrow().setup_context_menu(app);

    paned.set_start_child(Some(&file_explorer_rc.borrow().widget));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);

    // Editor area (Notebook for tabs)
    let notebook = Notebook::new();
    notebook.set_scrollable(true);
    notebook.set_vexpand(true);
    notebook.set_hexpand(true);
    // Set minimum height to prevent negative tab height calculations during window resize
    // This prevents GTK warning about negative min-height and window minimize issues
    notebook.set_size_request(-1, 100);

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

    // NEW FILE ACTION
    {
        let action = SimpleAction::new("new", None);
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let current_editor_clone = current_editor.clone();
        let ss_clone = ss.clone();
        let current_theme_clone = current_theme.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();

        action.connect_activate(move |_, _| {
            let theme_clone = current_theme_clone.borrow().clone();
            let editor = Editor::new("Untitled", None, None, ss_clone.clone(), theme_clone);
            
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
        let current_theme_clone = current_theme.clone();
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
            let current_theme_clone2 = current_theme_clone.clone();
            let status_label_clone2 = status_label_clone.clone();
            let status_info_label_clone2 = status_info_label_clone.clone();

            dialog.connect_response(move |dialog, response| {
                if response == gtk4::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let theme_clone = current_theme_clone2.borrow().clone();
                                let editor = Editor::new(
                                    "File",
                                    Some(content),
                                    Some(path.clone()),
                                    ss_clone2.clone(),
                                    theme_clone,
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

    // TOGGLE THEME ACTION
    {
        let action = SimpleAction::new("toggle-theme", None);
        let current_theme_mode_clone = current_theme_mode.clone();
        let current_theme_clone = current_theme.clone();
        let editors_clone = editors.clone();

        action.connect_activate(move |_, _| {
            // Toggle theme mode
            let new_mode = {
                let mut mode = current_theme_mode_clone.borrow_mut();
                *mode = match *mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                *mode
            };

            // Update CSS
            crate::load_css(new_mode);

            // Load new syntax highlighting theme
            let ts = ThemeSet::load_defaults();
            let theme_name = new_mode.syntax_theme_name();
            let new_theme = if let Some(theme) = ts.themes.get(theme_name) {
                Rc::new(theme.clone())
            } else {
                // Fallback to first available theme if the named theme doesn't exist
                eprintln!("Warning: Theme '{}' not found, using fallback", theme_name);
                Rc::new(ts.themes.values().next().unwrap().clone())
            };
            *current_theme_clone.borrow_mut() = new_theme.clone();

            // Update all open editors with the new theme
            for editor in editors_clone.borrow().iter() {
                editor.set_theme(new_theme.clone());
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
    app.set_accels_for_action("app.toggle-theme", &["<Ctrl>T"]);

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

    // Connect file explorer actions
    {
        // File activation (double-click or Enter)
        let file_explorer_clone = file_explorer_rc.clone();
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let current_editor_clone = current_editor.clone();
        let ss_clone = ss.clone();
        let current_theme_clone = current_theme.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();

        file_explorer_rc.borrow().connect_row_activated(move |path_buf, is_dir| {
            if !is_dir {
                // Open the file
                if let Ok(content) = std::fs::read_to_string(&path_buf) {
                    let theme_clone = current_theme_clone.borrow().clone();
                    let editor = Editor::new(
                        "File",
                        Some(content),
                        Some(path_buf.clone()),
                        ss_clone.clone(),
                        theme_clone,
                    );

                    let page_index = notebook_clone.append_page(
                        &editor.content_row(),
                        Some(&editor.header),
                    );

                    notebook_clone.set_current_page(Some(page_index));
                    editor.update(&status_label_clone, &status_info_label_clone);
                    
                    editors_clone.borrow_mut().push(editor.clone());
                    *current_editor_clone.borrow_mut() = Some(editor.clone());

                    // Highlight the file in the explorer
                    file_explorer_clone.borrow().highlight_file(&path_buf);

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
                }
            }
        });

        // Directory expansion
        let file_explorer_clone2 = file_explorer_rc.clone();
        file_explorer_rc.borrow().connect_row_expanded(move |tree_store, iter, _path| {
            // Check if this is the first expansion (has dummy child)
            if let Some(child_iter) = tree_store.iter_children(Some(iter)) {
                let child_path: String = tree_store.value(&child_iter, 1).get().unwrap_or_default();
                if child_path.is_empty() {
                    // This is a dummy child, expand the directory
                    file_explorer_clone2.borrow().expand_directory(iter);
                }
            }
        });

        // Update file explorer highlighting when switching tabs
        let file_explorer_clone3 = file_explorer_rc.clone();
        let notebook_clone = notebook.clone();
        notebook_clone.connect_switch_page(move |_notebook, page, _page_num| {
            // Find which editor corresponds to this page
            let editors = editors_clone.borrow();
            for editor in editors.iter() {
                if editor.content_row().upcast_ref::<gtk4::Widget>() == page {
                    if let Some(ref path) = *editor.current_file.borrow() {
                        file_explorer_clone3.borrow().highlight_file(path);
                    }
                    break;
                }
            }
        });
    }

    // File Explorer Context Menu Actions
    {
        // NEW FILE ACTION
        let action = SimpleAction::new("explorer-new-file", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let parent_dir = if file_explorer_clone.borrow().get_selected_is_dir() {
                    selected_path.clone()
                } else {
                    selected_path.parent().unwrap_or(&selected_path).to_path_buf()
                };

                // Create dialog for file name input
                let dialog = Dialog::with_buttons(
                    Some("New File"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[("Cancel", ResponseType::Cancel), ("Create", ResponseType::Accept)],
                );

                let content_area = dialog.content_area();
                let entry = Entry::new();
                entry.set_placeholder_text(Some("filename.txt"));
                entry.set_margin_top(10);
                entry.set_margin_bottom(10);
                entry.set_margin_start(10);
                entry.set_margin_end(10);
                content_area.append(&entry);

                let file_explorer_clone2 = file_explorer_clone.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        let file_name = entry.text();
                        if !file_name.is_empty() {
                            if let Err(e) = file_explorer_clone2.borrow().create_file(&parent_dir, &file_name) {
                                eprintln!("Failed to create file: {}", e);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        app.add_action(&action);

        // NEW FOLDER ACTION
        let action = SimpleAction::new("explorer-new-folder", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let parent_dir = if file_explorer_clone.borrow().get_selected_is_dir() {
                    selected_path.clone()
                } else {
                    selected_path.parent().unwrap_or(&selected_path).to_path_buf()
                };

                // Create dialog for folder name input
                let dialog = Dialog::with_buttons(
                    Some("New Folder"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[("Cancel", ResponseType::Cancel), ("Create", ResponseType::Accept)],
                );

                let content_area = dialog.content_area();
                let entry = Entry::new();
                entry.set_placeholder_text(Some("folder_name"));
                entry.set_margin_top(10);
                entry.set_margin_bottom(10);
                entry.set_margin_start(10);
                entry.set_margin_end(10);
                content_area.append(&entry);

                let file_explorer_clone2 = file_explorer_clone.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        let folder_name = entry.text();
                        if !folder_name.is_empty() {
                            if let Err(e) = file_explorer_clone2.borrow().create_directory(&parent_dir, &folder_name) {
                                eprintln!("Failed to create folder: {}", e);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        app.add_action(&action);

        // DELETE ACTION
        let action = SimpleAction::new("explorer-delete", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let file_name = selected_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("this item");

                // Create confirmation dialog
                let dialog = MessageDialog::new(
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    MessageType::Question,
                    ButtonsType::YesNo,
                    &format!("Are you sure you want to delete '{}'?", file_name),
                );

                let file_explorer_clone2 = file_explorer_clone.clone();
                let selected_path_clone = selected_path.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Yes {
                        if let Err(e) = file_explorer_clone2.borrow().delete_file(&selected_path_clone) {
                            eprintln!("Failed to delete: {}", e);
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        app.add_action(&action);

        // RENAME ACTION
        let action = SimpleAction::new("explorer-rename", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let current_name = selected_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Create dialog for new name input
                let dialog = Dialog::with_buttons(
                    Some("Rename"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[("Cancel", ResponseType::Cancel), ("Rename", ResponseType::Accept)],
                );

                let content_area = dialog.content_area();
                let entry = Entry::new();
                entry.set_text(current_name);
                entry.set_margin_top(10);
                entry.set_margin_bottom(10);
                entry.set_margin_start(10);
                entry.set_margin_end(10);
                content_area.append(&entry);

                let file_explorer_clone2 = file_explorer_clone.clone();
                let selected_path_clone = selected_path.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Accept {
                        let new_name = entry.text();
                        if !new_name.is_empty() && new_name.as_str() != current_name {
                            if let Err(e) = file_explorer_clone2.borrow().rename_file(&selected_path_clone, &new_name) {
                                eprintln!("Failed to rename: {}", e);
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
    menu.append(Some("Toggle Theme"), Some("app.toggle-theme"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}