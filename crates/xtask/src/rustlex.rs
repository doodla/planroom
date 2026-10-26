//! Just enough of a Rust lexer for the boundary scans: string literals, and the code with comments
//! and literal contents blanked out, so a pattern inside a string or a comment never matches code.

/// A string literal: its unescaped-enough contents and where it sits.
pub struct Literal {
    /// Byte offset of the opening quote (or prefix) in the source.
    pub start: usize,
    pub line: usize,
    /// The contents between the quotes, escapes left as written.
    pub value: String,
}

pub struct Lexed {
    /// The source with every comment and literal replaced by spaces (newlines kept).
    pub code: String,
    pub literals: Vec<Literal>,
}

pub fn lex(src: &str) -> Lexed {
    let bytes = src.as_bytes();
    let mut code = bytes.to_vec();
    let mut literals = Vec::new();
    let mut i = 0;
    let blank = |code: &mut Vec<u8>, from: usize, to: usize| {
        for b in &mut code[from..to] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    };
    let line_of = |at: usize| bytes[..at].iter().filter(|&&b| b == b'\n').count() + 1;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();
        if b == b'/' && next == Some(b'/') {
            let end = src[i..].find('\n').map_or(bytes.len(), |n| i + n);
            blank(&mut code, i, end);
            i = end;
        } else if b == b'/' && next == Some(b'*') {
            let (mut depth, mut j) = (1, i + 2);
            while j < bytes.len() && depth > 0 {
                if bytes[j] == b'/' && bytes.get(j + 1) == Some(&b'*') {
                    depth += 1;
                    j += 2;
                } else if bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/') {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            i = j;
        } else if let Some((open, hashes)) = raw_string_start(bytes, i) {
            let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat_n(b'#', hashes)).collect();
            let body = open + 1;
            let end = find(bytes, body, &close).unwrap_or(bytes.len());
            literals.push(Literal { start: i, line: line_of(i), value: src[body..end].to_owned() });
            let stop = (end + close.len()).min(bytes.len());
            blank(&mut code, i, stop);
            i = stop;
        } else if b == b'"' || (matches!(b, b'b' | b'c') && next == Some(b'"') && !ident_before(bytes, i)) {
            let body = if b == b'"' { i + 1 } else { i + 2 };
            let mut j = body;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = j.min(bytes.len());
            literals.push(Literal { start: i, line: line_of(i), value: src[body..end].to_owned() });
            let stop = (end + 1).min(bytes.len());
            blank(&mut code, i, stop);
            i = stop;
        } else if b == b'\'' {
            // A char literal ('x', '\n', '\u{..}', '"') or a lifetime ('a).
            let end = if next == Some(b'\\') {
                src[i + 2..].find('\'').map(|n| i + 2 + n)
            } else {
                let ch_len = src[i + 1..].chars().next().map_or(1, char::len_utf8);
                (bytes.get(i + 1 + ch_len) == Some(&b'\'')).then_some(i + 1 + ch_len)
            };
            match end {
                Some(end) => {
                    blank(&mut code, i, end + 1);
                    i = end + 1;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    Lexed { code: String::from_utf8(code).expect("only ASCII bytes were replaced"), literals }
}

/// `r"`, `r#"`, `br"`, `cr##"` and so on, not preceded by an identifier character.
fn raw_string_start(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if ident_before(bytes, i) {
        return None;
    }
    let mut j = i;
    if matches!(bytes.get(j), Some(b'b' | b'c')) {
        j += 1;
    }
    if bytes.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    let hashes = bytes[j..].iter().take_while(|&&b| b == b'#').count();
    j += hashes;
    (bytes.get(j) == Some(&b'"')).then_some((j, hashes))
}

fn ident_before(bytes: &[u8], i: usize) -> bool {
    i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    hay.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_are_found_and_blanked_but_comments_are_not_literals() {
        let src =
            "let a = \"code.x\"; // \"plan.y\"\n/* r\"z\" */ let b = r#\"q\"#; let c = '\"'; fn f<'a>() {}\n";
        let lexed = lex(src);
        let values: Vec<&str> = lexed.literals.iter().map(|l| l.value.as_str()).collect();
        assert_eq!(values, ["code.x", "q"]);
        assert!(!lexed.code.contains("code.x") && !lexed.code.contains("plan.y"));
        assert!(lexed.code.contains("fn f<'a>()"));
        assert_eq!(lexed.code.len(), src.len());
    }

    #[test]
    fn escaped_quotes_stay_inside_the_literal() {
        let lexed = lex(r#"x("a\"b", "c")"#);
        let values: Vec<&str> = lexed.literals.iter().map(|l| l.value.as_str()).collect();
        assert_eq!(values, [r#"a\"b"#, "c"]);
    }
}
