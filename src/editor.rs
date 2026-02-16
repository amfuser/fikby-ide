use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, ScrolledWindow, TextBuffer, TextView, WrapMode, PolicyType, Button};
use gtk4::{gdk, EventControllerKey, Inhibit};
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
    pub header: GtkBox,
    pub tab_label: Label,
    pub close_button: Button,
    pub current_file: Rc<RefCell<Option<PathBuf>>>,
    pub dirty: Rc<RefCell<bool>>,
    pub tag_cache: Rc<RefCell<HashMap<String, TextTag>>>,
    ss: Rc<SyntaxSet>,
    theme: Rc<RefCell<Rc<Theme>>>,
    rope: Rc<RefCell<Rope>>,
    highlight_gen: Arc<AtomicU64>,
    highlight_sender: glib::Sender<(u64, String)>,
}

impl Editor {
    pub fn new(title: &str, initial_text: Option<String>, path: Option<PathBuf>, ss: Rc<SyntaxSet>, theme: Rc<Theme>) -> Rc<Self> {
        // main TextView
        let main_view = TextView::new();
        main_view.set_wrap_mode(WrapMode::None);
        main_view.set_hexpand(true);
        main_view.set_vexpand(true);
        main_view.set_monospace(true);
        main_view.style_context().add_class("editor-view");
        main_view.set_accepts_tab(false); // We'll handle Tab ourselves
        
        main_view.set_pixels_above_lines(0);
        main_view.set_pixels_below_lines(0);
        main_view.set_pixels_inside_wrap(0);
        main_view.set_top_margin(0);
        main_view.set_bottom_margin(0);
        main_view.set_left_margin(4);
        main_view.set_right_margin(4);
        
        let main_buffer = main_view.buffer();
        
        // Enable undo/redo
        main_buffer.set_enable_undo(true);

        let gutter_label = Label::new(None);
        gutter_label.style_context().add_class("gutter");
        gutter_label.set_halign(Align::End);
        gutter_label.set_valign(Align::Start);
        gutter_label.set_yalign(0.0);
        gutter_label.set_xalign(1.0);

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
            .min_content_width(60)
            .build();

        let vadj: gtk4::Adjustment = main_scrolled.vadjustment();
        gutter_scrolled.set_vadjustment(Some(&vadj));

        let content_row = GtkBox::new(gtk4::Orientation::Horizontal, 0);
        content_row.append(&gutter_scrolled);
        content_row.append(&main_scrolled);
        content_row.set_hexpand(true);
        content_row.set_vexpand(true);

        // header for tab: file icon + label + close
        let header = GtkBox::new(gtk4::Orientation::Horizontal, 6);
        header.set_hexpand(false);
        header.set_vexpand(false);
        header.set_margin_start(4);
        header.set_margin_end(4);
        header.set_margin_top(4);
        header.set_margin_bottom(4);

        // file icon (try to use a source/text icon; theme-dependent)
        let file_icon = Image::from_icon_name("text-x-generic");
        file_icon.set_pixel_size(16);
        file_icon.set_margin_end(2);
        header.append(&file_icon);

        // label with filename
        let display_title = path
            .as_ref()
            .and_then(|p: &PathBuf| p.file_name())
            .and_then(|os| os.to_str())
            .unwrap_or(title)
            .to_string();

        let tab_label = Label::new(Some(&display_title));
        tab_label.set_xalign(0.0);
        tab_label.set_margin_start(2);
        tab_label.set_margin_end(4);
        tab_label.set_width_request(80); // Add minimum width
        tab_label.set_ellipsize(gtk4::pango::EllipsizeMode::End); // Ellipsize long names
        tab_label.set_tooltip_text(path.as_ref().and_then(|p| p.to_str()));
        header.append(&tab_label);

        // compact close button using a symbolic icon
        let close_btn = Button::builder()
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        let close_img = Image::from_icon_name("window-close-symbolic");
        close_img.set_pixel_size(12);
        close_btn.set_child(Some(&close_img));
        close_btn.set_tooltip_text(Some("Close tab"));
        close_btn.set_has_frame(false); // Remove button frame for cleaner look
        header.append(&close_btn);

        if let Some(t) = initial_text.clone() {
            main_buffer.set_text(&t);
        }

        let current_file = Rc::new(RefCell::new(path.clone()));
        let dirty = Rc::new(RefCell::new(false));
        let tag_cache = Rc::new(RefCell::new(HashMap::new()));

        let rope = Rc::new(RefCell::new(match &initial_text {
            Some(s) => Rope::from_str(s.as_str()),
            None => Rope::from_str(""),
        }));

        let highlight_gen = Arc::new(AtomicU64::new(0));
        let (tx, rx) = glib::MainContext::channel::<(u64, String)>(glib::Priority::default());

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
            theme: Rc::new(RefCell::new(theme.clone())),
            rope: rope.clone(),
            highlight_gen: highlight_gen.clone(),
            highlight_sender: tx.clone(),
        });

        // Set up keyboard event controller for Tab, Enter, and auto-dedent handling
        {
            let key_controller = EventControllerKey::new();
            let buffer_clone = editor.main_buffer.clone();
            
            key_controller.connect_key_pressed(move |_, keyval, _keycode, modifier| {
                let shift_pressed = modifier.contains(gdk::ModifierType::SHIFT_MASK);
                let ctrl_pressed = modifier.contains(gdk::ModifierType::CONTROL_MASK);
                
                // Don't interfere with Ctrl shortcuts
                if ctrl_pressed {
                    return Inhibit(false);
                }
                
                match keyval {
                    gdk::Key::Tab => {
                        if shift_pressed {
                            // Shift+Tab: Decrease indent
                            Self::decrease_indent(&buffer_clone);
                        } else {
                            // Tab: Increase indent
                            Self::increase_indent(&buffer_clone);
                        }
                        Inhibit(true)
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter => {
                        // Auto-indent on Enter
                        Self::auto_indent_newline(&buffer_clone);
                        Inhibit(true)
                    }
                    gdk::Key::braceright => {
                        // } - auto-dedent
                        Self::handle_closing_bracket(&buffer_clone, '}');
                        Inhibit(true)
                    }
                    gdk::Key::bracketright => {
                        // ] - auto-dedent
                        Self::handle_closing_bracket(&buffer_clone, ']');
                        Inhibit(true)
                    }
                    gdk::Key::parenright => {
                        // ) - auto-dedent
                        Self::handle_closing_bracket(&buffer_clone, ')');
                        Inhibit(true)
                    }
                    _ => Inhibit(false)
                }
            });
            
            editor.main_view.add_controller(key_controller);
        }

        // Attach receiver for highlighting
        {
            let buffer_cl = editor.main_buffer.clone();
            let tag_cache_cl = editor.tag_cache.clone();
            let ss_cl = ss.clone();
            let theme_cl = editor.theme.clone();
            let gen_cl = highlight_gen.clone();

            rx.attach(None, move |(job_gen, text)| {
                let cur = gen_cl.load(Ordering::Relaxed);
                if job_gen != cur {
                    return glib::Continue(false);
                }
                let current_theme = theme_cl.borrow().clone();
                highlight::highlight_with_syntect(&buffer_cl, &text, &*tag_cache_cl, &ss_cl, &current_theme);
                glib::Continue(false)
            });
        }

        // Buffer change handling
        {
            let sender = editor.highlight_sender.clone();
            let gen = editor.highlight_gen.clone();
            let rope_inner = editor.rope.clone();
            let buffer_cl = editor.main_buffer.clone();
            let dirty_cl = editor.dirty.clone();

            const HIGHLIGHT_MAX_CHARS: usize = 200_000;

            editor.main_buffer.connect_changed(move |_| {
                *dirty_cl.borrow_mut() = true;
                let job_gen = gen.fetch_add(1, Ordering::Relaxed) + 1;

                let rope_ref = rope_inner.clone();
                glib::idle_add_local(clone!(@strong buffer_cl, @strong sender => @default-return glib::Continue(false), move || {
                    let s_iter = buffer_cl.start_iter();
                    let e_iter = buffer_cl.end_iter();
                    let text = buffer_cl.text(&s_iter, &e_iter, false);

                    {
                        let mut r = rope_ref.borrow_mut();
                        let len = r.len_chars();
                        if len > 0 {
                            r.remove(0..len);
                        }
                        r.insert(0, &text);
                    }

                    if text.chars().count() > HIGHLIGHT_MAX_CHARS {
                        return glib::Continue(false);
                    }

                    let sender_for_thread = sender.clone();
                    let text_owned = text.to_string();
                    std::thread::spawn(move || {
                        let _ = sender_for_thread.send((job_gen, text_owned));
                    });

                    glib::Continue(false)
                }));
            });
        }

        // Mark dirty
        {
            let dirty_clone = editor.dirty.clone();
            let tab_label_clone = editor.tab_label.clone();
            let current_file_clone = editor.current_file.clone();
            editor.main_buffer.connect_changed(move |_| {
                *dirty_clone.borrow_mut() = true;
                let base = current_file_clone
                    .borrow()
                    .as_ref()
                    .and_then(|p| p.file_name().and_then(|s| s.to_str()))
                    .unwrap_or("Untitled")
                    .to_string();
                tab_label_clone.set_text(&format!("*{}", base));
            });
        }

        // Update gutter
        {
            let gutter_label_clone = editor.gutter_label.clone();
            let buffer_clone = editor.main_buffer.clone();
            
            editor.main_buffer.connect_changed(move |_| {
                let s = buffer_clone.start_iter();
                let e = buffer_clone.end_iter();
                let content = buffer_clone.text(&s, &e, false);

                let line_count = if content.is_empty() {
                    1
                } else {
                    let text_str = content.as_str();
                    let newline_count = text_str.chars().filter(|&c| c == '\n').count();
                    if text_str.ends_with('\n') {
                        newline_count
                    } else {
                        newline_count + 1
                    }
                };
                
                let width = line_count.to_string().len();
                let mut numbers = String::with_capacity(line_count * (width + 2));
                
                for i in 1..=line_count {
                    if i > 1 {
                        numbers.push('\n');
                    }
                    numbers.push_str(&format!("{:>width$}", i, width = width));
                }
                
                gutter_label_clone.set_text(&numbers);
            });
        }

        // Scroll to cursor
        {
            let view_clone = editor.main_view.clone();
            let buffer_clone = editor.main_buffer.clone();
            
            editor.main_buffer.connect_changed(move |_| {
                glib::idle_add_local(clone!(@strong view_clone, @strong buffer_clone => @default-return glib::Continue(false), move || {
                    let insert_mark = buffer_clone.get_insert();
                    view_clone.scroll_to_mark(&insert_mark, 0.0, false, 0.0, 0.0);
                    glib::Continue(false)
                }));
            });
        }

        editor
    }

    // Tab = 4 spaces
    const TAB_WIDTH: usize = 4;

    fn increase_indent(buffer: &TextBuffer) {
        let (has_selection, start, end) = buffer.selection_bounds()
            .map(|(s, e)| (true, s, e))
            .unwrap_or_else(|| {
                let cursor = buffer.get_insert();
                let iter = buffer.iter_at_mark(&cursor);
                (false, iter.clone(), iter)
            });

        if has_selection {
            // Indent all selected lines
            let start_line = start.line();
            let end_line = end.line();
            
            buffer.begin_user_action();
            for line_num in start_line..=end_line {
                let mut line_start = buffer.iter_at_line(line_num).unwrap_or_else(|| buffer.start_iter());
                buffer.insert(&mut line_start, &" ".repeat(Self::TAB_WIDTH));
            }
            buffer.end_user_action();
        } else {
            // Insert spaces at cursor
            let spaces = " ".repeat(Self::TAB_WIDTH);
            buffer.insert_at_cursor(&spaces);
        }
    }

    fn decrease_indent(buffer: &TextBuffer) {
        let (start, end) = buffer.selection_bounds()
            .unwrap_or_else(|| {
                let cursor = buffer.get_insert();
                let iter = buffer.iter_at_mark(&cursor);
                (iter.clone(), iter)
            });

        let start_line = start.line();
        let end_line = end.line();
        
        buffer.begin_user_action();
        for line_num in start_line..=end_line {
            if let Some(mut line_start) = buffer.iter_at_line(line_num) {
                let mut line_end = line_start.clone();
                line_end.forward_to_line_end();
                
                let line_text = buffer.text(&line_start, &line_end, false);
                let mut spaces_to_remove = 0;
                
                for ch in line_text.chars().take(Self::TAB_WIDTH) {
                    if ch == ' ' {
                        spaces_to_remove += 1;
                    } else {
                        break;
                    }
                }
                
                if spaces_to_remove > 0 {
                    let mut end_of_spaces = line_start.clone();
                    end_of_spaces.forward_chars(spaces_to_remove);
                    buffer.delete(&mut line_start, &mut end_of_spaces);
                }
            }
        }
        buffer.end_user_action();
    }

    fn auto_indent_newline(buffer: &TextBuffer) {
        let cursor = buffer.get_insert();
        let iter = buffer.iter_at_mark(&cursor);
        let line = iter.line();
        
        // Get current line start and cursor position
        if let Some(line_start) = buffer.iter_at_line(line) {
            let cursor_pos = iter.clone();
            
            let line_text = buffer.text(&line_start, &cursor_pos, false);
            
            // Count leading spaces
            let leading_spaces: String = line_text
                .chars()
                .take_while(|&c| c == ' ')
                .collect();
            
            // Check if we need to add extra indentation
            let trimmed = line_text.trim_end();
            let extra_indent = if trimmed.ends_with('{') 
                || trimmed.ends_with('[') 
                || trimmed.ends_with('(')
                || trimmed.ends_with(':') // For Python, YAML, etc.
            {
                " ".repeat(Self::TAB_WIDTH)
            } else {
                String::new()
            };
            
            // Insert newline + indentation
            let new_line_text = format!("\n{}{}", leading_spaces, extra_indent);
            buffer.insert_at_cursor(&new_line_text);
        }
    }
    
    fn handle_closing_bracket(buffer: &TextBuffer, bracket: char) {
        // Get cursor position
        let cursor = buffer.get_insert();
        let iter = buffer.iter_at_mark(&cursor);
        let line = iter.line();
        
        if let Some(mut line_start) = buffer.iter_at_line(line) {
            let cursor_pos = iter.clone();
            
            // Get text from line start to cursor
            let text_before_cursor = buffer.text(&line_start, &cursor_pos, false);
            
            // Check if line only has whitespace before cursor
            if text_before_cursor.trim().is_empty() {
                // Count leading spaces
                let leading_spaces = text_before_cursor.len();
                
                // If we have at least TAB_WIDTH spaces, remove them
                if leading_spaces >= Self::TAB_WIDTH {
                    let mut delete_end = line_start.clone();
                    delete_end.forward_chars(Self::TAB_WIDTH as i32);
                    buffer.delete(&mut line_start, &mut delete_end);
                }
            }
            
            // Insert the bracket
            buffer.insert_at_cursor(&bracket.to_string());
        }
    }

    pub fn undo(&self) {
        if self.main_buffer.can_undo() {
            self.main_buffer.undo();
        }
    }

    pub fn redo(&self) {
        if self.main_buffer.can_redo() {
            self.main_buffer.redo();
        }
    }

    pub fn cut(&self) {
        if let Some(display) = gdk::Display::default() {
            let clipboard = display.clipboard();
            self.main_buffer.cut_clipboard(&clipboard, true);
        }
    }

    pub fn paste(&self) {
        if let Some(display) = gdk::Display::default() {
            let clipboard = display.clipboard();
            self.main_buffer.paste_clipboard(&clipboard, None, true);
        }
    }

    pub fn find_text(&self, search_text: &str, case_sensitive: bool) -> bool {
        let flags = if case_sensitive {
            gtk4::TextSearchFlags::empty()
        } else {
            gtk4::TextSearchFlags::CASE_INSENSITIVE
        };

        let cursor = self.main_buffer.get_insert();
        let mut start = self.main_buffer.iter_at_mark(&cursor);
        
        // Start searching from cursor position
        if let Some((mut match_start, match_end)) = start.forward_search(search_text, flags, None) {
            self.main_buffer.select_range(&match_start, &match_end);
            self.main_view.scroll_to_iter(&mut match_start, 0.0, false, 0.0, 0.0);
            true
        } else {
            // Wrap around to beginning
            start = self.main_buffer.start_iter();
            if let Some((mut match_start, match_end)) = start.forward_search(search_text, flags, None) {
                self.main_buffer.select_range(&match_start, &match_end);
                self.main_view.scroll_to_iter(&mut match_start, 0.0, false, 0.0, 0.0);
                true
            } else {
                false
            }
        }
    }

    pub fn replace_current(&self, search_text: &str, replace_text: &str, case_sensitive: bool) -> bool {
        if let Some((mut start, mut end)) = self.main_buffer.selection_bounds() {
            let selected = self.main_buffer.text(&start, &end, false);
            let matches = if case_sensitive {
                selected == search_text
            } else {
                selected.to_lowercase() == search_text.to_lowercase()
            };
            
            if matches {
                self.main_buffer.delete(&mut start, &mut end);
                self.main_buffer.insert(&mut start, replace_text);
                return true;
            }
        }
        false
    }

    pub fn replace_all(&self, search_text: &str, replace_text: &str, case_sensitive: bool) -> i32 {
        let flags = if case_sensitive {
            gtk4::TextSearchFlags::empty()
        } else {
            gtk4::TextSearchFlags::CASE_INSENSITIVE
        };

        let mut count = 0;
        self.main_buffer.begin_user_action();
        
        let mut search_start = self.main_buffer.start_iter();
        while let Some((mut match_start, mut match_end)) = search_start.forward_search(search_text, flags, None) {
            self.main_buffer.delete(&mut match_start, &mut match_end);
            self.main_buffer.insert(&mut match_start, replace_text);
            search_start = match_start;
            count += 1;
        }
        
        self.main_buffer.end_user_action();
        count
    }

    pub fn update(&self, status_label: &Label, status_info_label: &Label) {
        let s = self.main_buffer.start_iter();
        let e = self.main_buffer.end_iter();
        let content = self.main_buffer.text(&s, &e, false);

        let line_count = if content.is_empty() {
            1
        } else {
            let text_str = content.as_str();
            let newline_count = text_str.chars().filter(|&c| c == '\n').count();
            if text_str.ends_with('\n') {
                newline_count
            } else {
                newline_count + 1
            }
        };
        
        let width = line_count.to_string().len();
        let mut numbers = String::with_capacity(line_count * (width + 2));
        
        for i in 1..=line_count {
            if i > 1 {
                numbers.push('\n');
            }
            numbers.push_str(&format!("{:>width$}", i, width = width));
        }
        
        self.gutter_label.set_text(&numbers);

        let ins = self.main_buffer.get_insert();
        let it = self.main_buffer.iter_at_mark(&ins);
        let line = it.line();
        let col = it.line_offset();
        status_label.set_text(&format!("Ln {}, Col {}", line + 1, col + 1));

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

        let do_highlight = self
            .current_file
            .borrow()
            .as_ref()
            .and_then(|p| p.extension().and_then(|s| s.to_str()))
            .map(|ext| matches!(ext, "rs" | "py" | "js" | "ts" | "json" | "toml" | "md" | "html" | "css" | "xml"))
            .unwrap_or(true);

        const HIGHLIGHT_MAX_CHARS: usize = 200_000;
        if content.chars().count() > HIGHLIGHT_MAX_CHARS {
            return;
        }

        if do_highlight {
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

    pub fn toggle_wrap(&self) {
        let current = self.main_view.wrap_mode();
        if current == WrapMode::None {
            self.main_view.set_wrap_mode(WrapMode::Word);
        } else {
            self.main_view.set_wrap_mode(WrapMode::None);
        }
    }

    pub fn content_row(&self) -> GtkBox {
        self.content_row.clone()
    }

    pub fn get_text(&self) -> String {
        let s = self.main_buffer.start_iter();
        let e = self.main_buffer.end_iter();
        self.main_buffer.text(&s, &e, false).to_string()
    }

    #[allow(dead_code)]
    pub fn set_text(&self, text: &str) {
        self.main_buffer.set_text(text);
        {
            let mut r = self.rope.borrow_mut();
            let len = r.len_chars();
            if len > 0 {
                r.remove(0..len);
            }
            r.insert(0, text);
        }
    }

    pub fn save_to_path(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let content = self.get_text();
        std::fs::write(path, content.as_str())?;
        *self.current_file.borrow_mut() = Some(path.clone());
        *self.dirty.borrow_mut() = false;

        let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string();
        self.tab_label.set_text(&base);

        Ok(())
    }
    
    /// Update the theme and re-highlight the editor
    pub fn set_theme(&self, new_theme: Rc<Theme>) {
        *self.theme.borrow_mut() = new_theme;
        // Trigger re-highlighting by incrementing the generation counter and sending current text
        let gen = self.highlight_gen.fetch_add(1, Ordering::Relaxed) + 1;
        let text = self.get_text();
        let _ = self.highlight_sender.send((gen, text));
    }
}