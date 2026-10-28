//! Lines as evidence counts them: 1-based, split on `\n`, without a trailing `\r`.
//!
//! A reader that cites many lines of one text indexes it once ([`Index`]) and finds each line by a
//! binary search, so citing every line of a file stays linear in its size.

use planroom_contract::CONTAINS_MAX_CHARS;

/// Line `n` (1-based) of `text`.
pub fn line(text: &str, n: u32) -> Option<&str> {
    let index = (n as usize).checked_sub(1)?;
    let line = text.split('\n').nth(index)?;
    // A text ending in `\n` has no line after it.
    if line.is_empty() && index > 0 && text.split('\n').count() == index + 1 {
        return None;
    }
    Some(line.strip_suffix('\r').unwrap_or(line))
}

/// The byte offset each line of one text starts at, read in one pass, so finding a line, or the
/// line an offset sits on, is a binary search rather than a pass over the text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    starts: Vec<usize>,
    len: usize,
}

impl Index {
    pub fn new(text: &str) -> Index {
        let starts = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        Index { starts, len: text.len() }
    }

    /// The 1-based line that byte offset `at` sits on.
    pub fn line_of(&self, at: usize) -> u32 {
        let index = self.starts.partition_point(|s| *s <= at);
        u32::try_from(index.max(1)).unwrap_or(u32::MAX)
    }

    /// Line `n` (1-based) of `text`, the text this index was made from, as [`line`] reads it.
    pub fn line<'t>(&self, text: &'t str, n: u32) -> Option<&'t str> {
        debug_assert_eq!(text.len(), self.len, "an index reads only the text it was made from");
        let index = (n as usize).checked_sub(1)?;
        let start = *self.starts.get(index)?;
        // A text ending in `\n` has no line after it.
        if start == self.len && index > 0 {
            return None;
        }
        let end = self.starts.get(index + 1).map_or(self.len, |next| next - 1);
        let line = text.get(start..end)?;
        Some(line.strip_suffix('\r').unwrap_or(line))
    }
}

/// `text` without the UTF-8 byte order mark it may start with, which some editors write and no
/// JSON, YAML or TOML parser here reads. It sits on line 1 alone, so lines number the same either
/// way.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// A line as `Evidence::Line.contains` keeps it: at most 200 characters.
pub fn cut(line: &str) -> String {
    line.chars().take(CONTAINS_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_one_based_and_end_before_the_final_newline() {
        let text = "a\r\nb\nc\n";
        assert_eq!(line(text, 1), Some("a"));
        assert_eq!(line(text, 3), Some("c"));
        assert_eq!(line(text, 4), None);
        assert_eq!(line(text, 0), None);
        assert_eq!(line("", 1), Some(""));
        assert_eq!(cut(&"é".repeat(300)).chars().count(), 200);
    }

    #[test]
    fn an_index_reads_every_line_as_a_split_does() {
        for text in ["", "a", "a\n", "a\r\nb\nc\n", "\n\n", "x\n\ny\r\n\r\nz", "é\nü", "\r\n"] {
            let index = Index::new(text);
            for n in 0..7 {
                assert_eq!(index.line(text, n), line(text, n), "{text:?} line {n}");
            }
        }
        let index = Index::new("ab\ncd\n\ne");
        assert_eq!([0, 1, 2, 3, 6, 7].map(|i| index.line_of(i)), [1, 1, 1, 2, 3, 4]);
    }
}
