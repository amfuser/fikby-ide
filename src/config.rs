pub const APP_ID: &str = "org.gtk_rs.Fikby";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn css(&self) -> &'static str {
        match self {
            ThemeMode::Light => LIGHT_CSS,
            ThemeMode::Dark => DARK_CSS,
        }
    }
    
    pub fn syntax_theme_name(&self) -> &'static str {
        match self {
            ThemeMode::Light => "base16-ocean.light",
            ThemeMode::Dark => "base16-ocean.dark",
        }
    }
}

// Light theme CSS
const LIGHT_CSS: &str = r#"
window {
    background: #ffffff;
    color: #000000;
}
.menubar {
    background: #f5f5f5;
    padding: 4px 10px;
}
.menubutton {
    font-weight: 600;
    padding: 2px 1px;
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
    padding-top: 0px;
    padding-bottom: 0px;
    font-family: monospace;
    font-size: 10pt;
    line-height: 1.2;
}
.editor-view {
    font-family: monospace;
    font-size: 10pt;
    line-height: 1.2;
    background: #ffffff;
    color: #000000;
}
.status {
    padding: 6px;
    background: #f5f5f5;
    color: #333;
    font-family: monospace;
}
notebook {
    background: #ffffff;
}
notebook > header {
    background: #f0f0f0;
}
notebook > header > tabs > tab {
    background: #e8e8e8;
    color: #333;
}
notebook > header > tabs > tab:checked {
    background: #ffffff;
    color: #000000;
}
paned > separator {
    background: #cccccc;
}
popover {
    background: #ffffff;
    color: #000000;
}
"#;

// Dark theme CSS
const DARK_CSS: &str = r#"
window {
    background: #1e1e1e;
    color: #d4d4d4;
}
.menubar {
    background: #2b2b2b;
    padding: 4px 10px;
}
.menubutton {
    font-weight: 600;
    padding: 2px 1px;
    border-radius: 4px;
    color: #e0e0e0;
}
.menubutton:hover {
    background: #3a3a3a;
}
.right-button {
    padding: 4px 8px;
    margin-right: 6px;
    color: #e0e0e0;
}
.gutter {
    background: #2b2b2b;
    color: #a0a0a0;
    padding-left: 6px;
    padding-right: 6px;
    padding-top: 0px;
    padding-bottom: 0px;
    font-family: monospace;
    font-size: 10pt;
    line-height: 1.2;
}
.editor-view {
    font-family: monospace;
    font-size: 10pt;
    line-height: 1.2;
    background: #1e1e1e;
    color: #d4d4d4;
}
.status {
    padding: 6px;
    background: #2b2b2b;
    color: #e0e0e0;
    font-family: monospace;
}
notebook {
    background: #1e1e1e;
}
notebook > header {
    background: #252526;
}
notebook > header > tabs > tab {
    background: #2b2b2b;
    color: #cccccc;
}
notebook > header > tabs > tab:checked {
    background: #1e1e1e;
    color: #ffffff;
}
paned > separator {
    background: #3e3e3e;
}
popover {
    background: #2b2b2b;
    color: #cccccc;
}
"#;

// Highlighting cutoff to avoid UI stalls on huge files
pub const HIGHLIGHT_CHAR_CUTOFF: usize = 200_000;