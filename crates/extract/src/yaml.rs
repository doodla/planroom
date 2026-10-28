//! A YAML tree with 1-based line numbers, built from `saphyr-parser` events, with caps on nesting
//! depth and on what anchors and aliases copy, so a hostile file can't exhaust memory. JSON is
//! YAML too, so the same tree, with its lines, reads a JSON file.
//!
//! The tree holds each node the text writes once, so those are bounded by the text itself. An
//! anchor keeps a copy of its node, and each alias copies it again: those copies are what could
//! grow past any file's size, so the copies of all a text's documents are counted, in nodes and
//! in the bytes of their scalars, and the text is refused before a copy would take either past
//! its cap.

use std::collections::HashMap;

use saphyr_parser::{Event, Parser};

/// Nesting deeper than this is refused.
pub const MAX_DEPTH: usize = 64;
/// The nodes a document's anchors and aliases may copy, all told, beyond which it is refused.
pub const MAX_NODES: usize = 100_000;
/// The bytes of scalar text a document's anchors and aliases may copy, all told, beyond which it
/// is refused: one alias of a long string copies all of it.
pub const MAX_COPIED_BYTES: usize = 1 << 20;

/// One YAML node and the line it starts on.
#[derive(Debug, Clone, PartialEq)]
pub enum Yaml {
    Scalar { value: String, line: u32 },
    Seq { items: Vec<Yaml>, line: u32 },
    Map { entries: Vec<(Yaml, Yaml)>, line: u32 },
}

impl Yaml {
    pub fn line(&self) -> u32 {
        match self {
            Yaml::Scalar { line, .. } | Yaml::Seq { line, .. } | Yaml::Map { line, .. } => *line,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Yaml::Scalar { value, .. } => Some(value),
            _ => None,
        }
    }

    /// A mapping's value for a scalar key.
    pub fn get(&self, key: &str) -> Option<&Yaml> {
        self.entries()?.iter().find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v)
    }

    pub fn entries(&self) -> Option<&[(Yaml, Yaml)]> {
        match self {
            Yaml::Map { entries, .. } => Some(entries),
            _ => None,
        }
    }
}

/// What a document's anchors and aliases have copied so far.
#[derive(Debug, Default)]
struct Copied {
    nodes: usize,
    bytes: usize,
}

impl Copied {
    /// Count one more copy of `node`, or say the document copies too much. The walk stops as
    /// soon as either count passes its cap, so a refused copy costs no more than the cap.
    fn charge(&mut self, node: &Yaml) -> bool {
        let mut todo = vec![node];
        while let Some(n) = todo.pop() {
            self.nodes += 1;
            match n {
                Yaml::Scalar { value, .. } => self.bytes += value.len(),
                Yaml::Seq { items, .. } => todo.extend(items),
                Yaml::Map { entries, .. } => todo.extend(entries.iter().flat_map(|(k, v)| [k, v])),
            }
            if self.nodes > MAX_NODES || self.bytes > MAX_COPIED_BYTES {
                return false;
            }
        }
        true
    }
}

/// Why a document didn't load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub line: u32,
    pub message: String,
}

enum Open {
    Seq { items: Vec<Yaml>, line: u32, anchor: usize },
    Map { entries: Vec<(Yaml, Yaml)>, key: Option<Yaml>, line: u32, anchor: usize },
}

/// Every document in `text`.
pub fn parse(text: &str) -> Result<Vec<Yaml>, YamlError> {
    let mut docs = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut anchors: HashMap<usize, Yaml> = HashMap::new();
    // Counted over the whole text, every document's together: the documents are all kept, so a
    // count per document would let each of many small documents copy up to the caps.
    let mut copied = Copied::default();
    let too_many = "too many nodes, aliases expanded";
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|e| YamlError { line: e.marker().line() as u32, message: e.to_string() })?;
        let line = span.start.line() as u32;
        let err = |message: &str| YamlError { line, message: message.to_owned() };
        let done = match event {
            Event::DocumentStart(_) => {
                anchors.clear();
                None
            }
            Event::SequenceStart(anchor, _) => {
                stack.push(Open::Seq { items: Vec::new(), line, anchor });
                None
            }
            Event::MappingStart(anchor, _) => {
                stack.push(Open::Map { entries: Vec::new(), key: None, line, anchor });
                None
            }
            Event::SequenceEnd => match stack.pop() {
                Some(Open::Seq { items, line, anchor }) => Some((Yaml::Seq { items, line }, anchor)),
                _ => return Err(err("unbalanced sequence")),
            },
            Event::MappingEnd => match stack.pop() {
                Some(Open::Map { entries, line, anchor, .. }) => Some((Yaml::Map { entries, line }, anchor)),
                _ => return Err(err("unbalanced mapping")),
            },
            Event::Scalar(value, _, anchor, _) => {
                Some((Yaml::Scalar { value: value.into_owned(), line }, anchor))
            }
            Event::Alias(id) => {
                let node = anchors.get(&id).ok_or_else(|| err("an alias names no anchor"))?;
                // Counted before it is copied, so a copy past the caps is never made.
                if !copied.charge(node) {
                    return Err(err(too_many));
                }
                Some((node.clone(), 0))
            }
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => None,
        };
        if stack.len() > MAX_DEPTH {
            return Err(err("nested too deeply"));
        }
        let Some((node, anchor)) = done else { continue };
        if anchor > 0 {
            if !copied.charge(&node) {
                return Err(err(too_many));
            }
            anchors.insert(anchor, node.clone());
        }
        match stack.last_mut() {
            None => docs.push(node),
            Some(Open::Seq { items, .. }) => items.push(node),
            Some(Open::Map { entries, key, .. }) => match key.take() {
                None => *key = Some(node),
                Some(k) => entries.push((k, node)),
            },
        }
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_carry_their_lines() {
        let docs = parse("# c\nname: shop\nservices:\n  a:\n    image: x:1\n    environment:\n      - K=V\n")
            .unwrap();
        let doc = &docs[0];
        assert_eq!(doc.get("name").unwrap().line(), 2);
        let a = doc.get("services").unwrap().get("a").unwrap();
        assert_eq!(a.get("image").unwrap().as_str(), Some("x:1"));
        assert_eq!(a.get("image").unwrap().line(), 5);
        let Some(Yaml::Seq { items, .. }) = a.get("environment") else { panic!("a sequence") };
        assert_eq!(items[0].line(), 7);
    }

    #[test]
    fn aliases_expand_within_the_cap() {
        let docs = parse("base: &b {k: v}\nother: *b\n").unwrap();
        assert_eq!(docs[0].get("other").unwrap().get("k").unwrap().as_str(), Some("v"));
    }

    #[test]
    fn an_alias_bomb_is_refused() {
        let mut text = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
        for i in 1..12 {
            let refs = vec![format!("*a{}", i - 1); 10].join(", ");
            text.push_str(&format!("a{i}: &a{i} [{refs}]\n"));
        }
        let e = parse(&text).unwrap_err();
        assert!(e.message.contains("too many nodes"), "{e:?}");
    }

    #[test]
    fn an_alias_of_a_long_string_is_refused_before_it_copies() {
        // Each alias of a 200 KB string would copy it: 100,000 of them hold 20 GB.
        let mut text = format!("big: &big \"{}\"\nmany: [", "x".repeat(200_000));
        text.push_str(&"*big, ".repeat(100_000));
        text.push_str("]\n");
        let e = parse(&text).unwrap_err();
        assert!(e.message.contains("too many nodes"), "{e:?}");
        // A few copies of it fit.
        let few = format!("big: &big \"{}\"\nmany: [*big, *big]\n", "x".repeat(200_000));
        let Some(Yaml::Seq { items, .. }) = parse(&few).unwrap()[0].get("many").cloned() else {
            panic!("a list")
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn many_documents_share_one_copy_budget() {
        // Each document alone copies 400 KB, within the cap; three of them together pass it.
        let doc = format!("a: &a \"{}\"\nb: [*a, *a, *a]\n", "x".repeat(100_000));
        assert!(parse(&doc).is_ok());
        let e = parse(&[doc.as_str(); 3].join("---\n")).unwrap_err();
        assert!(e.message.contains("too many nodes"), "{e:?}");
    }

    #[test]
    fn a_document_large_without_aliases_is_read_whole() {
        // More nodes than the copy cap, none of them copied.
        let text = format!("[{}]", vec!["1"; MAX_NODES + 10].join(","));
        let docs = parse(&text).unwrap();
        let Yaml::Seq { items, .. } = &docs[0] else { panic!("a sequence") };
        assert_eq!(items.len(), MAX_NODES + 10);
    }

    #[test]
    fn json_reads_with_its_lines() {
        let docs =
            parse("{\n  \"name\": \"@acme/kit\",\n  \"deps\": {\"a\":\"1\", \"b\": \"\\u00e9\\/\"}\n}\n")
                .unwrap();
        assert_eq!(docs[0].get("name").map(|n| (n.as_str(), n.line())), Some((Some("@acme/kit"), 2)));
        let deps = docs[0].get("deps").unwrap();
        assert_eq!(deps.get("a").and_then(Yaml::as_str), Some("1"));
        assert_eq!(deps.get("b").and_then(Yaml::as_str), Some("é/"));
    }

    #[test]
    fn deep_nesting_is_refused() {
        let text = format!("{}x{}", "[".repeat(MAX_DEPTH + 2), "]".repeat(MAX_DEPTH + 2));
        assert!(parse(&text).unwrap_err().message.contains("deeply"));
    }

    #[test]
    fn a_syntax_error_names_its_line() {
        let e = parse("a: b\nc: [d\n").unwrap_err();
        assert!(e.line >= 2, "{e:?}");
    }
}
