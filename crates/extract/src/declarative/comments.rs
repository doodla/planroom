//! The comment spans of a file in a language with no built-in grammar, read by the language's
//! comment marks, and blanking spans before a regex runs, so a comment never creates a fact
//! (DR-12, DR-46). A language with a grammar takes its spans from the parse ([`super::prose`])
//! instead.
//!
//! The marks come from the file's `[[languages]]` entry: `line_comment` and `block_comment`. A
//! mark inside a string literal is no comment. Every rule errs toward blanking: losing a fact
//! beats making one from a comment.
//!
//! - Every quote, a double quote included, opens a string only if its closing quote on the line
//!   comes before any comment mark, a line-comment mark or a block-comment opening mark, that
//!   follows it. Otherwise it is a plain character, as Ruby's `?'` and `?"`, a Lisp quote, a
//!   Clojure `\"`, a YAML inch mark, a lifetime or a regex literal's quote is, and the mark starts
//!   a comment.
//! - A URL's `//` is no comment mark while a quote looks for its close, so a URL in a string
//!   closes the string and matches. It is a URL's only when the `:` before it ends a scheme, a
//!   word of at least two characters that starts with a letter, such as `http` or `git+ssh`, and
//!   a character that isn't a space follows it. Outside a string it is still a mark.
//! - Any quote opens a string only when it starts a token or follows a string prefix such as
//!   Python's `f` or Rust's `b`, so an apostrophe inside a word opens nothing; a quote that
//!   doesn't close on its line opens nothing.
//! - A block comment ends where its opening and closing marks balance, so a nested one ends at its
//!   outer close. A language whose block comments don't nest loses the text between an inner open
//!   mark and the outer close, which only errs toward blanking.
//!
//! A blanked character becomes spaces of its own byte length and every line break stays, so a
//! match keeps its byte offset and its line number.

use std::ops::Range;

/// One language's comment marks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub line: Option<String>,
    /// The opening and closing marks.
    pub block: Option<(String, String)>,
}

impl Marks {
    pub fn is_empty(&self) -> bool {
        self.line.is_none() && self.block.is_none()
    }

    /// Whether a comment mark starts `rest`.
    fn comment_at(&self, rest: &str) -> bool {
        self.line.as_deref().is_some_and(|l| rest.starts_with(l))
            || self.block.as_ref().is_some_and(|(open, _)| rest.starts_with(open.as_str()))
    }
}

/// The letters that may prefix a string literal: Python's `r`, `b`, `f` and `u`, Rust's `b` and
/// `r`, in either case.
const PREFIXES: &str = "rRbBfFuU";

const QUOTES: [char; 3] = ['\'', '"', '`'];

/// The byte ranges of `text` that `marks` make comments. A block comment that never closes runs
/// to the end of the file.
pub fn spans(text: &str, marks: &Marks) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if marks.is_empty() {
        return out;
    }
    // Per quote, the offset before which it can't close: a quote that found no close on its line
    // proves that no later one on that line closes either, so a line is scanned once per quote.
    let mut open_until = [0usize; 3];
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let rest = &text[i..];
        if let Some((open, close)) = &marks.block
            && rest.starts_with(open.as_str())
        {
            let end = balanced(text, i + open.len(), open, close);
            out.push(i..end);
            i = end;
            continue;
        }
        if let Some(line) = &marks.line
            && rest.starts_with(line.as_str())
        {
            let end = rest.find('\n').map_or(text.len(), |k| i + k);
            out.push(i..end);
            i = end;
            continue;
        }
        if let Some(q) = QUOTES.iter().position(|quote| *quote == c)
            && i >= open_until[q]
            && opens(&text[..i])
        {
            // A quote whose close comes after a comment mark opens nothing.
            match closes(text, i + c.len_utf8(), c, marks) {
                Some(end) => {
                    i = end;
                    continue;
                }
                None => open_until[q] = rest.find('\n').map_or(text.len(), |k| i + k),
            }
        }
        i += c.len_utf8();
    }
    out
}

/// Where the string whose `quote` opened just before `from` ends: just past its closing quote on
/// the same line, or `None` when the line ends first, or when a comment mark of `marks` comes
/// first. A URL's `//` ([`url_slashes`]) is no mark.
fn closes(text: &str, from: usize, quote: char, marks: &Marks) -> Option<usize> {
    let mut chars = text[from..].char_indices();
    while let Some((k, c)) = chars.next() {
        let rest = &text[from + k..];
        if marks.comment_at(rest) && !url_slashes(&text[from..from + k], rest) {
            return None;
        }
        match c {
            '\n' => return None,
            '\\' => {
                if let Some((_, '\n')) = chars.next() {
                    return None;
                }
            }
            c if c == quote => return Some(from + k + c.len_utf8()),
            _ => {}
        }
    }
    None
}

/// Whether the `//` that starts `rest` is a URL's: the string so far, `head`, ends in a scheme and
/// its `:`, and a character that isn't a space follows the slashes. A scheme is a word of at least
/// two characters, a letter and then letters, digits, `+`, `.` or `-`, as in `http` or `git+ssh`.
/// The word is read back only to the character before it, which no other `://` reads, so a line
/// costs time linear in its length.
fn url_slashes(head: &str, rest: &str) -> bool {
    let Some(word) = head.strip_suffix(':') else { return false };
    let scheme_char = |c: &char| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-');
    let len = word.chars().rev().take_while(scheme_char).count();
    // Every scheme character is ASCII, one byte each.
    let scheme = &word[word.len() - len..];
    let after = rest.strip_prefix("//").and_then(|r| r.chars().next());
    len >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && after.is_some_and(|c| !c.is_whitespace())
}

/// Where a block comment whose body starts at `from` ends: just past the close that balances its
/// open, counting each open and close inside it, or the end of the file. Marks that are the same
/// can't nest, so the first close ends it.
fn balanced(text: &str, from: usize, open: &str, close: &str) -> usize {
    if open == close {
        return text[from..].find(close).map_or(text.len(), |k| from + k + close.len());
    }
    let mut depth = 1usize;
    let mut k = from;
    while let Some(c) = text[k..].chars().next() {
        let rest = &text[k..];
        if rest.starts_with(close) {
            depth -= 1;
            k += close.len();
            if depth == 0 {
                return k;
            }
        } else if rest.starts_with(open) {
            depth += 1;
            k += open.len();
        } else {
            k += c.len_utf8();
        }
    }
    text.len()
}

/// Whether a quote after `before` may open a string: it starts a token, or follows a short string
/// prefix that starts one. An apostrophe inside a word, as in `don't`, opens nothing.
fn opens(before: &str) -> bool {
    let word: Vec<char> =
        before.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').take(3).collect();
    word.len() <= 2 && word.iter().all(|c| PREFIXES.contains(*c))
}

/// `text` with every byte in `spans` blanked: a space per byte, line breaks kept. Spans may
/// overlap; each must start and end on a character boundary.
pub fn blank(text: &str, spans: &[Range<usize>]) -> String {
    if spans.is_empty() {
        return text.to_owned();
    }
    let mut bytes = text.as_bytes().to_vec();
    for span in spans {
        for b in &mut bytes[span.start.min(text.len())..span.end.min(text.len())] {
            if *b != b'\n' && *b != b'\r' {
                *b = b' ';
            }
        }
    }
    // Every blanked span covers whole characters, and a space is one byte, so the text stays
    // UTF-8.
    String::from_utf8(bytes).expect("blanking whole characters keeps UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_like() -> Marks {
        Marks { line: Some("//".into()), block: Some(("/*".into(), "*/".into())) }
    }

    fn hash() -> Marks {
        Marks { line: Some("#".into()), ..Marks::default() }
    }

    fn lexed(text: &str, marks: &Marks) -> String {
        blank(text, &spans(text, marks))
    }

    /// `text` with everything but its line breaks blanked.
    fn all_blank(text: &str) -> String {
        text.chars().map(|c| if c == '\n' { c } else { ' ' }).collect()
    }

    #[test]
    fn comments_blank_and_offsets_stay() {
        let text = "a // b\nc /* d\ne */ f\n\"a:/\" \"*\" 'x' g\n";
        let blanked = lexed(text, &c_like());
        assert_eq!(blanked.len(), text.len());
        assert_eq!(blanked, "a     \nc     \n     f\n\"a:/\" \"*\" 'x' g\n");
    }

    #[test]
    fn every_quote_stops_at_a_comment_mark_but_a_urls_slashes() {
        let c = c_like();
        let trimmed = |text: &str, marks: &Marks| lexed(text, marks).trim_end().to_owned();
        assert_eq!(trimmed("u = \"http://a/b\" // x\n", &c), "u = \"http://a/b\"", "a URL's // is no mark");
        assert_eq!(trimmed("u = 'http://a/b' // x\n", &c), "u = 'http://a/b'");
        assert_eq!(trimmed("'//' g\n", &c), "'", "a single quote stops at a line mark");
        assert_eq!(trimmed("\"//\" g\n", &c), "\"", "and so does a double quote");
        assert_eq!(trimmed("\"/*\" g */ h\n", &c), "\"         h", "at a block mark too");
        assert_eq!(trimmed("\"a: // b\" g\n", &c), "\"a:", "a // after a space is a mark");
        assert_eq!(trimmed("x: // \"a\"\n", &c), "x:", "outside a string, :// is still a mark");
        assert_eq!(trimmed("c = ?\" # the \"http://a\" quote\n", &hash()), "c = ?\"", "Ruby's ?\"");
        let lisp = Marks { line: Some(";".into()), ..Marks::default() };
        assert_eq!(trimmed("(def q \\\") ; see \"http://a\"\n", &lisp), "(def q \\\")", "a Clojure \\\"");
        assert_eq!(trimmed("a: b\" # see \"http://a\"\n", &hash()), "a: b\"", "a YAML inch mark");
        assert_eq!(
            lexed("`/*` g */ h\n", &c),
            format!("`{}h\n", " ".repeat(9)),
            "a backtick at a block mark"
        );
        assert_eq!(trimmed("'a' // it's\n", &c), "'a'", "a quote that closes first is a string");
        assert_eq!(trimmed("c = ?' # see http://a, don't\n", &hash()), "c = ?'", "a Ruby character literal");
        let lisp = Marks { line: Some(";".into()), ..Marks::default() };
        assert_eq!(trimmed("(def x 'foo) ; it's\n", &lisp), "(def x 'foo)", "a Lisp quote");
        assert_eq!(trimmed("a: ^'b # it's\n", &hash()), "a: ^'b", "a YAML ^'");
    }

    #[test]
    fn a_nested_block_comment_ends_where_its_marks_balance() {
        let c = c_like();
        assert_eq!(lexed("/* a /* b */ c */ d\n", &c), format!("{}d\n", " ".repeat(18)));
        let open = "/* a /* b */ c\nd\n";
        assert_eq!(lexed(open, &c), all_blank(open), "an unbalanced one runs to the end");
        let same = Marks { block: Some(("--".into(), "--".into())), ..Marks::default() };
        let eight = " ".repeat(8);
        assert_eq!(
            lexed("-- a -- b -- c --\n", &same),
            format!("{eight}b{eight}\n"),
            "equal marks can't nest"
        );
    }

    #[test]
    fn an_apostrophe_in_a_word_opens_no_string_and_a_prefix_does() {
        let hash = hash();
        assert_eq!(lexed("don't # x\n", &hash), "don't    \n");
        assert_eq!(lexed("f\"{x}\" # y\n", &hash), "f\"{x}\"    \n");
        assert_eq!(lexed("é # é\n", &hash), "é     \n", "a blanked é takes its two bytes");
    }

    #[test]
    fn a_quote_that_doesnt_close_on_its_line_opens_nothing() {
        let hash = hash();
        assert_eq!(lexed("'unclosed # x\nz # y\n", &hash), "'unclosed    \nz    \n");
        assert_eq!(lexed("c = ?' # x\n", &hash), "c = ?'    \n", "a Ruby character literal");
        let c = c_like();
        assert_eq!(lexed("fn f(k: &'static str) { // x\n", &c), "fn f(k: &'static str) {     \n");
        assert_eq!(lexed("const re = /'/; // x\n", &c), "const re = /'/;     \n");
        assert_eq!(lexed("`a\n// x\n`\n", &c), "`a\n    \n`\n", "a string never spans lines");
        assert_eq!(lexed("'a\\'b' // x\n", &c), "'a\\'b'     \n", "an escaped quote doesn't close");
        // A line of quotes that never close is read once, not once per quote.
        let long = format!("'{}// x\n", "\\'".repeat(200_000));
        assert!(lexed(&long, &c).ends_with("\\'    \n"));
    }

    #[test]
    fn no_marks_leaves_the_text() {
        assert_eq!(lexed("# a // b", &Marks::default()), "# a // b");
    }
}
