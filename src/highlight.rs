use gtk4::prelude::*;
use gtk4::TextBuffer;
use gtk4::TextTag;
use std::cell::RefCell;
use std::collections::HashMap;
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::config;

/// Highlight the contents `text` into `buffer` using syntect's `ss` + `theme`.
/// Tag objects are cached in `tag_cache` keyed by color string.
/// This implementation is character-aware and clamps ranges to the buffer length,
/// and uses forward_chars to build TextIters (safe for GTK).
pub fn highlight_with_syntect(
    buffer: &TextBuffer,
    text: &str,
    tag_cache: &RefCell<HashMap<String, TextTag>>,
    ss: &SyntaxSet,
    theme: &Theme,
) {
    // Remove previously-applied tags in the cache
    let buf_start = buffer.start_iter();
    let buf_end = buffer.end_iter();
    for tag in tag_cache.borrow().values() {
        buffer.remove_tag(tag, &buf_start, &buf_end);
    }

    // Compute total characters (character count, not bytes)
    let full_start = buffer.start_iter();
    let full_end = buffer.end_iter();
    let full_text = buffer.text(&full_start, &full_end, false);
    let total_chars: i32 = full_text.chars().count() as i32;

    // Skip highlighting for very large buffers to protect UI
    if (total_chars as usize) > config::HIGHLIGHT_CHAR_CUTOFF {
        return;
    }

    // Choose syntax (for now Rust); caller should only call when appropriate
    let syntax = ss.find_syntax_by_extension("rs").unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);

    let mut cumulative_chars: usize = 0;
    for line in LinesWithEndings::from(text) {
        let ranges = h.highlight_line(line, ss).unwrap_or_else(|_| vec![]);
        let mut local_char: usize = 0;
        for (style, slice) in ranges {
            if slice.is_empty() {
                continue;
            }
            let slice_chars = slice.chars().count() as i32;
            let mut start_char = (cumulative_chars + local_char) as i32;
            let mut end_char = start_char + slice_chars;

            // clamp to buffer boundaries
            if start_char < 0 {
                start_char = 0;
            }
            if end_char < 0 {
                end_char = 0;
            }
            if start_char > total_chars {
                start_char = total_chars;
            }
            if end_char > total_chars {
                end_char = total_chars;
            }
            if end_char <= start_char {
                local_char += slice_chars as usize;
                continue;
            }

            if style.foreground.a > 0 {
                let fg = style.foreground;
                let color = format!("#{:02X}{:02X}{:02X}", fg.r, fg.g, fg.b);

                // Lookup or create tag in cache
                let tag = {
                    let mut cache = tag_cache.borrow_mut();
                    if let Some(existing) = cache.get(&color) {
                        existing.clone()
                    } else {
                        let t = TextTag::builder()
                            .name(&format!("syn_{}", color.trim_start_matches('#')))
                            .foreground(&color)
                            .build();
                        buffer.tag_table().add(&t);
                        cache.insert(color.clone(), t.clone());
                        t
                    }
                };

                // Build character-aware iters by advancing from start_iter()
                let mut it_start = buffer.start_iter();
                if start_char > 0 {
                    let _ = it_start.forward_chars(start_char);
                }
                let mut it_end = it_start.clone();
                let _ = it_end.forward_chars(end_char - start_char);

                buffer.apply_tag(&tag, &it_start, &it_end);
            }

            local_char += slice_chars as usize;
        }
        cumulative_chars += line.chars().count();
    }
}