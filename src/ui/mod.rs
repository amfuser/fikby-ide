use gtk4::gio::SimpleAction;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ComboBoxText, Dialog, Entry, Grid,
    Label, MessageDialog, MessageType, Notebook, Orientation, Paned, Popover, PopoverMenu,
    ResponseType, ScrolledWindow, SpinButton, TextBuffer, TextView,
};
use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, process::Child, process::Command, process::Stdio};

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::config::ThemeMode;
use crate::editor::Editor;
use crate::file_explorer::FileExplorer;
use crate::find_replace::FindReplaceDialog;
use crate::settings::{EditorSettings, IndentStyle};

pub fn build_ui(app: &Application) {
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();

    // Start with dark theme by default
    let current_theme_mode = Rc::new(RefCell::new(ThemeMode::Dark));
    let theme = Rc::new(ts.themes["base16-ocean.dark"].clone());
    let current_theme: Rc<RefCell<Rc<Theme>>> = Rc::new(RefCell::new(theme.clone()));

    // Global editor settings (persisted via GSettings)
    let editor_settings = Rc::new(EditorSettings::new());

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
    let view_menu = create_view_menu();

    menubar.append(&file_menu);
    menubar.append(&edit_menu);
    menubar.append(&view_menu);

    // Right-aligned buttons
    let right_box = GtkBox::new(Orientation::Horizontal, 0);
    right_box.set_hexpand(true);
    right_box.set_halign(gtk4::Align::End);

    let run_button = Button::with_label("▶ Run");
    run_button.style_context().add_class("right-button");
    right_box.append(&run_button);

    let stop_button = Button::with_label("■ Stop");
    stop_button.set_sensitive(false);
    stop_button.style_context().add_class("right-button");
    right_box.append(&stop_button);

    let run_status_label = Label::new(Some("Idle"));
    run_status_label.set_margin_end(8);
    right_box.append(&run_status_label);

    let settings_btn = Button::new();
    settings_btn.set_icon_name("emblem-system-symbolic");
    settings_btn.style_context().add_class("right-button");
    right_box.append(&settings_btn);

    menubar.append(&right_box);
    vbox.append(&menubar);

    // Settings popover UI
    {
        let popover = Popover::new();
        popover.set_has_arrow(true);
        popover.set_autohide(true);
        popover.set_parent(&settings_btn);

        let grid = Grid::new();
        grid.set_margin_top(10);
        grid.set_margin_bottom(10);
        grid.set_margin_start(10);
        grid.set_margin_end(10);
        grid.set_row_spacing(8);
        grid.set_column_spacing(10);

        let style_label = Label::new(Some("Indent using:"));
        style_label.set_halign(gtk4::Align::Start);

        let style_combo = ComboBoxText::new();
        style_combo.append(Some("spaces"), "Spaces");
        style_combo.append(Some("tabs"), "Tabs");

        // IMPORTANT: set_active_id returns bool; ensure this is a statement.
        match editor_settings.indent_style() {
            IndentStyle::Spaces => {
                style_combo.set_active_id(Some("spaces"));
            }
            IndentStyle::Tabs => {
                style_combo.set_active_id(Some("tabs"));
            }
        }

        let width_label = Label::new(Some("Indent width:"));
        width_label.set_halign(gtk4::Align::Start);

        // SpinButton (1..8)
        let width_spin = SpinButton::with_range(1.0, 8.0, 1.0);
        width_spin.set_numeric(true);
        width_spin.set_value(editor_settings.indent_width() as f64);

        grid.attach(&style_label, 0, 0, 1, 1);
        grid.attach(&style_combo, 1, 0, 1, 1);
        grid.attach(&width_label, 0, 1, 1, 1);
        grid.attach(&width_spin, 1, 1, 1, 1);

        popover.set_child(Some(&grid));

        let settings_for_style = editor_settings.clone();
        style_combo.connect_changed(move |combo| {
            let style = match combo.active_id().as_deref() {
                Some("tabs") => IndentStyle::Tabs,
                _ => IndentStyle::Spaces,
            };
            settings_for_style.set_indent_style(style);
        });

        let settings_for_width = editor_settings.clone();
        width_spin.connect_value_changed(move |spin| {
            settings_for_width.set_indent_width(spin.value() as i32);
        });

        settings_btn.connect_clicked(move |_| {
            popover.popup();
        });
    }

    // Main paned (sidebar + editor area)
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);

    // Create file explorer
    let file_explorer_rc = FileExplorer::new();

    // Set current directory as root (or fallback to home)
    let root_dir = std::env::current_dir().unwrap_or_else(|_| {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
    });

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
    notebook.set_size_request(-1, 100);

    let output_buffer = TextBuffer::new(None);
    let output_view = TextView::new();
    output_view.set_editable(false);
    output_view.set_monospace(true);
    output_view.set_vexpand(false);
    output_view.set_hexpand(true);
    output_view.set_buffer(Some(&output_buffer));

    let output_scroller = ScrolledWindow::new();
    output_scroller.set_vexpand(false);
    output_scroller.set_min_content_height(140);
    output_scroller.set_child(Some(&output_view));

    let editor_area = GtkBox::new(Orientation::Vertical, 6);
    editor_area.append(&notebook);
    editor_area.append(&output_scroller);

    // Status bar
    let status_bar = GtkBox::new(Orientation::Horizontal, 10);
    status_bar.style_context().add_class("status");

    let status_label = Label::new(Some("Ln 1, Col 1"));
    let status_info_label = Label::new(Some("Ready"));
    status_info_label.set_hexpand(true);
    status_info_label.set_halign(gtk4::Align::Start);

    status_bar.append(&status_label);
    status_bar.append(&status_info_label);

    paned.set_end_child(Some(&editor_area));
    vbox.append(&paned);
    vbox.append(&status_bar);

    window.set_child(Some(&vbox));

    // Store references in Rc<RefCell<>> for sharing
    let editors: Rc<RefCell<Vec<Rc<Editor>>>> = Rc::new(RefCell::new(Vec::new()));
    let current_editor: Rc<RefCell<Option<Rc<Editor>>>> = Rc::new(RefCell::new(None));
    let active_child: Rc<RefCell<Option<Child>>> = Rc::new(RefCell::new(None));
    let active_run_artifacts: Rc<RefCell<Option<RunArtifacts>>> = Rc::new(RefCell::new(None));

    // RUN ACTION (runs current editor buffer in-process workflow)
    {
        let current_editor_clone = current_editor.clone();
        let active_child_clone = active_child.clone();
        let active_run_artifacts_clone = active_run_artifacts.clone();
        let output_buffer_clone = output_buffer.clone();
        let run_button_clone = run_button.clone();
        let stop_button_clone = stop_button.clone();
        let run_status_label_clone = run_status_label.clone();

        run_button.connect_clicked(move |_| {
            if active_child_clone.borrow().is_some() {
                return;
            }

            let editor = if let Some(editor) = current_editor_clone.borrow().as_ref() {
                editor.clone()
            } else {
                append_output(
                    &output_buffer_clone,
                    "No active editor tab is open. Open a file or create a new tab first.\n",
                );
                run_status_label_clone.set_text("Idle");
                return;
            };

            let source_text = editor.get_text();
            if source_text.trim().is_empty() {
                append_output(&output_buffer_clone, "Cannot run an empty editor buffer.\n");
                run_status_label_clone.set_text("Idle");
                return;
            }

            let current_file = editor.current_file.borrow().clone();
            let (mut command, run_artifacts, command_display) =
                match build_run_command(&source_text, current_file.as_ref()) {
                    Ok(spec) => spec,
                    Err(err) => {
                        append_output(&output_buffer_clone, &format!("{err}\n"));
                        run_status_label_clone.set_text("Idle");
                        return;
                    }
                };

            output_buffer_clone.set_text("");
            append_output(&output_buffer_clone, &format!("$ {command_display}\n"));

            command.stdout(Stdio::piped()).stderr(Stdio::piped());

            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(err) => {
                    append_output(
                        &output_buffer_clone,
                        &format!("Failed to start execution process: {err}\n"),
                    );
                    run_status_label_clone.set_text("Idle");
                    return;
                }
            };

            let (sender, receiver) = glib::MainContext::channel::<String>(glib::PRIORITY_DEFAULT);
            let output_buffer_for_receiver = output_buffer_clone.clone();
            receiver.attach(None, move |line| {
                append_output(&output_buffer_for_receiver, &line);
                glib::Continue(true)
            });

            if let Some(stdout) = child.stdout.take() {
                let sender_stdout = sender.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => {
                                let _ = sender_stdout.send(format!("{text}\n"));
                            }
                            Err(err) => {
                                let _ = sender_stdout.send(format!("[stdout read error] {err}\n"));
                                break;
                            }
                        }
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                let sender_stderr = sender.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => {
                                let _ = sender_stderr.send(format!("{text}\n"));
                            }
                            Err(err) => {
                                let _ = sender_stderr.send(format!("[stderr read error] {err}\n"));
                                break;
                            }
                        }
                    }
                });
            }

            run_button_clone.set_sensitive(false);
            stop_button_clone.set_sensitive(true);
            run_status_label_clone.set_text("Running");
            *active_run_artifacts_clone.borrow_mut() = Some(run_artifacts);
            *active_child_clone.borrow_mut() = Some(child);

            let active_child_for_timer = active_child_clone.clone();
            let active_artifacts_for_timer = active_run_artifacts_clone.clone();
            let run_button_for_timer = run_button_clone.clone();
            let stop_button_for_timer = stop_button_clone.clone();
            let run_status_for_timer = run_status_label_clone.clone();
            let output_for_timer = output_buffer_clone.clone();

            glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
                let mut exit_line: Option<String> = None;
                let mut should_stop = false;

                {
                    let mut child_slot = active_child_for_timer.borrow_mut();
                    if let Some(child_ref) = child_slot.as_mut() {
                        match child_ref.try_wait() {
                            Ok(Some(status)) => {
                                exit_line =
                                    Some(format!("\n[process exited with status: {status}]\n"));
                                should_stop = true;
                            }
                            Ok(None) => {}
                            Err(err) => {
                                exit_line = Some(format!("\n[process monitoring failed: {err}]\n"));
                                should_stop = true;
                            }
                        }
                    } else {
                        should_stop = true;
                    }
                }

                if should_stop {
                    active_child_for_timer.borrow_mut().take();
                    if let Some(artifacts) = active_artifacts_for_timer.borrow_mut().take() {
                        cleanup_run_artifacts(&artifacts);
                    }
                    if let Some(line) = exit_line {
                        append_output(&output_for_timer, &line);
                    }
                    run_button_for_timer.set_sensitive(true);
                    stop_button_for_timer.set_sensitive(false);
                    run_status_for_timer.set_text("Idle");
                    return glib::Continue(false);
                }

                glib::Continue(true)
            });
        });
    }

    // STOP ACTION (terminates active run process)
    {
        let active_child_clone = active_child.clone();
        let active_run_artifacts_clone = active_run_artifacts.clone();
        let output_buffer_clone = output_buffer.clone();
        let run_button_clone = run_button.clone();
        let stop_button_clone = stop_button.clone();
        let run_status_label_clone = run_status_label.clone();

        stop_button.connect_clicked(move |_| {
            if let Some(mut child) = active_child_clone.borrow_mut().take() {
                let _ = child.kill();
                let _ = child.wait();
                append_output(&output_buffer_clone, "\n[process stopped]\n");
            }

            if let Some(artifacts) = active_run_artifacts_clone.borrow_mut().take() {
                cleanup_run_artifacts(&artifacts);
            }

            run_button_clone.set_sensitive(true);
            stop_button_clone.set_sensitive(false);
            run_status_label_clone.set_text("Idle");
        });
    }

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
        let settings_clone = editor_settings.clone();

        action.connect_activate(move |_, _| {
            let theme_clone = current_theme_clone.borrow().clone();
            let editor = Editor::new(
                "Untitled",
                None,
                None,
                ss_clone.clone(),
                theme_clone,
                settings_clone.clone(),
            );

            let page_index =
                notebook_clone.append_page(&editor.content_row(), Some(&editor.header));
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
                    editors_clone2
                        .borrow_mut()
                        .retain(|e| !Rc::ptr_eq(e, &editor_clone));
                }
            });
        });

        window.add_action(&action);
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
        let settings_clone = editor_settings.clone();

        action.connect_activate(move |_, _| {
            let dialog = gtk4::FileChooserDialog::new(
                Some("Open File"),
                Some(&window_clone),
                gtk4::FileChooserAction::Open,
                &[
                    ("Cancel", gtk4::ResponseType::Cancel),
                    ("Open", gtk4::ResponseType::Accept),
                ],
            );

            let notebook_clone2 = notebook_clone.clone();
            let editors_clone2 = editors_clone.clone();
            let current_editor_clone2 = current_editor_clone.clone();
            let ss_clone2 = ss_clone.clone();
            let current_theme_clone2 = current_theme_clone.clone();
            let status_label_clone2 = status_label_clone.clone();
            let status_info_label_clone2 = status_info_label_clone.clone();
            let settings_clone2 = settings_clone.clone();

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
                                    settings_clone2.clone(),
                                );

                                let page_index = notebook_clone2
                                    .append_page(&editor.content_row(), Some(&editor.header));

                                notebook_clone2.set_current_page(Some(page_index));
                                editor.update(&status_label_clone2, &status_info_label_clone2);

                                editors_clone2.borrow_mut().push(editor.clone());
                                *current_editor_clone2.borrow_mut() = Some(editor.clone());

                                // Connect close button
                                let notebook_clone3 = notebook_clone2.clone();
                                let editors_clone3 = editors_clone2.clone();
                                let editor_clone = editor.clone();
                                editor.close_button.connect_clicked(move |_| {
                                    if let Some(page_num) =
                                        notebook_clone3.page_num(&editor_clone.content_row())
                                    {
                                        notebook_clone3.remove_page(Some(page_num));
                                        editors_clone3
                                            .borrow_mut()
                                            .retain(|e| !Rc::ptr_eq(e, &editor_clone));
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

        window.add_action(&action);
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
                    let _ = editor.save_to_path(&path);
                } else {
                    let dialog = gtk4::FileChooserDialog::new(
                        Some("Save File"),
                        Some(&window_clone),
                        gtk4::FileChooserAction::Save,
                        &[
                            ("Cancel", gtk4::ResponseType::Cancel),
                            ("Save", gtk4::ResponseType::Accept),
                        ],
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

        window.add_action(&action);
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
                    &[
                        ("Cancel", gtk4::ResponseType::Cancel),
                        ("Save", gtk4::ResponseType::Accept),
                    ],
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

        window.add_action(&action);
    }

    // QUIT ACTION — closes the currently focused window
    {
        let action = SimpleAction::new("quit", None);
        let app_clone = app.clone();
        action.connect_activate(move |_, _| {
            if let Some(win) = app_clone.active_window() {
                win.close();
            }
        });
        app.add_action(&action);
    }

    // NEW WINDOW ACTION — opens a second independent IDE window
    {
        let action = SimpleAction::new("new-window", None);
        let app_clone = app.clone();
        action.connect_activate(move |_, _| {
            build_ui(&app_clone);
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

        window.add_action(&action);
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

        window.add_action(&action);
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

        window.add_action(&action);
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

        window.add_action(&action);
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

        window.add_action(&action);
    }

    // FIND ACTION
    {
        let action = SimpleAction::new("find", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                let window_ref: &gtk4::Window = window_clone.upcast_ref();
                let find_dialog = FindReplaceDialog::new(window_ref, editor.clone());

                if let Some((start, end)) = editor.main_buffer.selection_bounds() {
                    let selected = editor.main_buffer.text(&start, &end, false);
                    if !selected.is_empty() && !selected.contains('\n') {
                        find_dialog.set_find_text(&selected);
                    }
                }

                find_dialog.show();
            }
        });

        window.add_action(&action);
    }

    // REPLACE ACTION
    {
        let action = SimpleAction::new("replace", None);
        let current_editor_clone = current_editor.clone();
        let window_clone = window.clone();

        action.connect_activate(move |_, _| {
            if let Some(editor) = current_editor_clone.borrow().as_ref() {
                let window_ref: &gtk4::Window = window_clone.upcast_ref();
                let find_dialog = FindReplaceDialog::new(window_ref, editor.clone());

                if let Some((start, end)) = editor.main_buffer.selection_bounds() {
                    let selected = editor.main_buffer.text(&start, &end, false);
                    if !selected.is_empty() && !selected.contains('\n') {
                        find_dialog.set_find_text(&selected);
                    }
                }

                find_dialog.show();
            }
        });

        window.add_action(&action);
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

        window.add_action(&action);
    }

    // TOGGLE THEME ACTION (updates UI CSS + syntect highlighting theme)
    {
        let action = SimpleAction::new("toggle-theme", None);
        let current_theme_mode_clone = current_theme_mode.clone();
        let current_theme_clone = current_theme.clone();
        let editors_clone = editors.clone();

        action.connect_activate(move |_, _| {
            let new_mode = {
                let mut mode = current_theme_mode_clone.borrow_mut();
                *mode = match *mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                *mode
            };

            // Apply UI theme (embedded CSS)
            crate::load_css(new_mode);

            // Update syntax highlighting theme
            let ts = ThemeSet::load_defaults();
            let theme_name = new_mode.syntax_theme_name();
            let new_theme = if let Some(theme) = ts.themes.get(theme_name) {
                Rc::new(theme.clone())
            } else {
                eprintln!("Warning: Theme '{}' not found, using fallback", theme_name);
                Rc::new(ts.themes.values().next().unwrap().clone())
            };
            *current_theme_clone.borrow_mut() = new_theme.clone();

            for editor in editors_clone.borrow().iter() {
                editor.set_theme(new_theme.clone());
            }
        });

        window.add_action(&action);
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

    // Shortcuts
    app.set_accels_for_action("win.new", &["<Ctrl>N"]);
    app.set_accels_for_action("win.open", &["<Ctrl>O"]);
    app.set_accels_for_action("win.save", &["<Ctrl>S"]);
    app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>S"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    app.set_accels_for_action("app.new-window", &["<Ctrl><Shift>N"]);
    app.set_accels_for_action("win.undo", &["<Ctrl>Z"]);
    app.set_accels_for_action("win.redo", &["<Ctrl><Shift>Z"]);
    app.set_accels_for_action("win.cut", &["<Ctrl>X"]);
    app.set_accels_for_action("win.copy", &["<Ctrl>C"]);
    app.set_accels_for_action("win.paste", &["<Ctrl>V"]);
    app.set_accels_for_action("win.find", &["<Ctrl>F"]);
    app.set_accels_for_action("win.replace", &["<Ctrl>H"]);
    app.set_accels_for_action("win.toggle-theme", &["<Ctrl>T"]);

    // Create initial empty tab
    let initial_editor = Editor::new(
        "Untitled",
        None,
        None,
        ss.clone(),
        theme.clone(),
        editor_settings.clone(),
    );
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
                editors_clone
                    .borrow_mut()
                    .retain(|e| !Rc::ptr_eq(e, &editor_clone));
            }
        });
    }

    // Connect file explorer actions
    {
        let file_explorer_clone = file_explorer_rc.clone();
        let notebook_clone = notebook.clone();
        let editors_clone = editors.clone();
        let current_editor_clone = current_editor.clone();
        let ss_clone = ss.clone();
        let current_theme_clone = current_theme.clone();
        let status_label_clone = status_label.clone();
        let status_info_label_clone = status_info_label.clone();
        let settings_clone = editor_settings.clone();

        file_explorer_rc
            .borrow()
            .connect_row_activated(move |path_buf, is_dir| {
                if !is_dir {
                    if let Ok(content) = std::fs::read_to_string(&path_buf) {
                        let theme_clone = current_theme_clone.borrow().clone();
                        let editor = Editor::new(
                            "File",
                            Some(content),
                            Some(path_buf.clone()),
                            ss_clone.clone(),
                            theme_clone,
                            settings_clone.clone(),
                        );

                        let page_index =
                            notebook_clone.append_page(&editor.content_row(), Some(&editor.header));
                        notebook_clone.set_current_page(Some(page_index));
                        editor.update(&status_label_clone, &status_info_label_clone);

                        editors_clone.borrow_mut().push(editor.clone());
                        *current_editor_clone.borrow_mut() = Some(editor.clone());

                        file_explorer_clone.borrow().highlight_file(&path_buf);

                        let notebook_clone2 = notebook_clone.clone();
                        let editors_clone2 = editors_clone.clone();
                        let editor_clone = editor.clone();
                        editor.close_button.connect_clicked(move |_| {
                            if let Some(page_num) =
                                notebook_clone2.page_num(&editor_clone.content_row())
                            {
                                notebook_clone2.remove_page(Some(page_num));
                                editors_clone2
                                    .borrow_mut()
                                    .retain(|e| !Rc::ptr_eq(e, &editor_clone));
                            }
                        });
                    }
                }
            });

        // Directory expansion
        let file_explorer_clone2 = file_explorer_rc.clone();
        file_explorer_rc
            .borrow()
            .connect_row_expanded(move |tree_store, iter, _path| {
                if let Some(child_iter) = tree_store.iter_children(Some(iter)) {
                    let child_path: String = tree_store.get(&child_iter, 1);
                    if child_path.is_empty() {
                        file_explorer_clone2.borrow().expand_directory(iter);
                    }
                }
            });

        // Update file explorer highlighting when switching tabs
        let file_explorer_clone3 = file_explorer_rc.clone();
        let editors_clone2 = editors.clone();
        notebook.connect_switch_page(move |_notebook, page, _page_num| {
            let editors = editors_clone2.borrow();
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

    // File Explorer Context Menu Actions (kept identical to original behavior)
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
                    selected_path
                        .parent()
                        .unwrap_or(&selected_path)
                        .to_path_buf()
                };

                let dialog = Dialog::with_buttons(
                    Some("New File"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[
                        ("Cancel", ResponseType::Cancel),
                        ("Create", ResponseType::Accept),
                    ],
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
                            if let Err(e) = file_explorer_clone2
                                .borrow()
                                .create_file(&parent_dir, &file_name)
                            {
                                eprintln!("Failed to create file: {}", e);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        window.add_action(&action);

        // NEW FOLDER ACTION
        let action = SimpleAction::new("explorer-new-folder", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let parent_dir = if file_explorer_clone.borrow().get_selected_is_dir() {
                    selected_path.clone()
                } else {
                    selected_path
                        .parent()
                        .unwrap_or(&selected_path)
                        .to_path_buf()
                };

                let dialog = Dialog::with_buttons(
                    Some("New Folder"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[
                        ("Cancel", ResponseType::Cancel),
                        ("Create", ResponseType::Accept),
                    ],
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
                            if let Err(e) = file_explorer_clone2
                                .borrow()
                                .create_directory(&parent_dir, &folder_name)
                            {
                                eprintln!("Failed to create folder: {}", e);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        window.add_action(&action);

        // DELETE ACTION
        let action = SimpleAction::new("explorer-delete", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let file_name = selected_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("this item");

                let dialog = MessageDialog::new(
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    MessageType::Question,
                    gtk4::ButtonsType::YesNo,
                    &format!("Are you sure you want to delete '{}'?", file_name),
                );

                let file_explorer_clone2 = file_explorer_clone.clone();
                let selected_path_clone = selected_path.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == ResponseType::Yes {
                        if let Err(e) = file_explorer_clone2
                            .borrow()
                            .delete_file(&selected_path_clone)
                        {
                            eprintln!("Failed to delete: {}", e);
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        window.add_action(&action);

        // RENAME ACTION
        let action = SimpleAction::new("explorer-rename", None);
        let window_clone = window.clone();
        let file_explorer_clone = file_explorer_rc.clone();

        action.connect_activate(move |_, _| {
            if let Some(selected_path) = file_explorer_clone.borrow().get_selected_path() {
                let current_name = selected_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let dialog = Dialog::with_buttons(
                    Some("Rename"),
                    Some(&window_clone),
                    gtk4::DialogFlags::MODAL,
                    &[
                        ("Cancel", ResponseType::Cancel),
                        ("Rename", ResponseType::Accept),
                    ],
                );

                let content_area = dialog.content_area();
                let entry = Entry::new();
                entry.set_text(&current_name);
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
                            if let Err(e) = file_explorer_clone2
                                .borrow()
                                .rename_file(&selected_path_clone, &new_name)
                            {
                                eprintln!("Failed to rename: {}", e);
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.show();
            }
        });

        window.add_action(&action);
    }

    window.present();
}

#[derive(Debug)]
struct RunArtifacts {
    source_path: PathBuf,
    binary_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum RunLanguage {
    Rust,
    Python,
    JavaScript,
    Shell,
}

fn detect_run_language(current_file: Option<&PathBuf>) -> Option<RunLanguage> {
    match current_file
        .and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
    {
        Some("rs") => Some(RunLanguage::Rust),
        Some("py") => Some(RunLanguage::Python),
        Some("js") => Some(RunLanguage::JavaScript),
        Some("sh") => Some(RunLanguage::Shell),
        Some(_) => None,
        None => Some(RunLanguage::Rust),
    }
}

fn build_run_command(
    source_text: &str,
    current_file: Option<&PathBuf>,
) -> Result<(Command, RunArtifacts, String), String> {
    let language = detect_run_language(current_file).ok_or_else(|| {
        "Unsupported file type for Run. Supported extensions: .rs, .py, .js, .sh".to_string()
    })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let temp_dir = std::env::temp_dir();

    match language {
        RunLanguage::Rust => {
            let source_path = temp_dir.join(format!("fikby_run_{now}.rs"));
            let binary_path = temp_dir.join(format!("fikby_run_{now}_bin"));
            fs::write(&source_path, source_text)
                .map_err(|err| format!("Failed to write temporary source file: {err}"))?;

            let command_line = format!(
                "rustc \"{}\" -o \"{}\" && \"{}\"",
                source_path.display(),
                binary_path.display(),
                binary_path.display()
            );

            let mut command = Command::new("sh");
            command.arg("-lc").arg(&command_line);

            Ok((
                command,
                RunArtifacts {
                    source_path,
                    binary_path: Some(binary_path),
                },
                command_line,
            ))
        }
        RunLanguage::Python => {
            let source_path = temp_dir.join(format!("fikby_run_{now}.py"));
            fs::write(&source_path, source_text)
                .map_err(|err| format!("Failed to write temporary source file: {err}"))?;

            let mut command = Command::new("python3");
            command.arg(&source_path);

            Ok((
                command,
                RunArtifacts {
                    source_path,
                    binary_path: None,
                },
                format!("python3 {}", source_path.display()),
            ))
        }
        RunLanguage::JavaScript => {
            let source_path = temp_dir.join(format!("fikby_run_{now}.js"));
            fs::write(&source_path, source_text)
                .map_err(|err| format!("Failed to write temporary source file: {err}"))?;

            let mut command = Command::new("node");
            command.arg(&source_path);

            Ok((
                command,
                RunArtifacts {
                    source_path,
                    binary_path: None,
                },
                format!("node {}", source_path.display()),
            ))
        }
        RunLanguage::Shell => {
            let source_path = temp_dir.join(format!("fikby_run_{now}.sh"));
            fs::write(&source_path, source_text)
                .map_err(|err| format!("Failed to write temporary source file: {err}"))?;

            let mut command = Command::new("bash");
            command.arg(&source_path);

            Ok((
                command,
                RunArtifacts {
                    source_path,
                    binary_path: None,
                },
                format!("bash {}", source_path.display()),
            ))
        }
    }
}

fn cleanup_run_artifacts(artifacts: &RunArtifacts) {
    let _ = fs::remove_file(&artifacts.source_path);
    if let Some(binary_path) = artifacts.binary_path.as_ref() {
        let _ = fs::remove_file(binary_path);
    }
}

fn append_output(output_buffer: &TextBuffer, text: &str) {
    let mut end = output_buffer.end_iter();
    output_buffer.insert(&mut end, text);
}

fn create_file_menu() -> gtk4::MenuButton {
    let menu_button = gtk4::MenuButton::new();
    menu_button.set_label("File");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("New"), Some("win.new"));
    menu.append(Some("New Window"), Some("app.new-window"));
    menu.append(Some("Open"), Some("win.open"));
    menu.append(Some("Save"), Some("win.save"));
    menu.append(Some("Save As"), Some("win.save-as"));
    menu.append(Some("Quit"), Some("app.quit"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}

fn create_edit_menu() -> gtk4::MenuButton {
    let menu_button = gtk4::MenuButton::new();
    menu_button.set_label("Edit");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("Undo"), Some("win.undo"));
    menu.append(Some("Redo"), Some("win.redo"));
    menu.append(Some("Cut"), Some("win.cut"));
    menu.append(Some("Copy"), Some("win.copy"));
    menu.append(Some("Paste"), Some("win.paste"));
    menu.append(Some("Find"), Some("win.find"));
    menu.append(Some("Replace"), Some("win.replace"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}

fn create_view_menu() -> gtk4::MenuButton {
    let menu_button = gtk4::MenuButton::new();
    menu_button.set_label("View");
    menu_button.style_context().add_class("menubutton");

    let menu = gtk4::gio::Menu::new();
    menu.append(Some("Toggle Word Wrap"), Some("win.toggle-wrap"));
    menu.append(Some("Toggle Theme"), Some("win.toggle-theme"));

    let popover = PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    menu_button
}
