use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, FileChooserAction,
    FileChooserNative, Label, MenuButton, MessageDialog, Notebook, Orientation, PolicyType,
    ResponseType, ScrolledWindow, TextTag, TextView, WrapMode, ButtonsType, MessageType,
};
use gtk4::{gdk, gio, glib, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const APP_ID: &str = "org.gtk_rs.Fikby";

const CSS: &str = r#"
.menubar {
    background: #f5f5f5;
    padding: 4px 10px;
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
.gutter {
    background: #efefef;
    color: #444;
    padding-left: 6px;
    padding-right: 6px;
    font-family: monospace;
}
"#;

#[derive(Clone)]
struct Tab {
    main_buffer: gtk4::TextBuffer,
    gutter_buffer: gtk4::TextBuffer,
    current_file: Rc<RefCell<Option<PathBuf>>>,
    dirty: Rc<RefCell<bool>>,
    tag_cache: Rc<RefCell<HashMap<String, TextTag>>>,
}

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

    // --- File menu ---
    let file_menu = gio::Menu::new();
    file_menu.append_item(&gio::MenuItem::new(Some("New"), Some("app.new")));
    let recent_submenu = gio::Menu::new();
    recent_submenu.append(Some("Recent 1"), Some("app.open_recent_1"));
    recent_submenu.append(Some("Recent 2"), Some("app.open_recent_2"));
    let mi_open_recent = gio::MenuItem::new(Some("Open Recent"), None);
    mi_open_recent.set_submenu(Some(&recent_submenu));
    file_menu.append_item(&mi_open_recent);
    file_menu.append_item(&gio::MenuItem::new(Some("Open..."), Some("app.open")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save"), Some("app.save")));
    file_menu.append_item(&gio::MenuItem::new(Some("Save As..."), Some("app.saveas")));
    let section = gio::Menu::new();
    section.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));
    file_menu.append_section(None, &section);

    // --- Menubar UI ---
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

    // --- Notebook for tabs ---
    let notebook = Notebook::new();
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);

    // syntect resources
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.get("InspiredGitHub"))
        .cloned()
        .unwrap_or_else(|| ts.themes.values().next().unwrap().clone());

    let tabs: Rc<RefCell<Vec<Option<Rc<Tab>>>>> = Rc::new(RefCell::new(Vec::new()));

    // highlight helper (cloneable)
    // Note: syntect returns byte offsets. Convert to character offsets for GTK iter_at_offset.
    let ss = Rc::new(ss);
    let theme = Rc::new(theme);
    let highlight_with_syntect: Rc<
        dyn Fn(
            &gtk4::TextBuffer,
            &str,
            &str,
            &Rc<RefCell<HashMap<String, TextTag>>>,
        ) + 'static,
    > = {
        let ss = ss.clone();
        let theme = theme.clone();
        Rc::new(move |buffer, text, _syntax_name, tag_cache| {
            // Remove existing tags (those in cache)
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            for tag in tag_cache.borrow().values() {
                buffer.remove_tag(tag, &start, &end);
            }

            // choose rust syntax (we only use rust here)
            let syntax = ss.find_syntax_by_extension("rs").unwrap_or_else(|| ss.find_syntax_plain_text());

            let mut h = HighlightLines::new(syntax, &theme);
            let mut byte_offset: usize = 0;
            // Precompute a vector mapping byte indices to char indices for faster conversion.
            // We'll build a vector of cumulative char counts at each byte boundary of the text.
            // For simplicity and small-to-medium files, we'll do a char count on slices when needed.
            for line in LinesWithEndings::from(text) {
                // highlight_line gives ranges with byte indices relative to the line slice
                let ranges = h.highlight_line(line, &ss).unwrap_or_else(|_| vec![]);
                let mut local_byte_offset = 0usize;
                for (style, slice) in ranges {
                    // Compute byte indices for start/end within the whole text
                    let start_byte = byte_offset + local_byte_offset;
                    let end_byte = start_byte + slice.len();

                    // Convert byte indices to character offsets (GTK expects character offsets)
                    // Count chars up to the byte index by slicing; this is correct for UTF-8.
                    let s_chars = text[..start_byte].chars().count() as i32;
                    let e_chars = text[..end_byte].chars().count() as i32;

                    if style.foreground.a > 0 {
                        let fg = style.foreground;
                        let color = format!("#{:02X}{:02X}{:02X}", fg.r, fg.g, fg.b);

                        // create or reuse tag; ensure tag is in buffer's tag table
                        let tag = {
                            let mut cache = tag_cache.borrow_mut();
                            if let Some(existing) = cache.get(&color) {
                                existing.clone()
                            } else {
                                let t = TextTag::builder()
                                    .name(&format!("syn_{}", color.trim_start_matches('#')))
                                    .foreground(&color)
                                    .build();
                                // add to buffer's tag table
                                let table = buffer.tag_table();
                                table.add(&t);
                                cache.insert(color.clone(), t.clone());
                                t
                            }
                        };

                        let it_start = buffer.iter_at_offset(s_chars);
                        let it_end = buffer.iter_at_offset(e_chars);
                        buffer.apply_tag(&tag, &it_start, &it_end);
                    }
                    local_byte_offset += slice.len();
                }
                byte_offset += line.len();
            }
        })
    };

    // create-tab helper
    let add_tab: Rc<dyn Fn(&str, Option<String>, Option<PathBuf>) -> u32 + 'static> = {
        let notebook = notebook.clone();
        let tabs = tabs.clone();
        let highlight_with_syntect = highlight_with_syntect.clone();

        Rc::new(move |title: &str, initial_text: Option<String>, path: Option<PathBuf>| -> u32 {
            // main text view
            let main_view = TextView::new();
            main_view.set_wrap_mode(WrapMode::None);
            main_view.set_hexpand(true);
            main_view.set_vexpand(true);
            main_view.set_monospace(true);
            let main_buffer = main_view.buffer();

            // gutter view (line numbers)
            let gutter_view = TextView::new();
            gutter_view.set_wrap_mode(WrapMode::None);
            gutter_view.set_editable(false);
            gutter_view.set_cursor_visible(false);
            gutter_view.set_monospace(true);
            gutter_view.set_vexpand(true);
            gutter_view.set_can_focus(false);
            gutter_view.style_context().add_class("gutter");
            gutter_view.set_halign(Align::Start);
            let gutter_buffer = gutter_view.buffer();

            // scrolled windows
            let main_scrolled = ScrolledWindow::builder()
                .child(&main_view)
                .min_content_height(200)
                .hscrollbar_policy(PolicyType::Automatic)
                .vscrollbar_policy(PolicyType::Automatic)
                .build();

            let gutter_scrolled = ScrolledWindow::builder()
                .child(&gutter_view)
                .min_content_height(200)
                .hscrollbar_policy(PolicyType::Never)
                .vscrollbar_policy(PolicyType::Automatic)
                .min_content_width(80)
                .build();

            // share vertical adjustment
            let vadj: gtk4::Adjustment = main_scrolled.vadjustment();
            gutter_scrolled.set_vadjustment(Some(&vadj));

            // margins
            main_scrolled.set_margin_start(2);
            main_scrolled.set_margin_end(2);
            main_scrolled.set_margin_top(2);
            main_scrolled.set_margin_bottom(2);
            main_scrolled.set_hexpand(true);
            main_scrolled.set_vexpand(true);

            gutter_scrolled.set_margin_start(2);
            gutter_scrolled.set_margin_end(2);
            gutter_scrolled.set_margin_top(2);
            gutter_scrolled.set_margin_bottom(2);
            gutter_scrolled.set_vexpand(true);

            // content row: gutter + editor
            let content_row = GtkBox::new(Orientation::Horizontal, 0);
            content_row.append(&gutter_scrolled);
            content_row.append(&main_scrolled);
            content_row.set_hexpand(true);
            content_row.set_vexpand(true);

            // tab header: label + close (avoid expansion so notebook measurement stays sane)
            let tab_box = GtkBox::new(Orientation::Horizontal, 4);
            tab_box.set_hexpand(false);
            tab_box.set_vexpand(false);

            let tab_label = Label::new(Some(title));
            tab_label.set_hexpand(false);
            tab_label.set_vexpand(false);
            tab_box.append(&tab_label);

            let close_btn = Button::with_label("✕");
            close_btn.set_tooltip_text(Some("Close tab"));
            tab_box.append(&close_btn);

            // per-tab metadata
            let current_file = Rc::new(RefCell::new(path.clone()));
            let dirty = Rc::new(RefCell::new(false));
            let tag_cache: Rc<RefCell<HashMap<String, TextTag>>> = Rc::new(RefCell::new(HashMap::new()));

            let tab = Rc::new(Tab {
                main_buffer: main_buffer.clone(),
                gutter_buffer: gutter_buffer.clone(),
                current_file: current_file.clone(),
                dirty: dirty.clone(),
                tag_cache: tag_cache.clone(),
            });

            // helper to update gutter
            let update_gutter = {
                let main_buffer = main_buffer.clone();
                let gutter_buffer = gutter_buffer.clone();
                Rc::new(move || {
                    let start = main_buffer.start_iter();
                    let end = main_buffer.end_iter();
                    let content = main_buffer.text(&start, &end, false);
                    let line_count = if content.is_empty() { 1 } else { content.lines().count() };
                    let width = line_count.to_string().len();
                    let mut numbers = String::with_capacity(line_count * (width + 1));
                    for i in 1..=line_count {
                        numbers.push_str(&format!("{:>width$}\n", i, width = width));
                    }
                    gutter_buffer.set_text(&numbers);
                })
            };

            // connect_changed: two clones to avoid borrow/move issues
            {
                let main_signal_buf = main_buffer.clone();
                let main_closure_buf = main_buffer.clone();
                let dirty = dirty.clone();
                let update_clone = update_gutter.clone();
                let tag_cache = tag_cache.clone();
                let current_file_clone = current_file.clone();
                let highlight_fn = highlight_with_syntect.clone();

                main_signal_buf.connect_changed(move |_| {
                    *dirty.borrow_mut() = true;
                    (update_clone)();
                    if let Some(p) = &*current_file_clone.borrow() {
                        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                            if ext == "rs" {
                                let text = {
                                    let s = main_closure_buf.start_iter();
                                    let e = main_closure_buf.end_iter();
                                    main_closure_buf.text(&s, &e, false)
                                };
                                (highlight_fn)(&main_closure_buf, &text, "rust", &tag_cache);
                            }
                        }
                    }
                });
            }

            // initial text / gutter / highlight
            if let Some(text) = initial_text {
                main_buffer.set_text(&text);
            }
            (update_gutter)();

            if let Some(p) = &path {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    if ext == "rs" {
                        let s = main_buffer.start_iter();
                        let e = main_buffer.end_iter();
                        let content = main_buffer.text(&s, &e, false);
                        (highlight_with_syntect)(&main_buffer, &content, "rust", &tag_cache);
                    }
                }
            }

            // append page and store tab
            let page = notebook.append_page(&content_row, Some(&tab_box));
            {
                let mut t = tabs.borrow_mut();
                let idx = page as usize;
                if t.len() <= idx {
                    t.resize_with(idx + 1, || None);
                }
                t[idx] = Some(tab.clone());
            }

            // close button
            {
                let notebook = notebook.clone();
                let tabs = tabs.clone();
                let content_row_clone = content_row.clone();
                close_btn.connect_clicked(move |_| {
                    if let Some(idx) = notebook.page_num(&content_row_clone) {
                        notebook.remove_page(Some(idx));
                        let p_usize: usize = idx as usize;
                        if let Some(mut t) = tabs.try_borrow_mut().ok() {
                            if p_usize < t.len() {
                                t[p_usize] = None;
                            }
                        }
                    }
                });
            }

            page
        })
    };

    // add initial blank tab
    (add_tab)("Untitled", Some(String::new()), None);

    // main layout
    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.append(&menubar);
    vbox.append(&notebook);

    // build window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Fikby")
        .default_width(1000)
        .default_height(700)
        .child(&vbox)
        .build();

    // helper to get active tab metadata
    let get_active_tab = {
        let tabs = tabs.clone();
        let notebook = notebook.clone();
        move || -> Option<Rc<Tab>> {
            if let Some(page) = notebook.current_page() {
                let idx: usize = page as usize;
                tabs.borrow().get(idx).and_then(|opt| opt.as_ref().cloned())
            } else {
                None
            }
        }
    };

    // New action
    {
        let add_tab = add_tab.clone();
        let notebook = notebook.clone();
        let new_act = gio::SimpleAction::new("new", None);
        new_act.connect_activate(move |_, _| {
            let page = (add_tab)("Untitled", Some(String::new()), None);
            notebook.set_current_page(Some(page));
        });
        app.add_action(&new_act);
        app.set_accels_for_action("app.new", &["<Ctrl>N"]);
    }

    // Open action
    {
        let add_tab = add_tab.clone();
        let notebook = notebook.clone();
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
            let notebook = notebook.clone();
            dlg.connect_response(move |dlg, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dlg.file() {
                        if let Some(path) = file.path() {
                            match fs::read_to_string(&path) {
                                Ok(text) => {
                                    let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
                                    let page = (add_tab)(title, Some(text), Some(path.clone()));
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

    // Save action
    {
        let get_active_tab = get_active_tab.clone();
        let window_clone = window.clone();
        let save_act = gio::SimpleAction::new("save", None);
        save_act.connect_activate(move |_, _| {
            if let Some(tab) = get_active_tab() {
                let buffer = tab.main_buffer.clone();
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let text = buffer.text(&start, &end, false);

                if let Some(path) = &*tab.current_file.borrow() {
                    if let Err(err) = fs::write(path, text.as_str()) {
                        eprintln!("Failed to save file {}: {}", path.display(), err);
                    } else {
                        *tab.dirty.borrow_mut() = false;
                        if let Some(app) = window_clone.application() {
                            if let Some(win) = app.active_window() {
                                win.set_title(Some(&format!("Fikby - {}", path.display())));
                            }
                        }
                    }
                } else {
                    // Save As
                    let dlg = FileChooserNative::new(
                        Some("Save File"),
                        Some(&window_clone),
                        FileChooserAction::Save,
                        Some("Save"),
                        Some("Cancel"),
                    );
                    let buffer = buffer.clone();
                    let tab_clone = tab.clone();
                    let window2 = window_clone.clone();
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
                                        *tab_clone.current_file.borrow_mut() = Some(path.clone());
                                        *tab_clone.dirty.borrow_mut() = false;
                                        if let Some(app) = window2.application() {
                                            if let Some(win) = app.active_window() {
                                                win.set_title(Some(&format!("Fikby - {}", path.display())));
                                            }
                                        }
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
        app.add_action(&save_act);
        app.set_accels_for_action("app.save", &["<Ctrl>S"]);
    }

    // Save As action
    {
        let get_active_tab = get_active_tab.clone();
        let window_clone = window.clone();
        let saveas_act = gio::SimpleAction::new("saveas", None);
        saveas_act.connect_activate(move |_, _| {
            if let Some(tab) = get_active_tab() {
                let buffer = tab.main_buffer.clone();
                let dlg = FileChooserNative::new(
                    Some("Save File As..."),
                    Some(&window_clone),
                    FileChooserAction::Save,
                    Some("Save"),
                    Some("Cancel"),
                );
                let buffer = buffer.clone();
                let tab_clone = tab.clone();
                let window2 = window_clone.clone();
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
                                    *tab_clone.current_file.borrow_mut() = Some(path.clone());
                                    *tab_clone.dirty.borrow_mut() = false;
                                    if let Some(app) = window2.application() {
                                        if let Some(win) = app.active_window() {
                                            win.set_title(Some(&format!("Fikby - {}", path.display())));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let _ = dlg;
                });
                dlg.show();
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
            about.set_comments(Some("A small text editor example"));
            about.set_authors(&["Your Name"]);
            about.present();
        });
        app.add_action(&about_act);
        // use "F1" (no angle brackets) to avoid the accelerator parse error on some platforms
        app.set_accels_for_action("app.about", &["F1"]);
    }

    // Quit (prompt if any dirty)
    {
        let tabs = tabs.clone();
        let window_clone = window.clone();
        let quit_act = gio::SimpleAction::new("quit", None);
        quit_act.connect_activate(move |_, _| {
            let mut any_dirty = false;
            for opt in tabs.borrow().iter() {
                if let Some(tab) = opt {
                    if *tab.dirty.borrow() {
                        any_dirty = true;
                        break;
                    }
                }
            }
            if any_dirty {
                let dlg = MessageDialog::builder()
                    .transient_for(&window_clone)
                    .modal(true)
                    .message_type(MessageType::Question)
                    .buttons(ButtonsType::None)
                    .text("You have unsaved changes. Save before quitting?")
                    .build();
                dlg.add_buttons(&[
                    ("Save", ResponseType::Yes),
                    ("Don't Save", ResponseType::No),
                    ("Cancel", ResponseType::Cancel),
                ]);
                let window2 = window_clone.clone();
                dlg.connect_response(move |dlg, resp| {
                    match resp {
                        ResponseType::Yes => {
                            if let Some(app) = window2.application() {
                                if let Some(action) = app.lookup_action("save") {
                                    action.activate(None);
                                }
                                app.quit();
                            }
                        }
                        ResponseType::No => {
                            if let Some(app) = window2.application() {
                                app.quit();
                            }
                        }
                        _ => {
                            dlg.close();
                        }
                    }
                });
                dlg.present();
            } else {
                if let Some(app) = window_clone.application() {
                    app.quit();
                }
            }
        });
        app.add_action(&quit_act);
        app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    }

    // Wire About / Quit right-side buttons
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

    // Alt+F to popup File menu
    {
        let file_button = file_menu_button.clone();
        let filemenu_act = gio::SimpleAction::new("filemenu", None);
        filemenu_act.connect_activate(move |_, _| {
            file_button.popup();
        });
        app.add_action(&filemenu_act);
        app.set_accels_for_action("app.filemenu", &["<Alt>F"]);
    }

    window.present();
}