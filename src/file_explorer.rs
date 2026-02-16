use gtk4::prelude::*;
use gtk4::{
    gio, glib, Box as GtkBox, CellRendererText, Label, Menu, MenuItem, Orientation,
    PopoverMenu, ScrolledWindow, TreeIter, TreePath, TreeStore, TreeView, TreeViewColumn,
    GestureClick, EventControllerKey,
};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::RefCell;

pub struct FileExplorer {
    pub widget: ScrolledWindow,
    tree_view: TreeView,
    tree_store: TreeStore,
    root_path: Option<PathBuf>,
}

// Column indices for the TreeStore
const COL_NAME: u32 = 0; // Display name
const COL_PATH: u32 = 1; // Full path
const COL_IS_DIR: u32 = 2; // Is directory (bool)
const COL_ICON: u32 = 3; // Icon name

impl FileExplorer {
    pub fn new() -> Rc<Self> {
        // Create TreeStore with columns: name, path, is_dir, icon_name
        let tree_store = TreeStore::new(&[
            glib::Type::STRING,  // Name
            glib::Type::STRING,  // Path
            glib::Type::BOOL,    // Is directory
            glib::Type::STRING,  // Icon name
        ]);

        let tree_view = TreeView::with_model(&tree_store);
        tree_view.set_headers_visible(false);
        tree_view.set_enable_tree_lines(true);

        // Icon column
        let icon_column = TreeViewColumn::new();
        let icon_renderer = gtk4::CellRendererPixbuf::new();
        icon_column.pack_start(&icon_renderer, false);
        icon_column.add_attribute(&icon_renderer, "icon-name", COL_ICON as i32);
        tree_view.append_column(&icon_column);

        // Name column
        let name_column = TreeViewColumn::new();
        let text_renderer = CellRendererText::new();
        name_column.pack_start(&text_renderer, true);
        name_column.add_attribute(&text_renderer, "text", COL_NAME as i32);
        tree_view.append_column(&name_column);

        let scrolled = ScrolledWindow::builder()
            .child(&tree_view)
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();

        let explorer = Rc::new(FileExplorer {
            widget: scrolled,
            tree_view,
            tree_store,
            root_path: None,
        });

        explorer
    }

    pub fn set_root_directory(&mut self, path: PathBuf) {
        self.root_path = Some(path.clone());
        self.tree_store.clear();
        self.populate_tree(&path, None);
    }

    fn populate_tree(&self, path: &Path, parent: Option<&TreeIter>) {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            
            // Sort: directories first, then files, alphabetically
            items.sort_by(|a, b| {
                let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
                
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in items {
                let file_name = entry.file_name();
                let file_path = entry.path();
                
                // Skip hidden files (starting with .)
                if let Some(name_str) = file_name.to_str() {
                    if name_str.starts_with('.') {
                        continue;
                    }
                }

                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let icon_name = if is_dir {
                    "folder-symbolic"
                } else {
                    "text-x-generic-symbolic"
                };

                let iter = if let Some(parent_iter) = parent {
                    self.tree_store.insert_with_values(
                        Some(parent_iter),
                        None,
                        &[
                            (COL_NAME, &file_name.to_string_lossy().to_string()),
                            (COL_PATH, &file_path.to_string_lossy().to_string()),
                            (COL_IS_DIR, &is_dir),
                            (COL_ICON, &icon_name),
                        ],
                    )
                } else {
                    self.tree_store.insert_with_values(
                        None,
                        None,
                        &[
                            (COL_NAME, &file_name.to_string_lossy().to_string()),
                            (COL_PATH, &file_path.to_string_lossy().to_string()),
                            (COL_IS_DIR, &is_dir),
                            (COL_ICON, &icon_name),
                        ],
                    )
                };

                // For directories, add a dummy child so the expander shows
                // We'll populate it when expanded
                if is_dir {
                    self.tree_store.insert_with_values(
                        Some(&iter),
                        None,
                        &[
                            (COL_NAME, &"Loading..."),
                            (COL_PATH, &""),
                            (COL_IS_DIR, &false),
                            (COL_ICON, &""),
                        ],
                    );
                }
            }
        }
    }

    pub fn connect_row_activated<F>(&self, callback: F)
    where
        F: Fn(PathBuf, bool) + 'static,
    {
        self.tree_view.connect_row_activated(move |tree_view, path, _column| {
            if let Some(model) = tree_view.model() {
                if let Some(iter) = model.iter(path) {
                    let file_path: String = model.value(&iter, COL_PATH as i32).get().unwrap();
                    let is_dir: bool = model.value(&iter, COL_IS_DIR as i32).get().unwrap();
                    
                    if !file_path.is_empty() {
                        callback(PathBuf::from(file_path), is_dir);
                    }
                }
            }
        });
    }

    pub fn connect_row_expanded<F>(&self, callback: F)
    where
        F: Fn(&TreeStore, &TreeIter, &TreePath) + 'static,
    {
        let tree_store = self.tree_store.clone();
        self.tree_view.connect_row_expanded(move |_tree_view, iter, path| {
            callback(&tree_store, iter, path);
        });
    }

    pub fn expand_directory(&self, iter: &TreeIter) {
        // Remove dummy child
        if let Some(child_iter) = self.tree_store.iter_children(Some(iter)) {
            self.tree_store.remove(&child_iter);
        }

        // Get the directory path
        let dir_path: String = self.tree_store.value(iter, COL_PATH as i32).get().unwrap();
        
        // Populate with actual contents
        self.populate_tree(&PathBuf::from(dir_path), Some(iter));
    }

    pub fn highlight_file(&self, file_path: &Path) {
        // Find and select the file in the tree
        if let Some(iter) = self.find_iter_for_path(file_path, None) {
            if let Some(path) = self.tree_store.path(&iter) {
                self.tree_view.selection().select_path(&path);
                self.tree_view.scroll_to_cell(Some(&path), None::<&TreeViewColumn>, false, 0.0, 0.0);
            }
        }
    }

    fn find_iter_for_path(&self, target_path: &Path, parent: Option<&TreeIter>) -> Option<TreeIter> {
        let iter = if let Some(parent_iter) = parent {
            self.tree_store.iter_children(Some(parent_iter))
        } else {
            self.tree_store.iter_first()
        };

        if let Some(mut current_iter) = iter {
            loop {
                let path: String = self.tree_store.value(&current_iter, COL_PATH as i32).get().ok()?;
                if PathBuf::from(&path) == target_path {
                    return Some(current_iter);
                }

                // Check children if it's a directory
                let is_dir: bool = self.tree_store.value(&current_iter, COL_IS_DIR as i32).get().unwrap_or(false);
                if is_dir {
                    if let Some(found) = self.find_iter_for_path(target_path, Some(&current_iter)) {
                        return Some(found);
                    }
                }

                if !self.tree_store.iter_next(&current_iter) {
                    break;
                }
            }
        }

        None
    }

    pub fn get_tree_view(&self) -> &TreeView {
        &self.tree_view
    }

    pub fn get_tree_store(&self) -> &TreeStore {
        &self.tree_store
    }

    pub fn refresh(&self) {
        if let Some(root) = &self.root_path {
            self.tree_store.clear();
            self.populate_tree(root, None);
        }
    }

    pub fn setup_context_menu(&self, app: &gtk4::Application) {
        let tree_view = self.tree_view.clone();
        let tree_store = self.tree_store.clone();
        
        // Right-click gesture
        let gesture = GestureClick::new();
        gesture.set_button(3); // Right mouse button
        
        gesture.connect_pressed(move |_gesture, _n_press, x, y| {
            // Get the path at the click position
            if let Some((Some(path), _, _, _)) = tree_view.path_at_pos(x as i32, y as i32) {
                // Select the item
                tree_view.selection().select_path(&path);
                
                if let Some(iter) = tree_store.iter(&path) {
                    let file_path: String = tree_store.value(&iter, COL_PATH as i32).get().unwrap_or_default();
                    let is_dir: bool = tree_store.value(&iter, COL_IS_DIR as i32).get().unwrap_or(false);
                    
                    if !file_path.is_empty() {
                        // Create context menu
                        let menu = gio::Menu::new();
                        
                        if is_dir {
                            menu.append(Some("New File"), Some("app.explorer-new-file"));
                            menu.append(Some("New Folder"), Some("app.explorer-new-folder"));
                            menu.append(Some("Delete Folder"), Some("app.explorer-delete"));
                            menu.append(Some("Rename"), Some("app.explorer-rename"));
                        } else {
                            menu.append(Some("Delete File"), Some("app.explorer-delete"));
                            menu.append(Some("Rename"), Some("app.explorer-rename"));
                        }
                        
                        let popover = PopoverMenu::from_model(Some(&menu));
                        popover.set_parent(&tree_view);
                        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                        popover.popup();
                    }
                }
            }
        });
        
        self.tree_view.add_controller(gesture);
    }

    pub fn create_file(&self, parent_dir: &Path, file_name: &str) -> std::io::Result<PathBuf> {
        let file_path = parent_dir.join(file_name);
        std::fs::File::create(&file_path)?;
        self.refresh();
        Ok(file_path)
    }

    pub fn create_directory(&self, parent_dir: &Path, dir_name: &str) -> std::io::Result<PathBuf> {
        let dir_path = parent_dir.join(dir_name);
        std::fs::create_dir(&dir_path)?;
        self.refresh();
        Ok(dir_path)
    }

    pub fn delete_file(&self, file_path: &Path) -> std::io::Result<()> {
        if file_path.is_dir() {
            std::fs::remove_dir_all(file_path)?;
        } else {
            std::fs::remove_file(file_path)?;
        }
        self.refresh();
        Ok(())
    }

    pub fn rename_file(&self, old_path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
        let parent = old_path.parent().unwrap_or(Path::new("."));
        let new_path = parent.join(new_name);
        std::fs::rename(old_path, &new_path)?;
        self.refresh();
        Ok(new_path)
    }

    pub fn get_selected_path(&self) -> Option<PathBuf> {
        if let Some((model, iter)) = self.tree_view.selection().selected() {
            let path: String = model.value(&iter, COL_PATH as i32).get().ok()?;
            Some(PathBuf::from(path))
        } else {
            None
        }
    }

    pub fn get_selected_is_dir(&self) -> bool {
        if let Some((model, iter)) = self.tree_view.selection().selected() {
            model.value(&iter, COL_IS_DIR as i32).get().unwrap_or(false)
        } else {
            false
        }
    }
}
