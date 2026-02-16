use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CheckButton, Dialog, Entry, Grid, Label, Orientation,
};
use std::rc::Rc;

use crate::editor::Editor;

pub struct FindReplaceDialog {
    dialog: Dialog,
    find_entry: Entry,
    _replace_entry: Entry,
    _case_sensitive: CheckButton,
    _editor: Rc<Editor>,
}

impl FindReplaceDialog {
    pub fn new(parent: &gtk4::Window, editor: Rc<Editor>) -> Self {
        let dialog = Dialog::builder()
            .title("Find and Replace")
            .transient_for(parent)
            .modal(true)
            .default_width(400)
            .default_height(200)
            .build();

        // Create the content area
        let content_area = dialog.content_area();
        content_area.set_margin_top(10);
        content_area.set_margin_bottom(10);
        content_area.set_margin_start(10);
        content_area.set_margin_end(10);

        // Create grid for layout
        let grid = Grid::builder()
            .column_spacing(10)
            .row_spacing(10)
            .build();

        // Find label and entry
        let find_label = Label::new(Some("Find:"));
        find_label.set_halign(gtk4::Align::End);
        let find_entry = Entry::new();
        find_entry.set_hexpand(true);

        // Replace label and entry
        let replace_label = Label::new(Some("Replace:"));
        replace_label.set_halign(gtk4::Align::End);
        let replace_entry = Entry::new();
        replace_entry.set_hexpand(true);

        // Case sensitive checkbox
        let case_sensitive = CheckButton::with_label("Case sensitive");

        // Add widgets to grid
        grid.attach(&find_label, 0, 0, 1, 1);
        grid.attach(&find_entry, 1, 0, 1, 1);
        grid.attach(&replace_label, 0, 1, 1, 1);
        grid.attach(&replace_entry, 1, 1, 1, 1);
        grid.attach(&case_sensitive, 1, 2, 1, 1);

        content_area.append(&grid);

        // Create button box
        let button_box = GtkBox::new(Orientation::Horizontal, 5);
        button_box.set_margin_top(10);
        button_box.set_halign(gtk4::Align::End);

        // Create buttons
        let find_next_btn = Button::with_label("Find Next");
        let replace_btn = Button::with_label("Replace");
        let replace_all_btn = Button::with_label("Replace All");
        let close_btn = Button::with_label("Close");

        button_box.append(&find_next_btn);
        button_box.append(&replace_btn);
        button_box.append(&replace_all_btn);
        button_box.append(&close_btn);

        content_area.append(&button_box);

        let find_replace = Self {
            dialog,
            find_entry: find_entry.clone(),
            _replace_entry: replace_entry.clone(),
            _case_sensitive: case_sensitive.clone(),
            _editor: editor.clone(),
        };

        // Connect Find Next button
        {
            let editor_clone = editor.clone();
            let find_entry_clone = find_entry.clone();
            let case_sensitive_clone = case_sensitive.clone();
            let dialog_clone = find_replace.dialog.clone();

            find_next_btn.connect_clicked(move |_| {
                let search_text = find_entry_clone.text();
                if search_text.is_empty() {
                    return;
                }

                let case_sensitive = case_sensitive_clone.is_active();
                let found = editor_clone.find_text(&search_text, case_sensitive);

                if !found {
                    // Show info dialog that text not found
                    let info_dialog = Dialog::builder()
                        .title("Not Found")
                        .transient_for(&dialog_clone)
                        .modal(true)
                        .build();

                    let content = info_dialog.content_area();
                    let label = Label::new(Some(&format!("'{}' not found", search_text)));
                    label.set_margin_top(10);
                    label.set_margin_bottom(10);
                    label.set_margin_start(10);
                    label.set_margin_end(10);
                    content.append(&label);

                    let ok_btn = Button::with_label("OK");
                    ok_btn.connect_clicked({
                        let dialog = info_dialog.clone();
                        move |_| dialog.close()
                    });
                    content.append(&ok_btn);

                    info_dialog.show();
                }
            });
        }

        // Connect Replace button
        {
            let editor_clone = editor.clone();
            let find_entry_clone = find_entry.clone();
            let replace_entry_clone = replace_entry.clone();
            let case_sensitive_clone = case_sensitive.clone();

            replace_btn.connect_clicked(move |_| {
                let search_text = find_entry_clone.text();
                let replace_text = replace_entry_clone.text();
                if search_text.is_empty() {
                    return;
                }

                let case_sensitive = case_sensitive_clone.is_active();
                let replaced = editor_clone.replace_current(&search_text, &replace_text, case_sensitive);

                // After replacing, find next occurrence
                if replaced {
                    editor_clone.find_text(&search_text, case_sensitive);
                }
            });
        }

        // Connect Replace All button
        {
            let editor_clone = editor.clone();
            let find_entry_clone = find_entry.clone();
            let replace_entry_clone = replace_entry.clone();
            let case_sensitive_clone = case_sensitive.clone();
            let dialog_clone = find_replace.dialog.clone();

            replace_all_btn.connect_clicked(move |_| {
                let search_text = find_entry_clone.text();
                let replace_text = replace_entry_clone.text();
                if search_text.is_empty() {
                    return;
                }

                let case_sensitive = case_sensitive_clone.is_active();
                let count = editor_clone.replace_all(&search_text, &replace_text, case_sensitive);

                // Show info dialog with count
                let info_dialog = Dialog::builder()
                    .title("Replace All")
                    .transient_for(&dialog_clone)
                    .modal(true)
                    .build();

                let content = info_dialog.content_area();
                let label = Label::new(Some(&format!("Replaced {} occurrence(s)", count)));
                label.set_margin_top(10);
                label.set_margin_bottom(10);
                label.set_margin_start(10);
                label.set_margin_end(10);
                content.append(&label);

                let ok_btn = Button::with_label("OK");
                ok_btn.connect_clicked({
                    let dialog = info_dialog.clone();
                    move |_| dialog.close()
                });
                content.append(&ok_btn);

                info_dialog.show();
            });
        }

        // Connect Close button
        {
            let dialog_clone = find_replace.dialog.clone();
            close_btn.connect_clicked(move |_| {
                dialog_clone.close();
            });
        }

        // Handle Enter key in find entry
        {
            let find_next_btn_clone = find_next_btn.clone();
            find_entry.connect_activate(move |_| {
                find_next_btn_clone.emit_clicked();
            });
        }

        find_replace
    }

    pub fn show(&self) {
        self.dialog.show();
    }

    pub fn set_find_text(&self, text: &str) {
        self.find_entry.set_text(text);
        self.find_entry.select_region(0, -1);
    }
}