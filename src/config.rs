pub const APP_ID: &str = "org.gtk_rs.Fikby";

// Keep CSS minimal and GTK-compatible (no unsupported properties)
pub const CSS: &str = r#"
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
.status {
    padding: 6px;
    color: #333;
    font-family: monospace;
}
"#;

// Highlighting cutoff to avoid UI stalls on huge files
pub const HIGHLIGHT_CHAR_CUTOFF: usize = 200_000;