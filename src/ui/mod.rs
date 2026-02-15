use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, FileChooserAction,
    FileChooserNative, Image, Label, ListBox, MenuButton, Notebook, Orientation, ResponseType, ScrolledWindow, Popover,
};
use gtk4::{gdk, STYLE_PROVIDER_PRIORITY_APPLICATION};
use gtk4::gio;
use glib::clone;

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::config;
use crate::editor::Editor;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;

/// Build the application UI with keybindings, command palette (basic), soft-wrap toggle, and status info.
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

    // --- Menubar ---
    let menubar = GtkBox::new(Orientation::Horizontal, 0);
    menubar.style_context().add_class("menubar");

    // File menu
    let file_button = MenuButton::builder().label("File").margin_start(6).margin_end(6).build();
    file_button.style_context().add_class("menubutton");
    let file_pop = Popover::new();
    let file_vbox = GtkBox::new(Orientation::Vertical, 0);
    let file_new = Button::with_label("New");
    let file_open = Button::with_label("Open...");
    let file_save = Button::with_label("Save");
    let file_saveas = Button::with_label("Save As...");
    let file_quit = Button::with_label("Quit");
    file_vbox.append(&file_new);
    file_vbox.append(&file_open);
    file_vbox.append(&file_save);
    file_vbox.append(&file_saveas);
    file_vbox.append(&file_quit);
    file_pop.set_child(Some(&file_vbox));
    file_button.set_popover(Some(&file_pop));
    menubar.append(&file_button);

    // Edit menu
    let edit_button = MenuButton::builder().label("Edit").margin_start(6).margin_end(6).build();
    edit_button.style_context().add_class("menubutton");
    let edit_pop = Popover::new();
    let edit_vbox = GtkBox::new(Orientation::Vertical, 0);
    let select_all_btn = Button::with_label("Select All\tCtrl+A");
    let copy_btn = Button::with_label("Copy\tCtrl+C");
    edit_vbox.append(&select_all_btn);
    edit_vbox.append(&copy_btn);
    edit_pop.set_child(Some(&edit_vbox));
    edit_button.set_popover(Some(&edit_pop));
    menubar.append(&edit_button);

    // View menu (placeholder)
    let view_button = MenuButton::builder().label("View").margin_start(6).margin_end(6).build();
    view_button.style_context().add_class("menubutton");
    let view_pop = Popover::new();
    let view_vbox = GtkBox::new(Orientation::Vertical, 0);
    view_pop.set_child(Some(&view_vbox));
    view_button.set_popover(Some(&view_pop));
    menubar.append(&view_button);

    // Help menu
    let help_button = MenuButton::builder().label("Help").margin_start(6).margin_end(6).build();
    help_button.style_context().add_class("menubutton");
    let help_pop = Popover::new();
    let help_vbox = GtkBox::new(Orientation::Vertical, 0);
    let help_about = Button::with_label("About");
    help_vbox.append(&help_about);
    help_pop.set_child(Some(&help_vbox));
    help_button.set_popover(Some(&help_pop));
    menubar.append(&help_button);

    // spacer
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    menubar.append(&spacer);

    // Activity bar
    let activity_bar = GtkBox::new(Orientation::Vertical, 6);
    activity_bar.style_context().add_class("activity-bar");
    activity_bar.set_margin_start(4);
    activity_bar.set_margin_end(4);
    activity_bar.set_margin_top(8);
    activity_bar.set_margin_bottom(8);
    activity_bar.set_hexpand(false);
    let explorer_btn = Button::builder().child(&Image::from_icon_name("folder-symbolic")).build();
    explorer_btn.set_tooltip_text(Some("Explorer"));
    activity_bar.append(&explorer_btn);
    let search_btn = Button::builder().child(&Image::from_icon_name("system-search-symbolic")).build();
    search_btn.set_tooltip_text(Some("Search"));
    activity_bar.append(&search_btn);

    // Sidebar stack
    let sidebar_stack = gtk4::Stack::new();
    sidebar_stack.set_halign(Align::Start);
    sidebar_stack.set_vexpand(true);
    sidebar_stack.set_valign(Align::Start);
    let file_tree_placeholder = Label::new(Some("Explorer (files)"));
    sidebar_stack.add_titled(&file_tree_placeholder, Some("explorer"), "Explorer");

    // Main area: tab bar + notebook
    let tab_bar_scrolled = ScrolledWindow::builder()
        .min_content_height(38)
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let tab_bar = GtkBox::new(Orientation::Horizontal, 4);
    tab_bar.set_margin_start(6);
    tab_bar.set_margin_end(6);
    tab_bar_scrolled.set_child(Some(&tab_bar));

    let editor_notebook = Notebook::new();
    editor_notebook.set_show_tabs(false);
    editor_notebook.set_hexpand(true);
    editor_notebook.set_vexpand(true);

    // syntect
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();
    let theme = Rc::new(
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().unwrap().clone()),
    );

    // storage
    let editors: Rc<RefCell<Vec<Rc<Editor>>>> = Rc::new(RefCell::new(Vec::new()));
    let tab_widgets: Rc<RefCell<Vec<gtk4::Button>>> = Rc::new(RefCell::new(Vec::new()));
    let tab_labels: Rc<RefCell<Vec<Label>>> = Rc::new(RefCell::new(Vec::new()));

    // status bar (left = Ln/Col, right = path & size)
    let status_bar = GtkBox::new(Orientation::Horizontal, 6);
    status_bar.style_context().add_class("status");
    let status_left = Label::new(Some("Ln 1, Col 1"));
    status_left.set_halign(Align::Start);
    let status_info = Label::new(Some("Untitled — 0 bytes"));
    status_info.set_halign(Align::End);
    status_info.set_hexpand(true);
    status_bar.append(&status_left);
    status_bar.append(&status_info);

    // Helper to remove a page by u32 index (keeps vectors in sync)
    let remove_page_at = {
        let editors = editors.clone();
        let tab_widgets = tab_widgets.clone();
        let tab_labels = tab_labels.clone();
        let tab_bar = tab_bar.clone();
        move |idx_u32: u32| {
            let idx = idx_u32 as usize;
            if let Ok(mut eds) = editors.try_borrow_mut() {
                if idx < eds.len() {
                    eds.remove(idx);
                }
            }
            if let Ok(mut tv) = tab_widgets.try_borrow_mut() {
                if idx < tv.len() {
                    let btn = tv.remove(idx);
                    tab_bar.remove(&btn);
                }
            }
            if let Ok(mut lbls) = tab_labels.try_borrow_mut() {
                if idx < lbls.len() {
                    lbls.remove(idx);
                }
                // renumber unnamed pages
                for (i, lbl) in lbls.iter().enumerate() {
                    if let Some(ed) = editors.borrow().get(i) {
                        if ed.current_file.borrow().is_none() {
                            lbl.set_text(&format!("Page {}", i + 1));
                        }
                    }
                }
            }
        }
    };

    // add_tab closure
    let add_tab: Rc<dyn Fn(&str, Option<String>, Option<PathBuf>, &Label, &Label) -> u32> = {
        let editor_notebook = editor_notebook.clone();
        let tab_bar = tab_bar.clone();
        let editors = editors.clone();
        let tab_widgets = tab_widgets.clone();
        let tab_labels = tab_labels.clone();
        let ss = ss.clone();
        let theme = theme.clone();
        let remove_page_fn = remove_page_at.clone();

        Rc::new(move |title: &str, initial_text: Option<String>, path: Option<PathBuf>, status: &Label, info: &Label| -> u32 {
            // create editor
            let editor = Editor::new(title, initial_text, path.clone(), ss.clone(), theme.clone());
            editor.update(status, info);

            // append page without native tab label
            let page = editor_notebook.append_page(&editor.content_row(), None::<&gtk4::Widget>);
            let idx = page as usize;

            // insert editor in editors vec
            {
                let mut eds = editors.borrow_mut();
                if idx <= eds.len() {
                    eds.insert(idx, editor.clone());
                } else {
                    eds.push(editor.clone());
                }
            }

            // build custom tab UI
            let tab_button = Button::new();
            tab_button.style_context().add_class("editor-tab");
            tab_button.set_margin_top(4);
            tab_button.set_margin_bottom(4);
            tab_button.set_margin_start(6);
            tab_button.set_margin_end(6);

            let content_box = GtkBox::new(Orientation::Horizontal, 6);

            let icon = match path.as_ref().and_then(|p| p.extension().and_then(|s| s.to_str())) {
                Some("rs") => Image::from_icon_name("text-x-rust"),
                _ => Image::from_icon_name("text-x-generic"),
            };
            icon.set_pixel_size(14);
            content_box.append(&icon);

            let label_text = if let Some(p) = path.as_ref() {
                p.file_name().and_then(|s| s.to_str()).unwrap_or(title).to_string()
            } else {
                format!("Page {}", idx + 1)
            };
            let tab_label = Label::new(Some(&label_text));
            tab_label.set_tooltip_text(path.as_ref().and_then(|p| p.to_str()));
            content_box.append(&tab_label);

            let modified_dot = Label::new(Some("●"));
            modified_dot.set_margin_start(6);
            modified_dot.set_visible(false);
            content_box.append(&modified_dot);

            let close_btn = Button::builder().halign(Align::Center).valign(Align::Center).build();
            let close_img = Image::from_icon_name("window-close-symbolic");
            close_img.set_pixel_size(12);
            close_btn.set_child(Some(&close_img));
            close_btn.set_tooltip_text(Some("Close"));
            content_box.append(&close_btn);

            tab_button.set_child(Some(&content_box));
            tab_bar.append(&tab_button);

            // insert tab widget and label into vectors
            {
                let mut tv = tab_widgets.borrow_mut();
                if idx <= tv.len() {
                    tv.insert(idx, tab_button.clone());
                } else {
                    tv.push(tab_button.clone());
                }
            }
            {
                let mut lbls = tab_labels.borrow_mut();
                if idx <= lbls.len() {
                    lbls.insert(idx, tab_label.clone());
                } else {
                    lbls.push(tab_label.clone());
                }
            }

            // click tab -> activate page
            {
                let notebook_cl = editor_notebook.clone();
                let page_widget = editor.content_row();
                tab_button.connect_clicked(move |_| {
                    if let Some(p) = notebook_cl.page_num(&page_widget) {
                        notebook_cl.set_current_page(Some(p));
                    }
                });
            }

            // close behavior
            {
                let notebook_cl = editor_notebook.clone();
                let editor_cl = editor.clone();
                let remove_fn = remove_page_fn.clone();
                close_btn.connect_clicked(move |_| {
                    if let Some(page_idx) = notebook_cl.page_num(&editor_cl.content_row()) {
                        let idx_u32 = page_idx;
                        if !*editor_cl.dirty.borrow() {
                            notebook_cl.remove_page(Some(idx_u32));
                            remove_fn(idx_u32);
                            return;
                        }

                        // dirty -> prompt
                        let title = editor_cl
                            .current_file
                            .borrow()
                            .as_ref()
                            .and_then(|p: &PathBuf| p.file_name())
                            .and_then(|os| os.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Untitled".to_string());

                        let dlg = Dialog::builder().modal(true).title("Save changes").build();
                        dlg.content_area().append(&Label::new(Some(&format!("Save changes to \"{}\"?", title))));
                        dlg.add_button("Save", ResponseType::Yes);
                        dlg.add_button("Don't Save", ResponseType::No);
                        dlg.add_button("Cancel", ResponseType::Cancel);

                        let notebook_resp = notebook_cl.clone();
                        let editor_resp = editor_cl.clone();
                        let remove_resp = remove_fn.clone();

                        dlg.connect_response(move |dialog, resp| {
                            match resp {
                                ResponseType::Yes => {
                                    let path_opt = { editor_resp.current_file.borrow().clone() };
                                    if let Some(path) = path_opt {
                                        if let Err(err) = editor_resp.save_to_path(&path) {
                                            eprintln!("Failed to save: {}", err);
                                        } else {
                                            if let Some(p2) = notebook_resp.page_num(&editor_resp.content_row()) {
                                                notebook_resp.remove_page(Some(p2));
                                                remove_resp(p2);
                                            }
                                        }
                                    } else {
                                        // Save As flow
                                        let save_dlg = FileChooserNative::new(
                                            Some("Save File As..."),
                                            None::<&gtk4::Window>,
                                            FileChooserAction::Save,
                                            Some("Save"),
                                            Some("Cancel"),
                                        );
                                        let buffer = editor_resp.main_buffer.clone();
                                        let editor_inner = editor_resp.clone();
                                        let notebook_inner = notebook_resp.clone();
                                        let remove_inner = remove_resp.clone();
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
                                                            *editor_inner.current_file.borrow_mut() = Some(path.clone());
                                                            *editor_inner.dirty.borrow_mut() = false;
                                                            if let Some(p3) = notebook_inner.page_num(&editor_inner.content_row()) {
                                                                notebook_inner.remove_page(Some(p3));
                                                                remove_inner(p3);
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
                                    if let Some(p2) = notebook_resp.page_num(&editor_resp.content_row()) {
                                        notebook_resp.remove_page(Some(p2));
                                        remove_resp(p2);
                                    }
                                }
                                _ => {}
                            }
                            dialog.close();
                        });

                        dlg.show();
                    }
                });
            }

            // update active tab style & status on switch
            {
                let tab_widgets2 = tab_widgets.clone();
                let editors2 = editors.clone();
                let status_clone = status.clone();
                let info_clone = info.clone();
                editor_notebook.connect_switch_page(move |nb, _, _| {
                    if let Some(current) = nb.current_page() {
                        let current_usize = current as usize;
                        if let Ok(tv) = tab_widgets2.try_borrow_mut() {
                            for (i, btn) in tv.iter().enumerate() {
                                if i == current_usize {
                                    btn.style_context().add_class("editor-tab-active");
                                } else {
                                    btn.style_context().remove_class("editor-tab-active");
                                }
                            }
                        }
                        if let Some(ed) = editors2.borrow().get(current_usize) {
                            ed.update(&status_clone, &info_clone);
                        }
                    }
                });
            }

            page
        })
    };

    // initial tab
    let first_page = (add_tab)("Page 1", Some(String::new()), None, &status_left, &status_info);
    editor_notebook.set_current_page(Some(first_page));

    // --- App actions & accelerators ---
    // Save action
    {
        let notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        let save_act = gio::SimpleAction::new("save", None);
        save_act.connect_activate(clone!(@strong notebook_cl, @strong editors_cl => move |_, _| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                let ed_opt = { editors_cl.borrow().get(idx).cloned() };
                if let Some(ed) = ed_opt {
                    let path_opt = { ed.current_file.borrow().clone() };
                    if let Some(path) = path_opt {
                        let _ = ed.save_to_path(&path);
                    }
                }
            }
        }));
        app.add_action(&save_act);
        app.set_accels_for_action("app.save", &["<Ctrl>S"]);
    }

    // Toggle wrap (Ctrl+T)
    {
        let notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        let wrap_act = gio::SimpleAction::new("toggle-wrap", None);
        wrap_act.connect_activate(clone!(@strong notebook_cl, @strong editors_cl => move |_, _| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                if let Some(ed) = editors_cl.borrow().get(idx) {
                    ed.toggle_wrap();
                }
            }
        }));
        app.add_action(&wrap_act);
        app.set_accels_for_action("app.toggle-wrap", &["<Ctrl>T"]);
    }

    // Command palette (Ctrl+P)
    {
        let editor_notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        let add_tab_cl = add_tab.clone();
        let sidebar_cl = sidebar_stack.clone();
        let palette_act = gio::SimpleAction::new("command-palette", None);
        palette_act.connect_activate(clone!(@strong editor_notebook_cl, @strong editors_cl, @strong add_tab_cl, @strong sidebar_cl => move |_, _| {
            // build command list
            let commands: Vec<(&str, Box<dyn Fn()>)> = vec![
                ("New Tab", Box::new(clone!(@strong add_tab_cl, @strong editor_notebook_cl => move || {
                    let page = (add_tab_cl)("Page", Some(String::new()), None, &Label::new(None), &Label::new(None));
                    editor_notebook_cl.set_current_page(Some(page));
                }))),
                ("Save File", Box::new(clone!(@strong editor_notebook_cl, @strong editors_cl => move || {
                    if let Some(page) = editor_notebook_cl.current_page() {
                        let idx = page as usize;
                        if let Some(ed) = editors_cl.borrow().get(idx) {
                            let path_opt = { ed.current_file.borrow().clone() };
                            if let Some(path) = path_opt {
                                let _ = ed.save_to_path(&path);
                            }
                        }
                    }
                }))),
                ("Toggle Wrap", Box::new(clone!(@strong editor_notebook_cl, @strong editors_cl => move || {
                    if let Some(page) = editor_notebook_cl.current_page() {
                        let idx = page as usize;
                        if let Some(ed) = editors_cl.borrow().get(idx) {
                            ed.toggle_wrap();
                        }
                    }
                }))),
                ("Toggle Sidebar", Box::new(clone!(@strong sidebar_cl => move || {
                    let vis = sidebar_cl.is_visible();
                    sidebar_cl.set_visible(!vis);
                }))),
            ];

            // dialog UI
            let dlg = Dialog::builder().modal(true).title("Command Palette").build();
            let content = dlg.content_area();

            let entry = gtk4::Entry::new();
            entry.set_placeholder_text(Some("Type a command..."));
            content.append(&entry);

            // Initial listbox
            let listbox = ListBox::new();
            for (label, _) in commands.iter() {
                let row = gtk4::Label::new(Some(label));
                listbox.append(&row);
            }
            content.append(&listbox);

            // Keep a reference to the current listbox so we can replace it on filter changes
            let current_listbox: Rc<RefCell<Option<ListBox>>> = Rc::new(RefCell::new(Some(listbox.clone())));

            let commands_rc = Rc::new(commands);
            let matcher = SkimMatcherV2::default();
            let content_clone = content.clone();
            let current_listbox_clone = current_listbox.clone();
            let dlg_for_closure = dlg.clone(); // clone for use inside closures

            entry.connect_changed(clone!(@strong commands_rc => move |e| {
                let q = e.text().to_string();

                // Build a new listbox with matching rows
                let new_listbox = ListBox::new();
                for (label, _) in commands_rc.iter() {
                    if q.is_empty() {
                        let row = gtk4::Label::new(Some(label));
                        new_listbox.append(&row);
                    } else if matcher.fuzzy_match(label, &q).is_some() {
                        let row = gtk4::Label::new(Some(label));
                        new_listbox.append(&row);
                    }
                }

                // Replace the old listbox in the dialog content with the new one
                if let Some(old) = current_listbox_clone.borrow_mut().take() {
                    content_clone.remove(&old);
                }
                content_clone.append(&new_listbox);
                *current_listbox_clone.borrow_mut() = Some(new_listbox.clone());

                // Attach row-activated handler for the newly created listbox
                let commands_for_rows = commands_rc.clone();
                let dlg_inner = dlg_for_closure.clone();
                new_listbox.connect_row_activated(clone!(@strong commands_for_rows, @strong dlg_inner => move |_, row| {
                    if let Some(lbl) = row.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
                        let text = lbl.text().to_string();
                        for (label, action) in commands_for_rows.iter() {
                            if label == &text {
                                (action)();
                                break;
                            }
                        }
                    }
                    dlg_inner.close();
                }));
            }));

            // row activation on initial listbox as well
            let commands_initial = commands_rc.clone();
            let dlg_clone = dlg.clone();
            listbox.connect_row_activated(move |_, row| {
                if let Some(lbl) = row.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
                    let text = lbl.text().to_string();
                    for (label, action) in commands_initial.iter() {
                        if label == &text {
                            (action)();
                            break;
                        }
                    }
                }
                dlg_clone.close();
            });

            dlg.add_button("Close", ResponseType::Close);
            dlg.show();
            entry.grab_focus();
        }));
        app.add_action(&palette_act);
        app.set_accels_for_action("app.command-palette", &["<Ctrl>P"]);
    }

    // Wire File menu buttons (New/Open/Save/Save As/Quit)
    {
        let add_tab = add_tab.clone();
        let notebook_cl = editor_notebook.clone();
        let status_left_cl = status_left.clone();
        let status_info_cl = status_info.clone();
        file_new.connect_clicked(move |_| {
            let page = (add_tab)("Page", Some(String::new()), None, &status_left_cl, &status_info_cl);
            notebook_cl.set_current_page(Some(page));
        });
    }

    // Open
    {
        let add_tab = add_tab.clone();
        let notebook_cl = editor_notebook.clone();
        let status_left_cl = status_left.clone();
        let status_info_cl = status_info.clone();
        let window_for_dialog = ApplicationWindow::builder().application(app).build();
        file_open.connect_clicked(move |_| {
            let dlg = FileChooserNative::new(
                Some("Open File"),
                Some(&window_for_dialog),
                FileChooserAction::Open,
                Some("Open"),
                Some("Cancel"),
            );
            let add_tab2 = add_tab.clone();
            let notebook2 = notebook_cl.clone();
            let status2 = status_left_cl.clone();
            let info2 = status_info_cl.clone();
            dlg.connect_response(move |dlg2, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dlg2.file() {
                        if let Some(path) = file.path() {
                            match fs::read_to_string(&path) {
                                Ok(text) => {
                                    let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
                                    let page = (add_tab2)(title, Some(text), Some(path.clone()), &status2, &info2);
                                    notebook2.set_current_page(Some(page));
                                }
                                Err(err) => {
                                    let err_dlg = Dialog::builder().title("Open error").modal(true).build();
                                    err_dlg.content_area().append(&Label::new(Some(&format!("Failed to read file: {}", err))));
                                    err_dlg.add_button("OK", ResponseType::Ok);
                                    err_dlg.connect_response(|d, _| { d.close(); });
                                    err_dlg.show();
                                }
                            }
                        }
                    }
                }
                let _ = dlg2;
            });
            dlg.show();
        });
    }

    // Save
    {
        let notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        let status_left_cl = status_left.clone();
        let status_info_cl = status_info.clone();
        file_save.connect_clicked(move |_| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                let ed_opt = { editors_cl.borrow().get(idx).cloned() };
                if let Some(ed) = ed_opt {
                    let path_opt = { ed.current_file.borrow().clone() };
                    if let Some(path) = path_opt {
                        let _ = ed.save_to_path(&path);
                        ed.update(&status_left_cl, &status_info_cl);
                    } else {
                        // Save As
                        let save_dlg = FileChooserNative::new(
                            Some("Save File As..."),
                            None::<&gtk4::Window>,
                            FileChooserAction::Save,
                            Some("Save"),
                            Some("Cancel"),
                        );
                        let buffer = ed.main_buffer.clone();
                        let ed_clone = ed.clone();
                        save_dlg.connect_response(move |dlg2, resp| {
                            if resp == ResponseType::Accept {
                                if let Some(file) = dlg2.file() {
                                    if let Some(path) = file.path() {
                                        let s = buffer.start_iter();
                                        let e = buffer.end_iter();
                                        let content = buffer.text(&s, &e, false);
                                        if let Err(err) = fs::write(&path, content.as_str()) {
                                            let err_dlg = Dialog::builder().title("Save error").modal(true).build();
                                            err_dlg.content_area().append(&Label::new(Some(&format!("Failed to save file {}: {}", path.display(), err))));
                                            err_dlg.add_button("OK", ResponseType::Ok);
                                            err_dlg.connect_response(|d, _| { d.close(); });
                                            err_dlg.show();
                                        } else {
                                            *ed_clone.current_file.borrow_mut() = Some(path.clone());
                                            *ed_clone.dirty.borrow_mut() = false;
                                        }
                                    }
                                }
                            }
                            let _ = dlg2;
                        });
                        save_dlg.show();
                    }
                }
            }
        });
    }

    // Save As explicit
    {
        let notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        file_saveas.connect_clicked(move |_| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                let ed_opt = { editors_cl.borrow().get(idx).cloned() };
                if let Some(ed) = ed_opt {
                    let buffer = ed.main_buffer.clone();
                    let dlg = FileChooserNative::new(
                        Some("Save File As..."),
                        None::<&gtk4::Window>,
                        FileChooserAction::Save,
                        Some("Save"),
                        Some("Cancel"),
                    );
                    let buffer2 = buffer.clone();
                    let ed_clone = ed.clone();
                    dlg.connect_response(move |dlg2, resp| {
                        if resp == ResponseType::Accept {
                            if let Some(file) = dlg2.file() {
                                if let Some(path) = file.path() {
                                    let s = buffer2.start_iter();
                                    let e = buffer2.end_iter();
                                    let content = buffer2.text(&s, &e, false);
                                    if let Err(err) = fs::write(&path, content.as_str()) {
                                        let err_dlg = Dialog::builder().title("Save error").modal(true).build();
                                        err_dlg.content_area().append(&Label::new(Some(&format!("Failed to save file {}: {}", path.display(), err))));
                                        err_dlg.add_button("OK", ResponseType::Ok);
                                        err_dlg.connect_response(|d, _| { d.close(); });
                                        err_dlg.show();
                                    } else {
                                        *ed_clone.current_file.borrow_mut() = Some(path.clone());
                                        *ed_clone.dirty.borrow_mut() = false;
                                    }
                                }
                            }
                        }
                        let _ = dlg2;
                    });
                    dlg.show();
                }
            }
        });
    }

    // Quit
    {
        let app_cl = app.clone();
        file_quit.connect_clicked(move |_| {
            app_cl.quit();
        });
    }

    // Help -> About
    {
        let app_cl = app.clone();
        help_about.connect_clicked(move |_| {
            if let Some(act) = app_cl.lookup_action("about") {
                act.activate(None);
            }
        });
    }

    // Edit: Select All
    {
        let editor_notebook = editor_notebook.clone();
        let editors = editors.clone();
        select_all_btn.connect_clicked(move |_| {
            if let Some(page) = editor_notebook.current_page() {
                let idx = page as usize;
                if let Some(ed) = editors.borrow().get(idx) {
                    let buffer = ed.main_buffer.clone();
                    let start = buffer.start_iter();
                    let end = buffer.end_iter();
                    buffer.select_range(&start, &end);
                }
            }
        });
    }

    // Edit: Copy
    {
        let editor_notebook = editor_notebook.clone();
        let editors = editors.clone();
        copy_btn.connect_clicked(move |_| {
            if let Some(page) = editor_notebook.current_page() {
                let idx = page as usize;
                if let Some(ed) = editors.borrow().get(idx) {
                    let buffer = ed.main_buffer.clone();
                    if let Some(display) = gdk::Display::default() {
                        let clipboard = display.clipboard();
                        buffer.copy_clipboard(&clipboard);
                    }
                }
            }
        });
    }

    // Layout compose
    let main_area = GtkBox::new(Orientation::Horizontal, 0);
    main_area.append(&activity_bar);
    main_area.append(&sidebar_stack);

    let center_area = GtkBox::new(Orientation::Vertical, 0);
    center_area.append(&tab_bar_scrolled);
    center_area.append(&editor_notebook);
    center_area.set_hexpand(true);
    center_area.set_vexpand(true);
    main_area.append(&center_area);

    let root_vbox = GtkBox::new(Orientation::Vertical, 0);
    root_vbox.append(&menubar);
    root_vbox.append(&main_area);
    root_vbox.append(&status_bar);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Fikby")
        .default_width(1200)
        .default_height(800)
        .child(&root_vbox)
        .build();

    window.present();
}