//! Lines as evidence counts them: 1-based, split on `\n`, without a trailing `\r`.

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
}
