# fikby-ide

A simple IDE built with GTK4 and Rust.

## Features

### Theme System
- Toggle between light and dark themes
- Keyboard shortcut: `Ctrl+T`
- Menu: View → Toggle Theme
- Themes include:
  - **Dark Theme**: Dark gray/black backgrounds with light text
  - **Light Theme**: Light gray/white backgrounds with dark text
- Automatic syntax highlighting updates when switching themes
- All UI elements update immediately (menubar, editor, gutter, tabs, status bar)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```
