//! How deep a file nests, measured before a grammar parses it, in one pass over its bytes
//! ([`super::NESTING_LIMIT`]).
//!
//! A deep parse tree costs every walk over it, and Python's scanner saves its indentation stack
//! in a fixed-size state that a few hundred levels overflow, which aborts the process. So the
//! runner parses no file that nests deeper than the bound, and the measure errs toward refusing:
//! every bracket counts, those in strings and comments included, and every leading whitespace
//! character is at least a column. A backslash and a newline in a line's leading whitespace join
//! the next physical line to it, as Python's scanner reads them, so the indentation goes on adding
//! up across them.

/// Whether `text` nests deeper than `limit`: more than `limit` brackets of `()[]{}` open at once,
/// or a line indented more than `limit` columns. A tab counts 8 columns, as Python's scanner
/// counts it, and any other leading whitespace character one. A backslash before a newline, or
/// before a carriage return and a newline, while a line's indentation is still being read, carries
/// its columns on to the next physical line's.
pub fn deeper_than(text: &str, limit: usize) -> bool {
    let bytes = text.as_bytes();
    let (mut depth, mut column, mut leading) = (0usize, 0usize, true);
    let mut at = 0;
    while let Some(&b) = bytes.get(at) {
        at += 1;
        if b == b'\n' {
            (column, leading) = (0, true);
            continue;
        }
        if leading && matches!(b, b' ' | b'\t' | b'\r' | 0x0b | 0x0c) {
            column += if b == b'\t' { 8 } else { 1 };
            if column > limit {
                return true;
            }
            continue;
        }
        if leading && b == b'\\' {
            // A continuation: the next physical line's whitespace adds to this line's columns.
            let newline = if bytes.get(at) == Some(&b'\r') { at + 1 } else { at };
            if bytes.get(newline) == Some(&b'\n') {
                at = newline + 1;
                continue;
            }
        }
        leading = false;
        match b {
            b'(' | b'[' | b'{' => {
                depth += 1;
                if depth > limit {
                    return true;
                }
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brackets_and_indentation_each_count_against_the_bound() {
        assert!(!deeper_than(&format!("{}{}", "(".repeat(4), ")".repeat(4)), 4));
        assert!(deeper_than(&"([{".repeat(2), 4), "brackets of every shape add up");
        assert!(!deeper_than(&"()".repeat(1000), 4), "closed brackets give their depth back");
        assert!(!deeper_than(&format!("{})", ")".repeat(10)), 4), "a stray close can't go below 0");
        assert!(!deeper_than("    x\n    y\n", 4));
        assert!(deeper_than("     x\n", 4));
        assert!(deeper_than("\tx\n", 4), "a tab is 8 columns");
        assert!(!deeper_than("x     \n", 4), "only leading whitespace is indentation");
    }

    #[test]
    fn indentation_adds_up_across_backslash_newlines() {
        assert!(deeper_than("   \\\n  x\n", 4), "3 then 2 columns are 5");
        assert!(deeper_than("   \\\r\n  x\n", 4), "a carriage return before the newline too");
        assert!(deeper_than("  \\\n\\\n  \\\n  x\n", 5), "across several continuations");
        assert!(!deeper_than("  \\\n  x\n", 4), "2 and 2 are within 4");
        assert!(!deeper_than("  x = 1 + \\\n  2\n", 4), "a continuation after code carries no columns");
        assert!(!deeper_than("   \\x\n  y\n", 4), "a backslash before anything else ends the indentation");
    }
}
