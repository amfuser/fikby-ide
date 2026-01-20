use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Verify syntect per-line highlight ranges map to correct character offsets (not bytes),
/// and that ranges are valid and non-overlapping for a sample Rust snippet containing
/// multi-byte characters.
#[test]
fn compute_syntect_ranges_are_character_correct() {
    // Load syntect resources
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    // Prefer a named theme if available, otherwise pick the first
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.get("InspiredGitHub"))
        .cloned()
        .unwrap_or_else(|| ts.themes.values().next().unwrap().clone());

    // Example text contains multi-byte characters (accent + emoji)
    let text = "fn main() {\n    println!(\"héllo 🌍\");\n}\n";

    // Total characters (not bytes)
    let total_chars = text.chars().count();

    // Build highlighter for Rust
    let syntax = ss
        .find_syntax_by_extension("rs")
        .expect("rust syntax should be present");
    let mut h = HighlightLines::new(syntax, &theme);

    let mut cumulative_chars: usize = 0;
    let mut ranges: Vec<(usize, usize, String)> = Vec::new();

    for line in LinesWithEndings::from(text) {
        let hl = h.highlight_line(line, &ss).expect("highlight_line should succeed");
        let mut local_char: usize = 0;
        for (style, slice) in hl {
            if slice.is_empty() {
                continue;
            }
            let slice_chars = slice.chars().count();
            let start = cumulative_chars + local_char;
            let end = start + slice_chars;
            if style.foreground.a > 0 {
                let fg = style.foreground;
                let color = format!("#{:02X}{:02X}{:02X}", fg.r, fg.g, fg.b);
                ranges.push((start, end, color));
            }
            local_char += slice_chars;
        }
        cumulative_chars += line.chars().count();
    }

    // cumulative count should match total characters
    assert_eq!(cumulative_chars, total_chars, "character counting mismatch");

    // Each range must be valid and inside [0, total_chars]
    for (s, e, _) in &ranges {
        assert!(*s < *e, "range start must be < end (start={}, end={})", s, e);
        assert!(*e <= total_chars, "range end must be <= total_chars (end={}, total={})", e, total_chars);
    }

    // Ensure ranges do not overlap (sanity check)
    let mut covered = vec![false; total_chars];
    for (s, e, _) in ranges {
        for i in s..e {
            assert!(!covered[i], "highlight ranges overlap at char index {}", i);
            covered[i] = true;
        }
    }
}