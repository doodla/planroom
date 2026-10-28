//! Prose in a parse: what neither a regex nor a query may read in a language with a built-in
//! grammar (DR-12). Prose is every comment node, and in Python every docstring: a string that is
//! the first statement of a module, a class body or a function body.
//!
//! The parse, never a lexer, decides it for these languages, so a lifetime, a character literal, a
//! regex literal or a multi-line string can't move where a comment starts or ends.
//!
//! A file's prose is read once, in one walk down its tree that carries what each node's ancestors
//! say about it, as sorted, disjoint byte ranges ([`Spans`]). A capture then finds the one span
//! that could hold it by a binary search, never by walking up its ancestors: tree-sitter finds a
//! node's parent by descending from the root, so a walk up costs time that grows with the square
//! of the depth.

use std::ops::Range;

use tree_sitter::Node;

/// Whether `node` is a comment. Every built-in grammar names its comment nodes with `comment`:
/// `comment`, Rust's `line_comment` and `block_comment`, and the doc-comment parts inside them.
fn is_comment(node: Node<'_>) -> bool {
    node.kind().contains("comment")
}

/// Whether `node` is an expression statement holding only a string: a docstring's shape.
fn is_string_statement(node: Node<'_>) -> bool {
    node.kind() == "expression_statement"
        && node.named_child_count() == 1
        && node.named_child(0).is_some_and(|c| matches!(c.kind(), "string" | "concatenated_string"))
}

/// The byte range of every outermost prose node under `root`: every comment, and when `python`,
/// every docstring.
pub fn spans(root: Node<'_>, python: bool) -> Spans {
    let mut out = Vec::new();
    // Each node, with whether it holds docstrings: a Python module, or the body of a class or a
    // function.
    let mut todo = vec![(root, python && root.kind() == "module")];
    while let Some((node, holds_docstrings)) = todo.pop() {
        if is_comment(node) {
            out.push(node.byte_range());
            continue;
        }
        let defines = python && matches!(node.kind(), "class_definition" | "function_definition");
        // Only the first named child that isn't a comment can be a docstring.
        let mut first = true;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if holds_docstrings && child.is_named() && !is_comment(child) {
                let docstring = first && is_string_statement(child);
                first = false;
                if docstring {
                    out.push(child.byte_range());
                    continue;
                }
            }
            todo.push((child, defines && child.kind() == "block"));
        }
    }
    Spans::of(out)
}

/// A file's prose spans, sorted and disjoint.
#[derive(Debug, Default)]
pub struct Spans(Vec<Range<usize>>);

impl Spans {
    /// `ranges`, the outermost prose nodes, sorted; one that another holds, such as a zero-width
    /// node at its edge, is dropped, so the rest are disjoint.
    fn of(mut ranges: Vec<Range<usize>>) -> Spans {
        ranges.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
        let mut spans: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for r in ranges {
            match spans.last_mut() {
                Some(last) if r.start < last.end || r.start == last.start => last.end = last.end.max(r.end),
                _ => spans.push(r),
            }
        }
        Spans(spans)
    }

    /// Whether a node with the byte range `bytes` is, or sits inside, prose: a span holds it. A
    /// zero-width node counts as the byte it stands at.
    pub fn hold(&self, bytes: &Range<usize>) -> bool {
        // The last span that starts at or before the node is the only one that could hold it.
        let after = self.0.partition_point(|r| r.start <= bytes.start);
        after.checked_sub(1).map(|k| &self.0[k]).is_some_and(|r| bytes.start < r.end && bytes.end <= r.end)
    }

    pub fn into_ranges(self) -> Vec<Range<usize>> {
        self.0
    }

    /// The spans, sorted and disjoint.
    pub fn ranges(&self) -> &[Range<usize>] {
        &self.0
    }

    /// Whether the file has no prose.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_holds_a_range_as_a_scan_of_every_span_finds() {
        let spans = Spans::of(vec![20..30, 3..5, 12..12, 40..41, 5..9]);
        let ranges = spans.0.clone();
        for start in 0..45 {
            for end in start..45 {
                let scan = ranges.iter().any(|r| r.start <= start && start < r.end && end <= r.end);
                assert_eq!(spans.hold(&(start..end)), scan, "{start}..{end}");
            }
        }
    }
}
