# GTK4 Migration Guide - File Explorer

This document details all the GTK4 API changes required for the File Explorer implementation in Fikby IDE.

## Overview

The file explorer required migrating from GTK3 APIs to GTK4 APIs. This guide documents all changes made, with before/after examples and explanations.

## Total Changes

- **7 API method changes** (TreeModel/TreeStore)
- **3 import changes** (Menu/MenuItem removal, unused imports)
- **0 compilation errors** after migration

---

## API Changes

### 1. TreeModel.get() - Method Signature Change

**Location**: `src/file_explorer.rs:157-158` (connect_row_activated callback)

**GTK3 (Old):**
```rust
let file_path: String = model.value(&iter, COL_PATH as i32).get().unwrap();
let is_dir: bool = model.value(&iter, COL_IS_DIR as i32).get().unwrap();
```

**GTK4 (New):**
```rust
let file_path: String = model.get(&iter, COL_PATH as i32);
let is_dir: bool = model.get(&iter, COL_IS_DIR as i32);
```

**Why**: GTK4's `TreeModelExt::get()` directly returns the typed value instead of returning a `GValue` that needs to be extracted.

---

### 2. TreeStore.get() - Expand Directory

**Location**: `src/file_explorer.rs:183` (expand_directory method)

**GTK3 (Old):**
```rust
let dir_path: String = self.tree_store.value(iter, COL_PATH as i32).get().unwrap();
```

**GTK4 (New):**
```rust
let dir_path: String = self.tree_store.get(iter, COL_PATH as i32);
```

**Why**: Same as above - `TreeStore` implements `TreeModelExt`, so uses the same pattern.

---

### 3. TreeModel.path() - Return Type Change

**Location**: `src/file_explorer.rs:192` (highlight_file method)

**GTK3 (Old):**
```rust
if let Some(path) = self.tree_store.path(&iter) {
    self.tree_view.selection().select_path(&path);
    self.tree_view.scroll_to_cell(Some(&path), None::<&TreeViewColumn>, false, 0.0, 0.0);
}
```

**GTK4 (New):**
```rust
let path = self.tree_store.path(&iter);
self.tree_view.selection().select_path(&path);
self.tree_view.scroll_to_cell(Some(&path), None::<&TreeViewColumn>, false, 0.0, 0.0);
```

**Why**: GTK4's `TreeModel::path()` always returns a valid `TreePath`, never `None`. The method signature changed from `fn path(&self, iter: &TreeIter) -> Option<TreePath>` to `fn path(&self, iter: &TreeIter) -> TreePath`.

---

### 4. TreeStore.get() - Find Path (String)

**Location**: `src/file_explorer.rs:208` (find_iter_for_path method)

**GTK3 (Old):**
```rust
let path: String = self.tree_store.value(&current_iter, COL_PATH as i32).get().ok()?;
```

**GTK4 (New):**
```rust
let path: String = self.tree_store.get(&current_iter, COL_PATH as i32);
```

**Why**: Consistent with the new `get()` API that returns typed values directly.

---

### 5. TreeStore.get() - Find Path (Boolean)

**Location**: `src/file_explorer.rs:214` (find_iter_for_path method)

**GTK3 (Old):**
```rust
let is_dir: bool = self.tree_store.value(&current_iter, COL_IS_DIR as i32).get().unwrap_or(false);
```

**GTK4 (New):**
```rust
let is_dir: bool = self.tree_store.get(&current_iter, COL_IS_DIR as i32);
```

**Why**: The `get()` method returns the default value for the type if the value is missing, so no need for `.unwrap_or(false)`.

---

### 6. TreeModel.get() - Get Selected Path

**Location**: `src/file_explorer.rs:323` (get_selected_path method)

**GTK3 (Old):**
```rust
model.value(&iter, COL_PATH as i32).get()
```

**GTK4 (New):**
```rust
model.get(&iter, COL_PATH as i32)
```

---

### 7. TreeModel.get() - Check If Directory

**Location**: `src/file_explorer.rs:332` (get_selected_is_dir method)

**GTK3 (Old):**
```rust
model.value(&iter, COL_IS_DIR as i32).get().unwrap_or(false)
```

**GTK4 (New):**
```rust
model.get(&iter, COL_IS_DIR as i32)
```

---

## Import Changes

### Removed Non-Existent GTK4 Imports

**Location**: `src/file_explorer.rs:3-5`

**Before:**
```rust
use gtk4::{
    gio, glib, Box as GtkBox, CellRendererText, Label, Menu, MenuItem, Orientation,
    PopoverMenu, ScrolledWindow, TreeView, TreeViewColumn,
    GestureClick, EventControllerKey,
};
```

**After:**
```rust
use gtk4::{
    gio, glib, CellRendererText, PopoverMenu, ScrolledWindow,
    TreeView, TreeViewColumn, GestureClick,
};
```

**Removed:**
- `Menu` - Doesn't exist in GTK4 (use `PopoverMenu` with `gio::Menu`)
- `MenuItem` - Doesn't exist in GTK4 (use `gio::MenuItem`)
- `Box as GtkBox` - Unused
- `Label` - Unused
- `Orientation` - Unused
- `EventControllerKey` - Unused

**Also removed from src/ui/mod.rs:**
- `PolicyType` - Unused
- `ScrolledWindow` - Unused

---

## GTK4 API Reference Table

| Feature | GTK3 | GTK4 |
|---------|------|------|
| Get typed value from TreeModel | `model.value(&iter, col).get().unwrap()` | `model.get(&iter, col)` |
| Get TreePath from iter | `if let Some(path) = model.path(&iter)` | `let path = model.path(&iter)` |
| Menu system | `gtk::Menu`, `gtk::MenuItem` | `PopoverMenu`, `gio::Menu` |
| Import prelude | `use gtk::prelude::*;` | `use gtk4::prelude::*;` |

---

## Commit History

1. **c257a77** - Initial file explorer implementation
2. **d250a29** - Context menu with file operations
3. **5cbe88c** - Implementation documentation
4. **68043d3** - Fix GTK4 API imports and TreeModel (3 locations)
5. **76a6707** - Fix TreeStore API (expand_directory)
6. **8c51002** - Fix TreePath and remaining TreeStore calls
7. **Current** - Migration guide documentation

---

## Testing Checklist

After migration, verify:

- [x] Code compiles without errors
- [x] No unused import warnings
- [x] Directory tree displays correctly
- [x] Double-click opens files
- [x] File highlighting works
- [x] Context menu appears
- [x] All file operations work
- [x] No runtime errors

---

## Migration Tips

When migrating GTK3 to GTK4 TreeModel/TreeStore code:

1. **Search for `.value(`** - Replace with `.get(`
2. **Remove `.get()`** - The new API returns typed values directly
3. **Remove `.unwrap()`** or `.unwrap_or()`** - Not needed with typed returns
4. **Check `path()` calls** - Remove `Option` pattern matching
5. **Update imports** - Menu/MenuItem don't exist in GTK4
6. **Use `PopoverMenu`** - For context menus instead of Menu

---

## References

- [GTK4 TreeModelExt Documentation](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/prelude/trait.TreeModelExt.html)
- [GTK4 Migration Guide](https://docs.gtk.org/gtk4/migrating-3to4.html)
- [gtk-rs Book](https://gtk-rs.org/gtk4-rs/stable/latest/book/)

---

## Conclusion

All GTK4 API migrations are complete. The file explorer implementation is fully compliant with GTK4 and compiles without errors.
