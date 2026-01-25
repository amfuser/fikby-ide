use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Dialog, FileChooserAction,
    FileChooserNative, Image, Label, MenuButton, Notebook, Orientation, ResponseType, ScrolledWindow, Popover,
};
use gtk4::{gdk, STYLE_PROVIDER_PRIORITY_APPLICATION};
use glib::clone;

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use crate::config;
use crate::editor::Editor;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;

/// Build the application UI.
///
/// Layout:
/// [menubar (MenuButtons with Popovers)]
/// [activity bar | sidebar stack | main area(custom tab bar + notebook editors)]
/// [status bar]
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

    // --- Menubar using MenuButtons + Popovers ---
    let menubar = GtkBox::new(Orientation::Horizontal, 0);
    menubar.style_context().add_class("menubar");

    // File Menu (plain labels)
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

    // Edit Menu (Select All / Copy)
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

    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    menubar.append(&spacer);

    // --- Activity bar (left) ---
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

    // --- Sidebar (stack) ---
    let sidebar_stack = gtk4::Stack::new();
    sidebar_stack.set_halign(Align::Start);
    sidebar_stack.set_vexpand(true);
    sidebar_stack.set_valign(Align::Start);
    let file_tree_placeholder = Label::new(Some("Explorer (files)"));
    sidebar_stack.add_titled(&file_tree_placeholder, Some("explorer"), "Explorer");

    // --- Main area: custom tab bar (our tabs) + notebook (editors) ---
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
    // Hide the notebook's own tabs; we render our custom tab bar instead
    editor_notebook.set_show_tabs(false);
    editor_notebook.set_hexpand(true);
    editor_notebook.set_vexpand(true);

    // syntect resources
    let ss = Rc::new(SyntaxSet::load_defaults_newlines());
    let ts = ThemeSet::load_defaults();
    let theme = Rc::new(
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().unwrap().clone()),
    );

    // Storage aligned by notebook page index:
    // editors[i] corresponds to notebook page i
    let editors: Rc<RefCell<Vec<Rc<Editor>>>> = Rc::new(RefCell::new(Vec::new()));
    let tab_widgets: Rc<RefCell<Vec<gtk4::Button>>> = Rc::new(RefCell::new(Vec::new()));
    // keep references to the label widgets inside each tab, so we can rename "Page N" after removes
    let tab_labels: Rc<RefCell<Vec<Label>>> = Rc::new(RefCell::new(Vec::new()));

    // Status bar
    let status_bar = GtkBox::new(Orientation::Horizontal, 6);
    status_bar.style_context().add_class("status");
    let status_left = Label::new(Some("Ln 1, Col 1"));
    status_left.set_halign(Align::Start);
    status_bar.append(&status_left);
    let status_spacer = Label::new(None);
    status_spacer.set_hexpand(true);
    status_bar.append(&status_spacer);

    // add_tab: Rc-wrapped closure so we can call it from multiple callbacks
    let add_tab: Rc<dyn Fn(&str, Option<String>, Option<PathBuf>, &Label) -> u32> = {
        let tab_bar = tab_bar.clone();
        let editors = editors.clone();
        let tab_widgets = tab_widgets.clone();
        let tab_labels = tab_labels.clone();
        let editor_notebook = editor_notebook.clone();
        let ss = ss.clone();
        let theme = theme.clone();

        Rc::new(move |title: &str, initial_text: Option<String>, path: Option<PathBuf>, status: &Label| -> u32 {
            // create editor
            let editor = Editor::new(title, initial_text, path.clone(), ss.clone(), theme.clone());
            editor.update(status);

            // append page without showing native tab label
            let page = editor_notebook.append_page(&editor.content_row(), None::<&gtk4::Widget>);
            let idx = page as usize;

            // insert into editors vec aligned to idx
            {
                let mut eds = editors.borrow_mut();
                if idx <= eds.len() {
                    eds.insert(idx, editor.clone());
                } else {
                    eds.push(editor.clone());
                }
            }

            // create tab widget
            let tab_button = Button::new();
            tab_button.style_context().add_class("editor-tab");
            tab_button.set_margin_top(4);
            tab_button.set_margin_bottom(4);
            tab_button.set_margin_start(6);
            tab_button.set_margin_end(6);

            let content_box = GtkBox::new(Orientation::Horizontal, 6);

            // icon
            let icon = match path.as_ref().and_then(|p| p.extension().and_then(|s| s.to_str())) {
                Some("rs") => Image::from_icon_name("text-x-rust"),
                _ => Image::from_icon_name("text-x-generic"),
            };
            icon.set_pixel_size(14);
            content_box.append(&icon);

            // label: file name if path provided, otherwise "Page N"
            let label_text = if let Some(p) = path.as_ref() {
                p.file_name().and_then(|s| s.to_str()).unwrap_or(title).to_string()
            } else {
                format!("Page {}", idx + 1)
            };
            let tab_label = Label::new(Some(&label_text));
            tab_label.set_tooltip_text(path.as_ref().and_then(|p| p.to_str()));
            content_box.append(&tab_label);

            // modified dot
            let modified_dot = Label::new(Some("●"));
            modified_dot.set_margin_start(6);
            modified_dot.set_visible(false);
            modified_dot.set_tooltip_text(Some("Unsaved changes"));
            content_box.append(&modified_dot);

            // close button
            let close_btn = Button::builder().halign(Align::Center).valign(Align::Center).build();
            let close_img = Image::from_icon_name("window-close-symbolic");
            close_img.set_pixel_size(12);
            close_btn.set_child(Some(&close_img));
            close_btn.set_tooltip_text(Some("Close"));
            content_box.append(&close_btn);

            tab_button.set_child(Some(&content_box));
            tab_bar.append(&tab_button);

            // insert tab widget and label into parallel vectors aligned to idx
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

            // clicking tab: set notebook current to the page for this editor
            {
                let notebook_cl = editor_notebook.clone();
                let page_widget = editor.content_row();
                tab_button.connect_clicked(move |_| {
                    if let Some(p) = notebook_cl.page_num(&page_widget) {
                        notebook_cl.set_current_page(Some(p));
                    }
                });
            }

            // close button logic uses notebook.page_num(page_widget) to find current index at click time
            {
                let notebook_cl = editor_notebook.clone();
                let editors_cl = editors.clone();
                let tab_widgets_cl = tab_widgets.clone();
                let tab_labels_cl = tab_labels.clone();
                let tab_bar_cl = tab_bar.clone();
                let page_widget = editor.content_row();
                let editor_cl = editor.clone();

                close_btn.connect_clicked(move |_| {
                    if let Some(page_idx) = notebook_cl.page_num(&page_widget) {
                        let idx_usize = page_idx as usize;

                        // if not dirty -> close immediately
                        if !*editor_cl.dirty.borrow() {
                            notebook_cl.remove_page(Some(page_idx));
                            {
                                let mut eds = editors_cl.borrow_mut();
                                if idx_usize < eds.len() { eds.remove(idx_usize); }
                            }
                            {
                                let mut tv = tab_widgets_cl.borrow_mut();
                                if idx_usize < tv.len() {
                                    let btn = tv.remove(idx_usize);
                                    tab_bar_cl.remove(&btn);
                                }
                            }
                            {
                                let mut lbls = tab_labels_cl.borrow_mut();
                                if idx_usize < lbls.len() { lbls.remove(idx_usize); }
                                // renumber remaining unnamed pages ("Page N")
                                for (i, lbl) in lbls.iter().enumerate() {
                                    // If corresponding editor's current_file is None, rename Page N
                                    let eds = editors_cl.borrow();
                                    if let Some(ed) = eds.get(i) {
                                        if ed.current_file.borrow().is_none() {
                                            lbl.set_text(&format!("Page {}", i + 1));
                                        }
                                    }
                                }
                            }
                            return;
                        }

                        // Dirty: show Save/Don't Save/Cancel dialog
                        let title = editor_cl
                            .current_file
                            .borrow()
                            .as_ref()
                            .and_then(|p: &PathBuf| p.file_name())
                            .and_then(|os| os.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Untitled".to_string());

                        let dlg = Dialog::builder()
                            .modal(true)
                            .title("Save changes")
                            .build();

                        let content_area = dlg.content_area();
                        let label = Label::new(Some(&format!("Save changes to \"{}\"?", title)));
                        content_area.append(&label);

                        dlg.add_button("Save", ResponseType::Yes);
                        dlg.add_button("Don't Save", ResponseType::No);
                        dlg.add_button("Cancel", ResponseType::Cancel);

                        dlg.connect_response(clone!(@strong notebook_cl, @strong editors_cl, @strong tab_widgets_cl, @strong tab_labels_cl, @strong tab_bar_cl, @strong page_widget, @strong editor_cl => move |dialog, resp| {
                            match resp {
                                ResponseType::Yes => {
                                    if let Some(path) = &*editor_cl.current_file.borrow() {
                                        if let Err(err) = editor_cl.save_to_path(path) {
                                            eprintln!("Failed to save: {}", err);
                                        } else {
                                            if let Some(idx2) = notebook_cl.page_num(&page_widget) {
                                                let idx2_usize = idx2 as usize;
                                                notebook_cl.remove_page(Some(idx2));
                                                let mut eds = editors_cl.borrow_mut();
                                                if idx2_usize < eds.len() { eds.remove(idx2_usize); }
                                                let mut tv = tab_widgets_cl.borrow_mut();
                                                if idx2_usize < tv.len() {
                                                    let btn = tv.remove(idx2_usize);
                                                    tab_bar_cl.remove(&btn);
                                                }
                                                let mut lbls = tab_labels_cl.borrow_mut();
                                                if idx2_usize < lbls.len() { lbls.remove(idx2_usize); }
                                                // renumber remaining unnamed pages
                                                for (i, lbl) in lbls.iter().enumerate() {
                                                    let eds = editors_cl.borrow();
                                                    if let Some(ed) = eds.get(i) {
                                                        if ed.current_file.borrow().is_none() {
                                                            lbl.set_text(&format!("Page {}", i + 1));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Save As flow (no current file)
                                        let save_dlg = FileChooserNative::new(
                                            Some("Save File As..."),
                                            None::<&gtk4::Window>,
                                            FileChooserAction::Save,
                                            Some("Save"),
                                            Some("Cancel"),
                                        );
                                        let buffer = editor_cl.main_buffer.clone();
                                        let ed_clone2 = editor_cl.clone();
                                        let notebook_inner = notebook_cl.clone();
                                        let editors_inner = editors_cl.clone();
                                        let tab_widgets_inner = tab_widgets_cl.clone();
                                        let tab_labels_inner = tab_labels_cl.clone();
                                        let tab_bar_inner = tab_bar_cl.clone();
                                        let page_widget_inner = page_widget.clone();
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
                                                            if let Some(idx3) = notebook_inner.page_num(&page_widget_inner) {
                                                                let idx3_usize = idx3 as usize;
                                                                notebook_inner.remove_page(Some(idx3));
                                                                let mut eds = editors_inner.borrow_mut();
                                                                if idx3_usize < eds.len() { eds.remove(idx3_usize); }
                                                                let mut tw = tab_widgets_inner.borrow_mut();
                                                                if idx3_usize < tw.len() {
                                                                    let btn = tw.remove(idx3_usize);
                                                                    tab_bar_inner.remove(&btn);
                                                                }
                                                                let mut lbls = tab_labels_inner.borrow_mut();
                                                                if idx3_usize < lbls.len() { lbls.remove(idx3_usize); }
                                                                // renumber remaining unnamed pages
                                                                for (i, lbl) in lbls.iter().enumerate() {
                                                                    let eds = editors_inner.borrow();
                                                                    if let Some(ed) = eds.get(i) {
                                                                        if ed.current_file.borrow().is_none() {
                                                                            lbl.set_text(&format!("Page {}", i + 1));
                                                                        }
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
                                    if let Some(idx2) = notebook_cl.page_num(&page_widget) {
                                        let idx2_usize = idx2 as usize;
                                        notebook_cl.remove_page(Some(idx2));
                                        let mut eds = editors_cl.borrow_mut();
                                        if idx2_usize < eds.len() { eds.remove(idx2_usize); }
                                        let mut tw = tab_widgets_cl.borrow_mut();
                                        if idx2_usize < tw.len() {
                                            let btn = tw.remove(idx2_usize);
                                            tab_bar_cl.remove(&btn);
                                        }
                                        let mut lbls = tab_labels_cl.borrow_mut();
                                        if idx2_usize < lbls.len() { lbls.remove(idx2_usize); }
                                        // renumber remaining unnamed pages
                                        for (i, lbl) in lbls.iter().enumerate() {
                                            let eds = editors_cl.borrow();
                                            if let Some(ed) = eds.get(i) {
                                                if ed.current_file.borrow().is_none() {
                                                    lbl.set_text(&format!("Page {}", i + 1));
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    // Cancel -> do nothing
                                }
                            }
                            dialog.close();
                        }));
                        dlg.show();
                    }
                });
            }

            // When notebook page switches, update active tab style and refresh editor status
            {
                let tab_widgets_cl = tab_widgets.clone();
                let editors_cl = editors.clone();
                let status_cl = status.clone();
                editor_notebook.connect_switch_page(move |nb, _, _| {
                    if let Some(current) = nb.current_page() {
                        let current_usize = current as usize;
                        // update tab widget active class
                        let mut tv = tab_widgets_cl.borrow_mut();
                        for (i, btn) in tv.iter().enumerate() {
                            if i == current_usize {
                                btn.style_context().add_class("editor-tab-active");
                            } else {
                                btn.style_context().remove_class("editor-tab-active");
                            }
                        }

                        // refresh status for current editor
                        if let Some(ed) = editors_cl.borrow().get(current_usize) {
                            ed.update(&status_cl);
                        }
                    }
                });
            }

            page
        })
    };

    // create an initial tab and select it
    let first_page = (add_tab)("Page 1", Some(String::new()), None, &status_left);
    editor_notebook.set_current_page(Some(first_page));

    // --- File menu wiring (New/Open/Save/Save As/Quit) ---

    // New
    {
        let add_tab = add_tab.clone();
        let notebook_cl = editor_notebook.clone();
        let status_cl = status_left.clone();
        file_new.connect_clicked(move |_| {
            let page = (add_tab)("Page", Some(String::new()), None, &status_cl);
            notebook_cl.set_current_page(Some(page));
        });
    }

    // Open
    {
        let add_tab = add_tab.clone();
        let notebook_cl = editor_notebook.clone();
        let status_cl = status_left.clone();
        let window_for_dialog = ApplicationWindow::builder().application(app).build();
        file_open.connect_clicked(move |_| {
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
            dlg.connect_response(move |dlg2, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dlg2.file() {
                        if let Some(path) = file.path() {
                            match fs::read_to_string(&path) {
                                Ok(text) => {
                                    let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
                                    let page = (add_tab)(title, Some(text), Some(path.clone()), &status_for_response);
                                    notebook.set_current_page(Some(page));
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
        file_save.connect_clicked(move |_| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                if let Some(ed) = editors_cl.borrow().get(idx) {
                    if let Some(path) = &*ed.current_file.borrow() {
                        if let Err(err) = ed.save_to_path(path) {
                            let err_dlg = Dialog::builder().title("Save error").modal(true).build();
                            err_dlg.content_area().append(&Label::new(Some(&format!("Failed to save file: {}", err))));
                            err_dlg.add_button("OK", ResponseType::Ok);
                            err_dlg.connect_response(|d, _| { d.close(); });
                            err_dlg.show();
                        }
                    } else {
                        // Save As if no path
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

    // Save As (explicit)
    {
        let notebook_cl = editor_notebook.clone();
        let editors_cl = editors.clone();
        file_saveas.connect_clicked(move |_| {
            if let Some(page) = notebook_cl.current_page() {
                let idx = page as usize;
                if let Some(ed) = editors_cl.borrow().get(idx) {
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

    // Wire Edit buttons (Select All / Copy) directly
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

    // Final composition
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