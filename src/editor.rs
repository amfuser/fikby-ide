use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, ScrolledWindow, TextBuffer, TextView, WrapMode, PolicyType, Button};
use gtk4::TextTag;
use glib::clone;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use syntect::parsing::SyntaxSet;
use syntect::highlighting::Theme;

use ropey::Rope;

use crate::highlight;

/// Strongly typed TabId (newtype)
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub usize);

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FileState {
    New,
    Saved(PathBuf),
}

/// Editor encapsulates a text editor view, gutter label, buffers and tag cache.
#[allow(dead_code)]
pub struct Editor {
    pub main_view: TextView,
    pub main_buffer: TextBuffer,
    pub gutter_label: Label,
    pub content_row: GtkBox,
    pub header: GtkBox, // tab header (icon + label + close) — kept for potential future use
    pub tab_label: Label, // label inside header (for updating dirty / name)
    pub close_button: Button, // the "x" button for closing the tab (also kept on header)
    pub current_file: Rc<RefCell<Option<PathBuf>>>,
    pub dirty: Rc<RefCell<bool>>,
    pub tag_cache: Rc<RefCell<HashMap<String, TextTag>>>,
    // keep syntect refs to avoid reloading repeatedly
    ss: Rc<SyntaxSet>,
    theme: Rc<Theme>,

    // Performance fields
    rope: Rc<RefCell<Rope>>,
    highlight_gen: Arc<AtomicU64>, // generation id to cancel stale jobs
    // sender is cloned into worker closures; receiver is attached to the main context in new()
    highlight_sender: glib::Sender<(u64, String)>,
}

impl Editor {
    /// Create a new Editor with explicit title, optional initial text and path.
    /// Sets up background highlighting pipeline (worker -> main thread) and generation-based cancellation.
    pub fn new(title: &str, initial_text: Option<String>, path: Option<PathBuf>, ss: Rc<SyntaxSet>, theme: Rc<Theme>) -> Rc<Self> {
        // main TextView
        let main_view = TextView::new();
        main_view.set_wrap_mode(WrapMode::None);
        main_view.set_hexpand(true);
        main_view.set_vexpand(true);
        main_view.set_monospace(true);
        let main_buffer = main_view.buffer();

        // gutter as Label
        let gutter_label = Label::new(None);
        gutter_label.style_context().add_class("gutter");
        gutter_label.set_halign(Align::Start);

        // scrolled windows
        let main_scrolled = ScrolledWindow::builder()
            .child(&main_view)
            .min_content_height(200)
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .build();

        let gutter_scrolled = ScrolledWindow::builder()
            .child(&gutter_label)
            .min_content_height(200)
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_width(80)
            .build();

        // share vertical adjustment
        let vadj: gtk4::Adjustment = main_scrolled.vadjustment();
        gutter_scrolled.set_vadjustment(Some(&vadj));

        // content row
        let content_row = GtkBox::new(gtk4::Orientation::Horizontal, 0);
        content_row.append(&gutter_scrolled);
        content_row.append(&main_scrolled);
        content_row.set_hexpand(true);
        content_row.set_vexpand(true);

        // header for tab: file icon + label + close (kept but not used as notebook tab label)
        let header = GtkBox::new(gtk4::Orientation::Horizontal, 6);
        header.set_hexpand(false);
        header.set_vexpand(false);

        // file icon (try to use a source/text icon; theme-dependent)
        let file_icon = Image::from_icon_name("text-x-generic");
        file_icon.set_pixel_size(16);
        header.append(&file_icon);

        // label with filename
        let display_title = path
            .as_ref()
            .and_then(|p: &PathBuf| p.file_name())
            .and_then(|os| os.to_str())
            .unwrap_or(title)
            .to_string();

        let tab_label = Label::new(Some(&display_title));
        tab_label.set_xalign(0.0); // left align inside header
        tab_label.set_margin_start(2);
        tab_label.set_margin_end(4);
        tab_label.set_tooltip_text(path.as_ref().and_then(|p| p.to_str()));
        header.append(&tab_label);

        // compact close button using a symbolic icon
        let close_btn = Button::builder().halign(gtk4::Align::Center).valign(gtk4::Align::Center).build();
        let close_img = Image::from_icon_name("window-close-symbolic");
        close_img.set_pixel_size(12);
        close_btn.set_child(Some(&close_img));
        close_btn.set_tooltip_text(Some("Close tab"));
        header.append(&close_btn);

        // initial text
        if let Some(t) = initial_text.clone() {
            main_buffer.set_text(&t);
        }

        let current_file = Rc::new(RefCell::new(path.clone()));
        let dirty = Rc::new(RefCell::new(false));
        let tag_cache = Rc::new(RefCell::new(HashMap::new()));

        // rope backing (initialize from initial_text or buffer)
        let rope = Rc::new(RefCell::new(match &initial_text {
            Some(s) => Rope::from_str(s.as_str()),
            None => Rope::from_str(""),
        }));

        // generation id for cancellation
        let highlight_gen = Arc::new(AtomicU64::new(0));

        // channel from workers -> main context
        let (tx, rx) = glib::MainContext::channel::<(u64, String)>(glib::Priority::default());

        // create the editor instance first (placeholder)
        let editor = Rc::new(Self {
            main_view,
            main_buffer,
            gutter_label,
            content_row,
            header,
            tab_label: tab_label.clone(),
            close_button: close_btn.clone(),
            current_file,
            dirty,
            tag_cache,
            ss: ss.clone(),
            theme: theme.clone(),
            rope: rope.clone(),
            highlight_gen: highlight_gen.clone(),
            highlight_sender: tx.clone(),
        });

        // Attach receiver to main context to receive background-prepared text for highlighting
        {
            let buffer_cl = editor.main_buffer.clone();
            let tag_cache_cl = editor.tag_cache.clone();
            let ss_cl = ss.clone();
            let theme_cl = theme.clone();
            let gen_cl = highlight_gen.clone();

            rx.attach(None, move |(job_gen, text)| {
                // Only apply if generation still matches (cancellation)
                let cur = gen_cl.load(Ordering::Relaxed);
                if job_gen != cur {
                    // stale job — ignore
                    return glib::Continue(false);
                }

                // Apply highlight on the main thread using your existing helper
                highlight::highlight_with_syntect(&buffer_cl, &text, &*tag_cache_cl, &ss_cl, &theme_cl);

                glib::Continue(false)
            });
        }

        // Debounced/coalesced buffer change handling
        {
            let sender = editor.highlight_sender.clone();
            let gen = editor.highlight_gen.clone();
            let rope_inner = editor.rope.clone();
            let buffer_cl = editor.main_buffer.clone();
            let dirty_cl = editor.dirty.clone();

            // threshold for skipping automatic highlighting of extremely large files
            const HIGHLIGHT_MAX_CHARS: usize = 200_000;

            // When the buffer changes, increment generation and schedule idle job to prepare text and spawn background worker.
            editor.main_buffer.connect_changed(move |_| {
                // Mark dirty
                *dirty_cl.borrow_mut() = true;

                // bump generation to cancel outstanding workers
                let job_gen = gen.fetch_add(1, Ordering::Relaxed) + 1;

                // schedule idle to coalesce rapid changes and capture buffer text on main thread in a non-blocking slot
                let rope_ref = rope_inner.clone();
                glib::idle_add_local(clone!(@strong buffer_cl, @strong sender => @default-return glib::Continue(false), move || {
                    // Read whole buffer text (this must run on main context)
                    let s_iter = buffer_cl.start_iter();
                    let e_iter = buffer_cl.end_iter();
                    let text = buffer_cl.text(&s_iter, &e_iter, false);

                    // Update the rope: remove all chars and insert new text
                    {
                        let mut r = rope_ref.borrow_mut();
                        let len = r.len_chars();
                        if len > 0 {
                            r.remove(0..len);
                        }
                        r.insert(0, &text);
                    }

                    // Skip heavy work for very large files
                    if text.chars().count() > HIGHLIGHT_MAX_CHARS {
                        // We intentionally skip scheduling a worker; main thread will not highlight automatically
                        return glib::Continue(false);
                    }

                    // Clone the sender inside this idle closure so we can move it into the spawned thread.
                    let sender_for_thread = sender.clone();
                    let text_owned = text.to_string();
                    std::thread::spawn(move || {
                        // (heavy precomputation could be done here off-main-thread)
                        // Send the prepared text + generation back to the main thread
                        let _ = sender_for_thread.send((job_gen, text_owned));
                    });

                    glib::Continue(false)
                }));
            });
        }

        // Mark buffer dirty and update label when content changes (existing behavior)
        {
            let dirty_clone = editor.dirty.clone();
            let tab_label_clone = editor.tab_label.clone();
            let current_file_clone = editor.current_file.clone();
            editor.main_buffer.connect_changed(move |_| {
                *dirty_clone.borrow_mut() = true;
                // update label to show dirty prefix (asterisk)
                let base = current_file_clone
                    .borrow()
                    .as_ref()
                    .and_then(|p| p.file_name().and_then(|s| s.to_str()))
                    .unwrap_or("Untitled")
                    .to_string();
                tab_label_clone.set_text(&format!("*{}", base));
            });
        }

        editor
    }

    /// Update gutter numbers, schedule highlighting (deferred), update status immediately.
    /// status_info_label will be updated with path & size if available.
    pub fn update(&self, status_label: &Label, status_info_label: &Label) {
        let s = self.main_buffer.start_iter();
        let e = self.main_buffer.end_iter();
        let content = self.main_buffer.text(&s, &e, false);

        // gutter numbers - immediate
        let line_count = if content.is_empty() { 1 } else { content.lines().count() };
        let width = line_count.to_string().len();
        let mut numbers = String::with_capacity(line_count * (width + 1));
        for i in 1..=line_count {
            numbers.push_str(&format!("{:>width$}\n", i, width = width));
        }
        self.gutter_label.set_text(&numbers);

        // status - immediate
        let ins = self.main_buffer.get_insert();
        let it = self.main_buffer.iter_at_mark(&ins);
        let line = it.line();
        let col = it.line_offset();
        status_label.set_text(&format!("Ln {}, Col {}", line + 1, col + 1));

        // update status info (path + size)
        let info = if let Some(p) = self.current_file.borrow().as_ref() {
            if let Ok(meta) = std::fs::metadata(p) {
                let size = meta.len();
                format!("{} — {} bytes", p.display(), size)
            } else {
                format!("{}", p.display())
            }
        } else {
            let len = content.len();
            format!("Untitled — {} bytes", len)
        };
        status_info_label.set_text(&info);

        // decide whether to highlight (based on extension or default true)
        let do_highlight = self
            .current_file
            .borrow()
            .as_ref()
            .and_then(|p| p.extension().and_then(|s| s.to_str()))
            .map(|ext| ext == "rs")
            .unwrap_or(true);

        // If file is large, avoid automatic highlighting here (background pipeline also respects threshold)
        const HIGHLIGHT_MAX_CHARS: usize = 200_000;
        if content.chars().count() > HIGHLIGHT_MAX_CHARS {
            // optionally: indicate in status that highlighting is disabled for large files
            // status_label.set_text("Highlighting disabled for large file");
            return;
        }

        if do_highlight {
            // Use idle_add_local to avoid re-entrancy and keep UI responsive;
            // the background pipeline is already set up; we can also perform a final highlight here as fallback.
            let buffer = self.main_buffer.clone();
            let content_clone = content.to_string();
            let tag_cache = self.tag_cache.clone();
            let ss = self.ss.clone();
            let theme = self.theme.clone();

            glib::idle_add_local(clone!(@strong buffer, @strong tag_cache, @strong ss, @strong theme => @default-return glib::Continue(false), move || {
                highlight::highlight_with_syntect(&buffer, &content_clone, &*tag_cache, &ss, &theme);
                glib::Continue(false)
            }));
        }
    }

    /// Toggle soft wrap for this editor
    pub fn toggle_wrap(&self) {
        let current = self.main_view.wrap_mode();
        if current == WrapMode::None {
            self.main_view.set_wrap_mode(WrapMode::Word);
        } else {
            self.main_view.set_wrap_mode(WrapMode::None);
        }
    }

    /// Convenience: return the content row widget to insert into Notebook
    pub fn content_row(&self) -> GtkBox {
        self.content_row.clone()
    }

    /// Get the current buffer text
    pub fn get_text(&self) -> String {
        let s = self.main_buffer.start_iter();
        let e = self.main_buffer.end_iter();
        self.main_buffer.text(&s, &e, false).to_string()
    }

    /// Set the buffer text
    #[allow(dead_code)]
    pub fn set_text(&self, text: &str) {
        self.main_buffer.set_text(text);
        // keep rope consistent
        {
            let mut r = self.rope.borrow_mut();
            let len = r.len_chars();
            if len > 0 {
                r.remove(0..len);
            }
            r.insert(0, text);
        }
    }

    /// Save buffer to given path
    pub fn save_to_path(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let content = self.get_text();
        std::fs::write(path, content.as_str())?;
        *self.current_file.borrow_mut() = Some(path.clone());
        *self.dirty.borrow_mut() = false;

        // Update tab label to remove dirty marker and show filename
        let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string();
        self.tab_label.set_text(&base);

        Ok(())
    }
}