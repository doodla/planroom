//! Reading a call's text: the one reader of a route call's arguments, a definition's parameters
//! and a call site's arguments, so a wrapper's call sites bind the method and path it passes on
//! (DR-12).
//!
//! - **A literal** is a quoted string, a template with no interpolation, a Python string with an
//!   `r`, `b`, `u` or `f` prefix and no interpolation, or a Go raw string.
//! - **A template** is a JavaScript or TypeScript template literal with `${…}`, or a Python
//!   f-string with `{…}`. Each interpolation is a hole; in an f-string `{{` and `}}` are a plain
//!   `{` and `}`, and in a template literal a bare `{` is plain text.
//! - **A route's base** is the hint's `base` when a pack sets one; otherwise a template's leading
//!   interpolation, the leading operand of a `+` that isn't a literal, or the scheme and host of a
//!   literal URL. Only a literal base names a host.
//! - **Its path** is what follows the base, as one canonical path: a segment holding a hole
//!   becomes `{}` whole, the query and fragment are left out, so no query-string secret reaches an
//!   edge's `via`, and a trailing `/` is dropped but at the root. A path that doesn't start with
//!   `/` makes no route call. [`super::normalize_path`] then compares it with the served routes.
//! - **A method** is a quoted verb or a bare word naming one, in any case; `PostForm` is `POST`,
//!   and a selector ending `Method<Verb>` (Go's `http.MethodPost`) is that verb. None is `GET`.
//!   Any other bare name can only bind.
//! - **Parameters** come from a definition's value with one outer `(…)` stripped, split at
//!   top-level commas: a name is the first identifier after a modifier (`private`, `public`,
//!   `protected`, `readonly`, `override`, `mut`, `&`, `*`, `**`, `...`), ending at `:`, `=`, `?`
//!   or a space. A bare `*` or `/` is no parameter, and the parameters after a bare `*` or a
//!   `*args` bind only by keyword. A first parameter named `self` or `cls`, Rust's `self` in any
//!   form, or TypeScript's `this`, is the receiver and is skipped. A default is the text after a
//!   parameter's top-level `=`.
//! - **A call site's arguments** split the same way: `name=value` binds by keyword, the rest by
//!   position. A spread argument (`*xs`, `**kw`, `...xs`) leaves every parameter it could fill
//!   unknown, so a call site that needs one binds nothing.
//!
//! Hand-rolled: the crate may use no parser of argument text across five languages. Every scan is
//! bracket- and quote-aware and linear in the text it reads, which the runner bounds per hint
//! (DR-100), and each call site's arguments are split once however many hints bind through it.

use super::{last_segment, url_host, url_path};

/// The methods a call may name.
pub const METHODS: [&str; 9] =
    ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT"];

/// How many call sites one wrapped hint reads at most, the first by path and line. The rest bind
/// nothing.
pub const BIND_SITES: usize = 64;

/// One piece of an argument's text: literal text, a hole a template or a non-literal operand
/// leaves, or a whole operand that is a bare name, which only a call site binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Lit(String),
    Hole(String),
    Name(String),
}

/// The text inside the brackets, quotes and template interpolations a scan has opened.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    Bracket(char),
    Quote(char),
    Template,
    Interpolation,
}

/// Each top-level `sep` of `text`, outside brackets, quotes and templates, by byte offset. Angle
/// brackets count too when `angles`: a `<` after a name opens one, and a `>` closes one but after
/// `=` or `-`, so a type's `Map<string, number>` stays one parameter and an arrow ends nothing.
fn top_level(text: &str, sep: &[char], angles: bool) -> Vec<usize> {
    let mut stack: Vec<Open> = Vec::new();
    let mut out = Vec::new();
    let mut prev = ' ';
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match stack.last().copied() {
            Some(Open::Quote(q)) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    stack.pop();
                }
            }
            Some(Open::Template) => {
                if c == '\\' {
                    chars.next();
                } else if c == '`' {
                    stack.pop();
                } else if c == '$' && chars.peek().is_some_and(|(_, n)| *n == '{') {
                    chars.next();
                    stack.push(Open::Interpolation);
                }
            }
            _ => match c {
                // A Rust lifetime, `&'a str`, opens no quote in a parameter list.
                '\'' if angles && lifetime(&text[i + 1..]) => {}
                '\'' | '"' => stack.push(Open::Quote(c)),
                '`' => stack.push(Open::Template),
                '(' | '[' | '{' => stack.push(Open::Bracket(c)),
                '<' if angles && (prev.is_alphanumeric() || prev == '_') => stack.push(Open::Bracket('<')),
                ')' | ']' | '}' | '>' => {
                    let opener = match c {
                        ')' => Open::Bracket('('),
                        ']' => Open::Bracket('['),
                        '>' => Open::Bracket('<'),
                        _ => Open::Bracket('{'),
                    };
                    let closes = match stack.last() {
                        Some(Open::Interpolation) => c == '}',
                        Some(top) => *top == opener && !(c == '>' && matches!(prev, '=' | '-')),
                        None => false,
                    };
                    if closes {
                        stack.pop();
                    }
                }
                _ if stack.is_empty() && sep.contains(&c) => out.push(i),
                _ => {}
            },
        }
        prev = c;
    }
    out
}

/// Whether `rest`, what follows a `'`, is a lifetime's name: an identifier no `'` closes.
fn lifetime(rest: &str) -> bool {
    let name_len = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    name_len > 0 && !rest[name_len..].starts_with('\'')
}

/// `text` split at its top-level `sep`, each part trimmed; an empty text gives no part.
fn split(text: &str, sep: char, angles: bool) -> Vec<&str> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut start = 0;
    for at in top_level(text, &[sep], angles) {
        parts.push(text[start..at].trim());
        start = at + sep.len_utf8();
    }
    parts.push(text[start..].trim());
    parts
}

/// `text` without one pair of outer parentheses, when they enclose all of it.
fn unwrap_parens(text: &str) -> &str {
    let text = text.trim();
    let Some(inner) = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) else { return text };
    // `(a)(b)` isn't wrapped: the first `(` must close at the end.
    if top_level(inner, &[')'], false).is_empty() && balanced(inner) { inner } else { text }
}

/// Whether `text` opens no bracket, quote or template it doesn't close.
fn balanced(text: &str) -> bool {
    let mut depth = 0i64;
    for c in text.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Whether `text` is a bare identifier.
pub fn ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Whether `text` is a bare name: identifiers joined by `.`, `::` or `->`, such as `path`,
/// `self._base` or `http.MethodPost`.
pub fn name(text: &str) -> bool {
    !text.is_empty() && text.replace("::", ".").replace("->", ".").split('.').all(ident)
}

/// The pieces of one string literal or template, quotes and prefix included in `text`, or `None`
/// when `text` is no single string.
fn string(text: &str) -> Option<Vec<Piece>> {
    let quote_at = text.find(['\'', '"', '`'])?;
    let prefix = text[..quote_at].to_ascii_lowercase();
    let q = text[quote_at..].chars().next()?;
    let ok_prefix = prefix.is_empty()
        || (q != '`' && prefix.len() <= 2 && prefix.chars().all(|c| matches!(c, 'r' | 'b' | 'u' | 'f')));
    if !ok_prefix {
        return None;
    }
    let body = &text[quote_at..];
    // One string: its opening quote closes at its last character, and nothing follows.
    if body.len() < 2 || !body.ends_with(q) {
        return None;
    }
    let closes = {
        let mut stack: Vec<Open> = Vec::new();
        let mut end = None;
        let mut chars = body.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            match stack.last().copied() {
                None if i == 0 => stack.push(if q == '`' { Open::Template } else { Open::Quote(q) }),
                None => {
                    end = Some(i);
                    break;
                }
                Some(Open::Quote(o)) => {
                    if c == '\\' && !prefix.contains('r') {
                        chars.next();
                    } else if c == o {
                        stack.pop();
                        end = Some(i);
                    }
                }
                Some(Open::Template) => {
                    if c == '\\' {
                        chars.next();
                    } else if c == '`' {
                        stack.pop();
                        end = Some(i);
                    } else if c == '$' && chars.peek().is_some_and(|(_, n)| *n == '{') {
                        chars.next();
                        stack.push(Open::Interpolation);
                    }
                }
                Some(Open::Interpolation | Open::Bracket(_)) => match c {
                    '{' => stack.push(Open::Bracket('{')),
                    '}' => {
                        stack.pop();
                    }
                    _ => {}
                },
            }
        }
        stack.is_empty() && end == Some(body.len() - q.len_utf8())
    };
    if !closes {
        return None;
    }
    let inner = &body[q.len_utf8()..body.len() - q.len_utf8()];
    if q == '`' {
        Some(template(inner))
    } else if prefix.contains('f') {
        Some(f_string(inner))
    } else {
        Some(vec![Piece::Lit(inner.to_owned())])
    }
}

/// A template literal's pieces: `${…}` is a hole, as deep as its braces nest.
fn template(inner: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            lit.push(c);
            lit.extend(chars.next());
        } else if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let source = hole(&mut chars);
            if !lit.is_empty() {
                pieces.push(Piece::Lit(std::mem::take(&mut lit)));
            }
            pieces.push(Piece::Hole(source));
        } else {
            lit.push(c);
        }
    }
    if !lit.is_empty() {
        pieces.push(Piece::Lit(lit));
    }
    pieces
}

/// An f-string's pieces: `{…}` is a hole, and `{{` and `}}` a plain brace.
fn f_string(inner: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                let source = hole(&mut chars);
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                }
                pieces.push(Piece::Hole(source));
            }
            _ => lit.push(c),
        }
    }
    if !lit.is_empty() {
        pieces.push(Piece::Lit(lit));
    }
    pieces
}

/// The source of an interpolation whose `{` was just read, up to its balancing `}`, which it
/// consumes.
fn hole(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut depth = 1usize;
    let mut source = String::new();
    for c in chars.by_ref() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        source.push(c);
    }
    source
}

/// The pieces of an argument's source text: each operand of a top-level `+`, in order. A string
/// gives its literal text and holes, a bare name a [`Piece::Name`], and anything else a hole.
pub fn pieces(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    for part in split(text, '+', false) {
        let part = unwrap_parens(part);
        match string(part) {
            Some(p) => out.extend(p),
            None if name(part) => out.push(Piece::Name(part.to_owned())),
            None => out.push(Piece::Hole(part.to_owned())),
        }
    }
    out
}

/// Whether `text` is a literal, and its text: a string with no hole.
pub fn literal(text: &str) -> Option<String> {
    match &pieces(text)[..] {
        [] => Some(String::new()),
        [Piece::Lit(s)] => Some(s.clone()),
        _ => None,
    }
}

/// What a route call's path names before any binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A canonical path, and the host its literal base names; `None` when its base isn't a
    /// literal, or it has none. `base` is the base's source text.
    Path { base: Option<String>, host: Option<String>, path: String },
    /// A bare name that only a call site binds: the whole URL, or, when `based`, the path after a
    /// base, whose host is `host` as above.
    Name { host: Option<String>, name: String, based: bool },
}

/// The path a route call's `path` text gives, with the `base` a pack set, if any: `None` when it
/// makes no route call.
pub fn target(path: &str, base: Option<&str>) -> Option<Target> {
    let mut pieces = pieces(path);
    let (base, host) = match base {
        Some(base) => {
            (Some(base.trim().to_owned()), literal(base).as_deref().and_then(url_host).map(str::to_owned))
        }
        None => match pieces.first() {
            None => return None,
            Some(Piece::Name(n)) if pieces.len() == 1 => {
                return Some(Target::Name { host: None, name: n.clone(), based: false });
            }
            // A leading operand or interpolation that isn't a literal is the base.
            Some(Piece::Name(source) | Piece::Hole(source)) => {
                let base = source.clone();
                pieces.remove(0);
                (Some(base), None)
            }
            Some(Piece::Lit(first)) if first.contains("://") => {
                let (base, host, rest) = split_url(&pieces)?;
                pieces = rest;
                (base, host)
            }
            Some(Piece::Lit(_)) => (None, None),
        },
    };
    let based = base.is_some();
    match &pieces[..] {
        [Piece::Name(n)] => Some(Target::Name { host, name: n.clone(), based }),
        [] => None,
        rest => Some(Target::Path { base, host, path: canonical(rest)? }),
    }
}

/// A URL's literal base and host, when its authority holds no hole, and the pieces of the path
/// that follows it.
fn split_url(pieces: &[Piece]) -> Option<(Option<String>, Option<String>, Vec<Piece>)> {
    let Some(Piece::Lit(first)) = pieces.first() else { return None };
    let after_scheme = first.find("://")? + 3;
    // The authority ends at the first `/`, `?` or `#` of the literal pieces.
    match first[after_scheme..].find(['/', '?', '#']) {
        Some(end) => {
            let base = &first[..after_scheme + end];
            let host = url_host(base).map(str::to_owned);
            let mut rest = vec![Piece::Lit(url_path(first).to_owned())];
            if first[after_scheme + end..].starts_with(['?', '#']) {
                rest = vec![Piece::Lit(format!("/{}", &first[after_scheme + end..]))];
            }
            rest.extend(pieces[1..].iter().cloned());
            Some((Some(base.to_owned()), host, rest))
        }
        None if pieces.len() == 1 => {
            Some((Some(first.clone()), url_host(first).map(str::to_owned), vec![Piece::Lit("/".into())]))
        }
        // A hole in the authority: no literal host, and the path starts at the next `/` after it.
        None => {
            let mut rest = Vec::new();
            let mut started = false;
            for piece in &pieces[1..] {
                match piece {
                    Piece::Lit(s) if !started => {
                        if let Some(i) = s.find('/') {
                            started = true;
                            rest.push(Piece::Lit(s[i..].to_owned()));
                        }
                    }
                    _ if started => rest.push(piece.clone()),
                    _ => {}
                }
            }
            Some((Some(first.clone()), None, if started { rest } else { vec![Piece::Lit("/".into())] }))
        }
    }
}

/// Where a hole sits in [`canonical`]'s text: a character no path the source writes holds.
const HOLE: char = '\u{0}';

/// The canonical path the pieces give, or `None` when it doesn't start with `/`: each segment
/// holding a hole becomes `{}` whole, the query and fragment are left out, and a trailing `/` is
/// dropped but at the root.
pub fn canonical(pieces: &[Piece]) -> Option<String> {
    let mut text = String::new();
    for piece in pieces {
        match piece {
            Piece::Lit(s) => text.extend(s.chars().filter(|c| *c != HOLE)),
            Piece::Hole(_) | Piece::Name(_) => text.push(HOLE),
        }
    }
    let text = text.split(['?', '#']).next().unwrap_or_default();
    if !text.starts_with('/') {
        return None;
    }
    let mut path: String =
        text.split('/').map(|s| if s.contains(HOLE) { "{}" } else { s }).collect::<Vec<_>>().join("/");
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    Some(path)
}

/// What a route call's method is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// A verb, in capitals.
    Verb(String),
    /// A bare identifier, which only a call site binds.
    Name(String),
    /// Anything else: it never joins.
    Unknown,
}

/// A verb, in capitals, when `text` names one in any case.
pub fn verb(text: &str) -> Option<String> {
    let upper = text.trim().to_ascii_uppercase();
    METHODS.contains(&upper.as_str()).then_some(upper)
}

/// The method a route call's `method` text names: none is `GET`.
pub fn method(text: Option<&str>) -> Method {
    let Some(text) = text.map(str::trim) else { return Method::Verb("GET".into()) };
    if let Some(lit) = literal(text).filter(|_| !name(text)) {
        return verb(&lit).map_or(Method::Unknown, Method::Verb);
    }
    if !name(text) {
        return Method::Unknown;
    }
    let last = last_segment(text);
    if let Some(v) = verb(last) {
        return Method::Verb(v);
    }
    if last == "PostForm" {
        return Method::Verb("POST".into());
    }
    if let Some(v) = last.strip_prefix("Method").and_then(verb) {
        return Method::Verb(v);
    }
    if ident(text) { Method::Name(text.to_owned()) } else { Method::Unknown }
}

/// One parameter of a definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    /// Its position among the parameters a call passes by position, or `None` when it binds only
    /// by keyword.
    pub position: Option<usize>,
    /// The text after its top-level `=`.
    pub default: Option<String>,
}

/// The modifiers a parameter's name may follow.
const MODIFIERS: [&str; 6] = ["private", "public", "protected", "readonly", "mut", "override"];

/// The parameters of a definition whose value is `value`, its parameter list as the source
/// writes it, the receiver aside.
pub fn params(value: &str) -> Vec<Param> {
    let mut out = Vec::new();
    let mut position = 0usize;
    let mut keyword_only = false;
    for (i, part) in split(unwrap_parens(value), ',', true).into_iter().enumerate() {
        let (head, default) = match top_level(part, &['='], true).into_iter().find(|&at| {
            // `=` alone: not `==`, `=>`, `<=`, `>=` or `!=`.
            let before = part[..at].chars().next_back();
            let after = part[at + 1..].chars().next();
            !matches!(before, Some('=' | '<' | '>' | '!')) && !matches!(after, Some('=' | '>'))
        }) {
            Some(at) => (part[..at].trim(), Some(part[at + 1..].trim().to_owned())),
            None => (part, None),
        };
        if head == "*" {
            keyword_only = true;
            continue;
        }
        if head == "/" || head.is_empty() {
            continue;
        }
        let spread = head.starts_with("...") || (head.starts_with('*') && !head.starts_with("**"));
        let kwargs = head.starts_with("**");
        let mut rest = head.trim_start_matches(['&', '*', '.']).trim_start();
        loop {
            let word = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or_default();
            if MODIFIERS.contains(&word) && rest[word.len()..].starts_with(char::is_whitespace) {
                rest = rest[word.len()..].trim_start().trim_start_matches(['&', '*']).trim_start();
            } else {
                break;
            }
        }
        let name: String =
            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
        if name.is_empty() {
            continue;
        }
        if i == 0 && matches!(name.as_str(), "self" | "cls" | "this") {
            continue;
        }
        if kwargs {
            continue;
        }
        if spread {
            keyword_only = true;
            continue;
        }
        let at = (!keyword_only).then_some(position);
        if !keyword_only {
            position += 1;
        }
        out.push(Param { name, position: at, default });
    }
    out
}

/// A call site's arguments, split once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    positional: Vec<String>,
    keyword: Vec<(String, String)>,
    /// A spread positional argument: every position from its own on is unknown.
    open_from: Option<usize>,
    /// A spread keyword argument: every keyword not passed by name is unknown.
    open_keywords: bool,
}

/// The arguments of a call whose value is `value`, its arguments as the source writes them.
pub fn args(value: &str) -> Args {
    let mut out = Args::default();
    for part in split(unwrap_parens(value), ',', false) {
        if part.starts_with("**") {
            out.open_keywords = true;
            continue;
        }
        if part.starts_with("...") || part.starts_with('*') {
            out.open_from.get_or_insert(out.positional.len());
            continue;
        }
        let keyword = top_level(part, &['='], false).into_iter().find(|&at| {
            ident(part[..at].trim())
                && !part[at + 1..].starts_with('=')
                && !part[..at].ends_with(['=', '!', '<', '>'])
        });
        match keyword {
            Some(at) => out.keyword.push((part[..at].trim().to_owned(), part[at + 1..].trim().to_owned())),
            None if out.open_from.is_none() => out.positional.push(part.to_owned()),
            None => {}
        }
    }
    out
}

/// What a call site passes for a parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound<'a> {
    /// The argument's text.
    Arg(&'a str),
    /// The call site leaves it out, and its default is a literal: that default's text.
    Default(&'a str),
    /// Unknown: left out with no literal default, or behind a spread argument.
    Unknown,
}

/// What `args` pass for the parameter `name` of `params`.
pub fn bind<'a>(params: &'a [Param], args: &'a Args, name: &str) -> Bound<'a> {
    let Some(param) = params.iter().find(|p| p.name == name) else { return Bound::Unknown };
    if let Some((_, value)) = args.keyword.iter().find(|(k, _)| *k == name) {
        return Bound::Arg(value);
    }
    if let Some(at) = param.position {
        if args.open_from.is_some_and(|from| at >= from) {
            return Bound::Unknown;
        }
        if let Some(value) = args.positional.get(at) {
            return Bound::Arg(value);
        }
    }
    if args.open_keywords || (param.position.is_some() && args.open_from.is_some()) {
        return Bound::Unknown;
    }
    match param.default.as_deref() {
        Some(default) if literal(default).is_some() && !name_only(default) => Bound::Default(default),
        _ => Bound::Unknown,
    }
}

/// Whether `text` is a bare name, which [`literal`] would read as no string.
fn name_only(text: &str) -> bool {
    name(text.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A call's text gives its base, host, method and path; a method or path that names a
    /// parameter waits for the call site that binds it, and never joins without one.
    #[test]
    fn a_calls_text_gives_its_host_method_and_path() {
        assert_eq!(method(Some("method")), Method::Name("method".into()));
        assert_eq!(method(Some("'POST'")), Method::Verb("POST".into()));
        assert_eq!(method(Some("opts.method")), Method::Unknown);
        assert_eq!(
            target("`${this.baseUrl}/orders`", None),
            Some(Target::Path { base: Some("this.baseUrl".into()), host: None, path: "/orders".into() })
        );
        assert_eq!(
            target("'http://orders:8080/orders/{id}?x=1'", None),
            Some(Target::Path {
                base: Some("http://orders:8080".into()),
                host: Some("orders".into()),
                path: "/orders/{id}".into()
            })
        );
        assert_eq!(
            target("f\"{base}/orders/{quote(id)}/events\"", None),
            Some(Target::Path { base: Some("base".into()), host: None, path: "/orders/{}/events".into() })
        );
        assert_eq!(
            target("url", None),
            Some(Target::Name { host: None, name: "url".into(), based: false }),
            "a parameter waits for its call site"
        );
        assert_eq!(
            target("self._base + path", None),
            Some(Target::Name { host: None, name: "path".into(), based: true })
        );
    }

    #[test]
    fn parameters_skip_the_receiver_and_keep_defaults() {
        let p = params("(self, method: str, path: str, body: dict[str, Any] | None = None)");
        let names: Vec<(&str, Option<usize>)> = p.iter().map(|p| (p.name.as_str(), p.position)).collect();
        assert_eq!(names, [("method", Some(0)), ("path", Some(1)), ("body", Some(2))]);
        assert_eq!(p[2].default.as_deref(), Some("None"));
        let go = params("(method, path string)");
        assert_eq!(go.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["method", "path"]);
        let ts = params(
            "(private readonly base: string, opts: Map<string, number> = new Map(), ...rest: string[])",
        );
        assert_eq!(ts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["base", "opts"]);
        let rust = params("(&mut self, path: &str)");
        assert_eq!(rust.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["path"]);
        let kw = params("(a, /, b, *, c='x')");
        assert_eq!(
            kw.iter().map(|p| (p.name.as_str(), p.position)).collect::<Vec<_>>(),
            [("a", Some(0)), ("b", Some(1)), ("c", None)]
        );
    }

    #[test]
    fn a_call_binds_by_keyword_position_and_literal_default() {
        let p = params("(self, path, method='GET', *, timeout=t)");
        let a = args("(\"/x\", timeout=3)");
        assert_eq!(bind(&p, &a, "path"), Bound::Arg("\"/x\""));
        assert_eq!(bind(&p, &a, "method"), Bound::Default("'GET'"));
        let a = args("(method='PUT', path=f\"/a/{b}\")");
        assert_eq!(bind(&p, &a, "method"), Bound::Arg("'PUT'"));
        assert_eq!(bind(&p, &a, "path"), Bound::Arg("f\"/a/{b}\""));
        let spread = args("(*xs)");
        assert_eq!(bind(&p, &spread, "path"), Bound::Unknown);
        assert_eq!(bind(&p, &spread, "method"), Bound::Unknown);
        let p = params("(path, method=DEFAULT)");
        assert_eq!(bind(&p, &args("('/x')"), "method"), Bound::Unknown, "a name is no literal default");
    }
}
