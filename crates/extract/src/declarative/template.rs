//! Emit templates: text with variables, and the references `from`, `to`, `parent` and `scope`
//! hold (architecture.md, "The extractor contract").
//!
//! Variables: `$file` (the repo-relative path), `$stem` (the file name without its last
//! extension), `$repo`, `$line` (the line the match starts on), `$end` (the line it ends on: the
//! last its captures, or a regex's whole match, reach), `$1`..`$9` (a regex's groups) and `@name`
//! (a tree-sitter query's captures). In a query extractor an `@` that names none of the query's captures is
//! text, so a literal such as `pkg:@acme/kit` stays as written. A `$` that names no variable is
//! refused, as is a group the regex doesn't have.
//!
//! A reference is `@alias:KEY`, `@ext:NAME` (a workspace-level `kernel.external` node, which goes
//! out as the alias `ext:NAME` for the engine to resolve), `@id:KIND:KEY` (the ID in this repo's
//! namespace, with the kind's `id_prefix`), or an ID `<namespace>:<kind-short>:<key>`. Anything
//! else, such as a bare `$file`, is no reference.

use planroom_contract::{NodeId, Ref};

use super::Why;
use crate::ids::node_id;

/// What a template may name besides the file's own variables.
#[derive(Debug, Clone, Copy)]
pub enum Names<'a> {
    /// A regex with this many groups.
    Regex { groups: usize },
    /// A tree-sitter query with these captures, by index.
    Query { captures: &'a [&'a str] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    File,
    Stem,
    Repo,
    Line,
    End,
    Group(usize),
    Capture(usize),
}

/// A text with variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template(Vec<Part>);

/// The longest template a pack may write. Every match walks its template's parts, which the work
/// budget doesn't count, so a pack can't stall a scan only while a template's cost per match is
/// bounded: past either bound the pack is refused at load.
pub const TEMPLATE_MAX_BYTES: usize = 1024;
/// The most variables a template may name; see [`TEMPLATE_MAX_BYTES`].
pub const TEMPLATE_MAX_VARS: usize = 64;

/// A variable of a template that can change within a file: the line, the end line, a regex's
/// group or a query's capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Var {
    Line,
    End,
    Group(usize),
    Capture(usize),
}

impl Var {
    /// What one match from line `line` to line `end` with `groups` or `captures` gives it.
    pub fn value<'t>(
        self,
        line: u32,
        end: u32,
        groups: &[Option<&'t str>],
        captures: &[Option<&'t str>],
    ) -> Val<'t> {
        match self {
            Var::Line => Val::Line(line),
            Var::End => Val::Line(end),
            Var::Group(n) => Val::Text(groups.get(n).copied().flatten()),
            Var::Capture(i) => Val::Text(captures.get(i).copied().flatten()),
        }
    }
}

/// What one match gives one of a template's variables that can change within a file: a group's
/// or a capture's text, or a line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Val<'t> {
    Text(Option<&'t str>),
    Line(u32),
}

impl Val<'_> {
    /// The bytes of its text.
    pub fn len(&self) -> usize {
        match self {
            Val::Text(t) => t.map_or(0, str::len),
            Val::Line(_) => 0,
        }
    }
}

/// What one match gives a template.
pub struct Env<'a> {
    pub repo: &'a str,
    pub file: &'a str,
    pub line: u32,
    /// The last line the match reaches: its captures', or a regex's whole match's.
    pub end: u32,
    /// A regex's groups, the whole match first; a query has none.
    pub groups: &'a [Option<&'a str>],
    /// A query's captures by index, each its first node's text; a regex has none.
    pub captures: &'a [Option<&'a str>],
}

impl Env<'_> {
    fn stem(&self) -> &str {
        let name = self.file.rsplit('/').next().unwrap_or(self.file);
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        }
    }
}

impl Template {
    /// `text` read against `names`.
    pub fn parse(text: &str, names: Names<'_>) -> Result<Template, Why> {
        if text.len() > TEMPLATE_MAX_BYTES {
            return Err(Why::TemplateTooLong);
        }
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut rest = text;
        while let Some(c) = rest.chars().next() {
            let part = match c {
                '$' => {
                    let after = &rest[1..];
                    let named = [
                        ("file", Part::File),
                        ("stem", Part::Stem),
                        ("repo", Part::Repo),
                        ("line", Part::Line),
                        ("end", Part::End),
                    ]
                    .into_iter()
                    .find(|(name, _)| after.starts_with(name));
                    if let Some((name, part)) = named {
                        Some((part, 1 + name.len()))
                    } else if let Some(n) =
                        after.chars().next().and_then(|d| d.to_digit(10)).filter(|n| *n >= 1)
                    {
                        let n = n as usize;
                        match names {
                            Names::Regex { groups } if n <= groups => Some((Part::Group(n), 2)),
                            Names::Regex { groups } => return Err(Why::UnknownGroup { group: n, groups }),
                            Names::Query { .. } => return Err(Why::UnknownVariable(format!("${n}"))),
                        }
                    } else {
                        let word: String =
                            after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                        return Err(Why::UnknownVariable(format!("${word}")));
                    }
                }
                '@' => match names {
                    Names::Query { captures } => {
                        let run: &str = {
                            let end = rest[1..]
                                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
                                .map_or(rest.len(), |i| i + 1);
                            &rest[1..end]
                        };
                        // The longest capture name the run starts with.
                        (1..=run.len())
                            .rev()
                            .filter(|n| run.is_char_boundary(*n))
                            .find_map(|n| captures.iter().position(|c| *c == &run[..n]).map(|i| (n, i)))
                            .map(|(n, i)| (Part::Capture(i), 1 + n))
                    }
                    Names::Regex { .. } => None,
                },
                _ => None,
            };
            match part {
                Some((part, len)) => {
                    if !literal.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut literal)));
                    }
                    parts.push(part);
                    rest = &rest[len..];
                }
                None => {
                    literal.push(c);
                    rest = &rest[c.len_utf8()..];
                }
            }
        }
        if !literal.is_empty() {
            parts.push(Part::Text(literal));
        }
        if parts.iter().filter(|p| !matches!(p, Part::Text(_))).count() > TEMPLATE_MAX_VARS {
            return Err(Why::TemplateTooLong);
        }
        Ok(Template(parts))
    }

    /// Whether it holds no variable.
    pub fn is_text(&self) -> bool {
        self.0.iter().all(|p| matches!(p, Part::Text(_)))
    }

    /// The text for one match, or `None` when it names a group or capture the match didn't set.
    pub fn expand(&self, env: &Env<'_>) -> Option<String> {
        // Sized up front, so the text holds no more heap than the capture budget charged for it.
        let mut out = String::with_capacity(self.size(env));
        for part in &self.0 {
            match part {
                Part::Text(t) => out.push_str(t),
                Part::File => out.push_str(env.file),
                Part::Stem => out.push_str(env.stem()),
                Part::Repo => out.push_str(env.repo),
                Part::Line => out.push_str(&env.line.to_string()),
                Part::End => out.push_str(&env.end.to_string()),
                Part::Group(n) => out.push_str(env.groups.get(*n).copied().flatten()?),
                Part::Capture(i) => out.push_str(env.captures.get(*i).copied().flatten()?),
            }
        }
        Some(out)
    }

    /// Each variable it names that can change within a file, onto `out`, once for each time it
    /// names one. With the template's own text and the file, their values fix what
    /// [`Template::expand`] gives.
    pub fn vars(&self, out: &mut Vec<Var>) {
        for part in &self.0 {
            match part {
                Part::Text(_) | Part::File | Part::Stem | Part::Repo => {}
                Part::Line => out.push(Var::Line),
                Part::End => out.push(Var::End),
                Part::Group(n) => out.push(Var::Group(*n)),
                Part::Capture(i) => out.push(Var::Capture(*i)),
            }
        }
    }

    /// The bytes [`Template::expand`] gives for one match, an unset variable counting as none and
    /// `$line` and `$end` as the most digits a line number has, without building the text.
    pub fn size(&self, env: &Env<'_>) -> usize {
        let value = |v: Option<&Option<&str>>| v.copied().flatten().map_or(0, str::len);
        self.0
            .iter()
            .map(|part| match part {
                Part::Text(t) => t.len(),
                Part::File => env.file.len(),
                Part::Stem => env.stem().len(),
                Part::Repo => env.repo.len(),
                // The digits of `u32::MAX`, the widest line number.
                Part::Line | Part::End => 10,
                Part::Group(n) => value(env.groups.get(*n)),
                Part::Capture(i) => value(env.captures.get(*i)),
            })
            .sum()
    }
}

/// A reference in `from`, `to`, `parent` or `scope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTemplate {
    Alias(Template),
    Ext(Template),
    /// `@id:KIND:KEY`, with the kind's ID prefix.
    Id {
        prefix: String,
        key: Template,
    },
    /// An ID spelled out, whose parts may hold variables.
    Literal(Template),
}

impl RefTemplate {
    /// `text` as a reference; `prefix` gives a kind's `id_prefix`, or `None` for an unknown kind.
    pub fn parse(
        text: &str,
        names: Names<'_>,
        prefix: impl Fn(&str) -> Option<String>,
    ) -> Result<RefTemplate, Why> {
        if let Some(rest) = text.strip_prefix("@alias:") {
            return Ok(RefTemplate::Alias(Template::parse(rest, names)?));
        }
        if let Some(rest) = text.strip_prefix("@ext:") {
            return Ok(RefTemplate::Ext(Template::parse(rest, names)?));
        }
        if let Some(rest) = text.strip_prefix("@id:") {
            let Some((kind, key)) = rest.split_once(':') else { return Err(Why::BareRef) };
            let Some(prefix) = prefix(kind) else { return Err(Why::UnknownKind(kind.to_owned())) };
            return Ok(RefTemplate::Id { prefix, key: Template::parse(key, names)? });
        }
        if text.starts_with('@') || text.split(':').count() < 3 {
            return Err(Why::BareRef);
        }
        Ok(RefTemplate::Literal(Template::parse(text, names)?))
    }

    /// Whether it names a node by its ID, as a `parent` and a hint's `scope` must.
    pub fn is_id(&self) -> bool {
        matches!(self, RefTemplate::Id { .. } | RefTemplate::Literal(_))
    }

    /// The reference for one match: `None` when a variable is unset or an ID isn't well formed.
    pub fn expand(&self, env: &Env<'_>) -> Option<Ref> {
        let filled = |t: &Template| t.expand(env).filter(|s| !s.is_empty());
        match self {
            RefTemplate::Alias(t) => filled(t).map(Ref::Alias),
            RefTemplate::Ext(t) => filled(t).map(|name| Ref::Alias(format!("ext:{name}"))),
            RefTemplate::Id { prefix, key } => {
                filled(key).map(|key| Ref::Id(node_id(env.repo, prefix, &key)))
            }
            RefTemplate::Literal(t) => filled(t)?.parse::<NodeId>().ok().map(Ref::Id),
        }
    }

    /// At least the bytes [`RefTemplate::expand`] gives for one match, without building it.
    pub fn size(&self, env: &Env<'_>) -> usize {
        match self {
            RefTemplate::Alias(t) | RefTemplate::Literal(t) => t.size(env),
            RefTemplate::Ext(t) => "ext:".len() + t.size(env),
            RefTemplate::Id { prefix, key } => env.repo.len() + prefix.len() + 2 + key.size(env),
        }
    }

    /// What [`Template::vars`] gives for the reference's template.
    pub fn vars(&self, out: &mut Vec<Var>) {
        match self {
            RefTemplate::Alias(t) | RefTemplate::Ext(t) | RefTemplate::Literal(t) => t.vars(out),
            RefTemplate::Id { key, .. } => key.vars(out),
        }
    }

    /// The node ID for one match, for a reference that names one.
    pub fn expand_id(&self, env: &Env<'_>) -> Option<NodeId> {
        match self.expand(env)? {
            Ref::Id(id) => Some(id),
            Ref::Alias(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(groups: &'a [Option<&'a str>], captures: &'a [Option<&'a str>]) -> Env<'a> {
        Env { repo: "web", file: "src/api/orders.client.ts", line: 7, end: 9, groups, captures }
    }

    #[test]
    fn variables_expand_and_text_stays() {
        let t = Template::parse("$repo/$stem@$line:$file#$1", Names::Regex { groups: 1 }).unwrap();
        assert!(!t.is_text());
        let groups = [Some("all"), Some("one")];
        assert_eq!(t.expand(&env(&groups, &[])).unwrap(), "web/orders.client@7:src/api/orders.client.ts#one");
        assert_eq!(t.expand(&env(&[Some("all"), None], &[])), None, "an unset group skips the emit");
        assert!(Template::parse("pkg:@acme/kit", Names::Regex { groups: 0 }).unwrap().is_text());
    }

    #[test]
    fn end_is_the_last_line_a_match_reaches() {
        let names = Names::Query { captures: &["test"] };
        let t = Template::parse("@test:$end from $line", names).unwrap();
        assert_eq!(t.expand(&env(&[], &[Some("test")])).unwrap(), "test:9 from 7");
        let mut vars = Vec::new();
        t.vars(&mut vars);
        assert_eq!(vars, [Var::Capture(0), Var::End, Var::Line]);
        assert_eq!(Var::End.value(7, 9, &[], &[]), Val::Line(9));
    }

    #[test]
    fn a_templates_size_is_what_it_expands_to_with_line_numbers_at_their_widest() {
        let names = Names::Regex { groups: 2 };
        let groups = [Some("all"), Some("redis"), None];
        let e = env(&groups, &[]);
        for text in ["$repo/$stem:$file#$1$1", "pkg:@acme/kit", "$1$2"] {
            let t = Template::parse(text, names).unwrap();
            let expanded = t.expand(&e).map_or(0, |s| s.len());
            assert!(text.contains('$') || t.size(&e) == text.len());
            if !text.contains("$2") {
                assert_eq!(t.size(&e), expanded, "{text}");
            }
        }
        for text in ["$line", "$end"] {
            assert_eq!(Template::parse(text, names).unwrap().size(&e), u32::MAX.to_string().len(), "{text}");
        }
        let prefix = |k: &str| (k == "code.file").then(|| "file".to_owned());
        for text in ["@id:code.file:$file", "@alias:host:$1", "@ext:$1", "ws:container:$1"] {
            let r = RefTemplate::parse(text, names, prefix).unwrap();
            let expanded = match r.expand(&e).unwrap() {
                Ref::Id(id) => id.0.len(),
                Ref::Alias(a) => a.len(),
            };
            assert!(r.size(&e) >= expanded, "{text}");
        }
    }

    #[test]
    fn unknown_variables_and_groups_are_refused() {
        assert_eq!(
            Template::parse("$2", Names::Regex { groups: 1 }),
            Err(Why::UnknownGroup { group: 2, groups: 1 })
        );
        assert_eq!(Template::parse("$files", Names::Regex { groups: 0 }).map(|t| t.is_text()), Ok(false));
        assert_eq!(
            Template::parse("$path", Names::Regex { groups: 0 }),
            Err(Why::UnknownVariable("$path".into()))
        );
        assert_eq!(
            Template::parse("$1", Names::Query { captures: &[] }),
            Err(Why::UnknownVariable("$1".into()))
        );
    }

    #[test]
    fn captures_take_the_longest_name_and_other_ats_are_text() {
        let names = Names::Query { captures: &["mod", "mod.name"] };
        let t = Template::parse("@mod.name/@acme", names).unwrap();
        assert_eq!(t.expand(&env(&[], &[Some("a"), Some("b")])).unwrap(), "b/@acme");
    }

    #[test]
    fn references_take_their_four_forms_and_a_bare_path_is_none() {
        let names = Names::Regex { groups: 1 };
        let prefix = |k: &str| (k == "code.file").then(|| "file".to_owned());
        let groups = [Some("x"), Some("redis")];
        let e = env(&groups, &[]);
        let id = RefTemplate::parse("@id:code.file:$file", names, prefix).unwrap();
        assert_eq!(id.expand(&e), Some(Ref::Id(NodeId("web:file:src/api/orders.client.ts".into()))));
        let alias = RefTemplate::parse("@alias:host:$1", names, prefix).unwrap();
        assert_eq!(alias.expand(&e), Some(Ref::Alias("host:redis".into())));
        assert!(!alias.is_id());
        let ext = RefTemplate::parse("@ext:$1", names, prefix).unwrap();
        assert_eq!(ext.expand(&e), Some(Ref::Alias("ext:redis".into())));
        let literal = RefTemplate::parse("ws:container:$1", names, prefix).unwrap();
        assert_eq!(literal.expand_id(&e), Some(NodeId("ws:container:redis".into())));
        for bare in ["$file", "$1", "@file", "src/main.ts", "@id:code.file"] {
            assert_eq!(RefTemplate::parse(bare, names, prefix), Err(Why::BareRef), "{bare}");
        }
        assert_eq!(
            RefTemplate::parse("@id:code.nope:$1", names, prefix),
            Err(Why::UnknownKind("code.nope".into()))
        );
    }
}
