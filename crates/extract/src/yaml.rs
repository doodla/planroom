//! A YAML tree with 1-based line numbers, built from `saphyr-parser` events, with caps on nesting
//! depth and on alias expansion so a hostile file can't exhaust memory.

use std::collections::HashMap;

use saphyr_parser::{Event, Parser};

/// Nesting deeper than this is refused.
pub const MAX_DEPTH: usize = 64;
/// Nodes in a document, counting every alias expansion, beyond which it is refused.
pub const MAX_NODES: usize = 100_000;

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

    fn size(&self) -> usize {
        match self {
            Yaml::Scalar { .. } => 1,
            Yaml::Seq { items, .. } => 1 + items.iter().map(Yaml::size).sum::<usize>(),
            Yaml::Map { entries, .. } => 1 + entries.iter().map(|(k, v)| k.size() + v.size()).sum::<usize>(),
        }
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
    let mut nodes = 0usize;
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|e| YamlError { line: e.marker().line() as u32, message: e.to_string() })?;
        let line = span.start.line() as u32;
        let err = |message: &str| YamlError { line, message: message.to_owned() };
        let done = match event {
            Event::DocumentStart(_) => {
                anchors.clear();
                nodes = 0;
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
                let node = anchors.get(&id).cloned().ok_or_else(|| err("an alias names no anchor"))?;
                nodes += node.size();
                Some((node, 0))
            }
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => None,
        };
        if stack.len() > MAX_DEPTH {
            return Err(err("nested too deeply"));
        }
        let Some((node, anchor)) = done else { continue };
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(err("too many nodes, aliases expanded"));
        }
        if anchor > 0 {
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
