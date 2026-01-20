use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, ScrolledWindow, TextBuffer, TextView, WrapMode, PolicyType, Button};
use gtk4::TextTag;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use syntect::parsing::SyntaxSet;
use syntect::highlighting::Theme;

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
    pub header: GtkBox, // tab header (label + close)
    pub close_button: Button, // the "x" button for closing the tab
    pub current_file: Rc<RefCell<Option<PathBuf>>>,
    pub dirty: Rc<RefCell<bool>>,
    pub tag_cache: Rc<RefCell<HashMap<String, TextTag>>>,
    // keep syntect refs to avoid reloading repeatedly
    ss: Rc<SyntaxSet>,
    theme: Rc<Theme>,
}

impl Editor {
    /// Create a new Editor with explicit title, optional initial text and path.
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

        // header for tab: label + close
        let header = GtkBox::new(gtk4::Orientation::Horizontal, 4);
        header.set_hexpand(false);
        header.set_vexpand(false);
        let tab_label = Label::new(Some(title));
        header.append(&tab_label);
        let close_btn = Button::with_label("✕");
        close_btn.set_tooltip_text(Some("Close tab"));
        header.append(&close_btn);

        // initial text
        if let Some(t) = initial_text {
            main_buffer.set_text(&t);
        }

        let current_file = Rc::new(RefCell::new(path.clone()));
        let dirty = Rc::new(RefCell::new(false));
        let tag_cache = Rc::new(RefCell::new(HashMap::new()));

        Rc::new(Self {
            main_view,
            main_buffer,
            gutter_label,
            content_row,
            header,
            close_button: close_btn,
            current_file,
            dirty,
            tag_cache,
            ss,
            theme,
        })
    }

    /// Update gutter numbers, optionally run highlighting (safe).
    /// `status_label` is updated with cursor position text.
    pub fn update(&self, status_label: &Label) {
        let s = self.main_buffer.start_iter();
        let e = self.main_buffer.end_iter();
        let content = self.main_buffer.text(&s, &e, false);

        // gutter numbers
        let line_count = if content.is_empty() { 1 } else { content.lines().count() };
        let width = line_count.to_string().len();
        let mut numbers = String::with_capacity(line_count * (width + 1));
        for i in 1..=line_count {
            numbers.push_str(&format!("{:>width$}\n", i, width = width));
        }
        self.gutter_label.set_text(&numbers);

        // decide whether to highlight (based on extension or default true)
        let do_highlight = self
            .current_file
            .borrow()
            .as_ref()
            .and_then(|p| p.extension().and_then(|s| s.to_str()))
            .map(|ext| ext == "rs")
            .unwrap_or(true);

        if do_highlight {
            // Use highlight module (clamped and safe)
            highlight::highlight_with_syntect(
                &self.main_buffer,
                &content,
                &*self.tag_cache,
                &self.ss,
                &self.theme,
            );
        }

        // status
        let ins = self.main_buffer.get_insert();
        let it = self.main_buffer.iter_at_mark(&ins);
        let line = it.line();
        let col = it.line_offset();
        status_label.set_text(&format!("Ln {}, Col {}", line + 1, col + 1));
    }

    /// Convenience: return the content row widget to insert into Notebook
    pub fn content_row(&self) -> GtkBox {
        self.content_row.clone()
    }

    /// Convenience: return the header widget for the tab label
    pub fn header(&self) -> GtkBox {
        self.header.clone()
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
    }

    /// Save buffer to given path
    pub fn save_to_path(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let content = self.get_text();
        std::fs::write(path, content.as_str())?;
        *self.current_file.borrow_mut() = Some(path.clone());
        *self.dirty.borrow_mut() = false;
        Ok(())
    }
}