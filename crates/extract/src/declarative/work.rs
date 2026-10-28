//! The work a grammar may spend on one file, counted in steps and never in time, so whether a file
//! is within it is the same on every machine ([`super::WORK_PER_BYTE`], [`super::WORK_FLOOR`]),
//! and the heap a file's facts may hold ([`capture_budget`]) and those of an extractor's run over
//! a repo ([`Held`]).
//!
//! Some inputs cost a grammar time that grows faster than the file: a file of block-comment opens
//! that never close makes the lexer read to the end of the file from each one, and a member chain
//! or a left-nested sum as deep as the file is long slows the query cursor at every node. Each
//! shows in a count tree-sitter's callbacks give: the parse reads its text through a callback in
//! [`READ_CHUNK`]-byte chunks and reports its progress every hundred parse actions, and the query
//! cursor reports its progress every thousand units of matching work, which grow with the matches
//! it holds open. The cursor also costs time for the depth of each node it visits, which no report
//! counts, so before a query runs the tree's depth is summed. A step is one chunk read, one parse
//! progress report, or [`DEPTH_STEP`] levels of that sum, and a query progress report is
//! [`QUERY_REPORT_STEPS`] steps, so a file's steps grow as its parse and query cost.
//!
//! A query's text predicates (`#match?`, `#eq?`, `#any-of?` and the rest) read each capture's
//! text before a match reaches the runner, and nested captures each hold their children's text,
//! so what they read can grow as the square of the file while the cursor's reports stay few. The
//! cursor reads that text only through [`Predicates`], which takes a step for every
//! [`READ_CHUNK`] bytes it hands over, as the parse does for each chunk it reads, and hands over
//! nothing once the budget is spent; the next progress report then stops the query. A predicate on
//! a node that nests, a call or a block, reads the file once for each level it nests, which an
//! ordinary file's budget holds; text that nests as deep as the file is long passes it.
//!
//! What a query's or a regex's matches hold in facts can grow as the square of a file too, so a
//! file's facts are charged against a budget of their own that grows with the file
//! ([`capture_budget`]), and what an extractor holds over a whole repo is bounded once more
//! ([`Held`]). A file's budget depends on its size alone, and what its matches charge on its
//! text, its path and its repo's name, which its facts copy; neither on the files beside it.

use std::cell::Cell;
use std::ops::ControlFlow;

use tree_sitter::{Node, ParseOptions, ParseState, Parser, Point, QueryCursorState, TextProvider, Tree};

/// How many bytes of the text the parse reads at a time.
pub const READ_CHUNK: usize = 64;

/// The steps a query progress report counts: each stands for a thousand units of the cursor's
/// matching work, where a parse report stands for a hundred parse actions.
pub const QUERY_REPORT_STEPS: u64 = 16;

/// The steps one file may take, and those it has taken.
pub struct Work {
    spent: Cell<u64>,
    budget: u64,
    /// The bytes handed to text predicates since their last step, fewer than [`READ_CHUNK`].
    unread: Cell<u64>,
}

impl Work {
    /// The budget of a file of `len` bytes: [`super::WORK_FLOOR`] plus [`super::WORK_PER_BYTE`] a
    /// byte.
    pub fn for_len(len: usize) -> Work {
        let len = u64::try_from(len).unwrap_or(u64::MAX);
        let budget = super::WORK_FLOOR.saturating_add(super::WORK_PER_BYTE.saturating_mul(len));
        Work { spent: Cell::new(0), budget, unread: Cell::new(0) }
    }

    /// Take a step for every [`READ_CHUNK`] bytes of text handed to a query's text predicates,
    /// carrying the rest to the next text handed over; whether the file has now taken more than
    /// its budget.
    fn read(&self, bytes: usize) -> bool {
        let chunk = READ_CHUNK as u64;
        let bytes = self.unread.get().saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
        self.unread.set(bytes % chunk);
        self.take(bytes / chunk).is_break()
    }

    /// What a query cursor reads the text of `text`'s nodes through, for its text predicates.
    pub fn predicates<'a>(&'a self, text: &'a [u8]) -> Predicates<'a> {
        Predicates { work: self, text }
    }

    /// Take `steps`; break once the file has taken more than its budget.
    fn take(&self, steps: u64) -> ControlFlow<()> {
        self.spent.set(self.spent.get().saturating_add(steps));
        if self.over() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    }

    /// Whether the file has taken more steps than its budget.
    pub fn over(&self) -> bool {
        self.spent.get() > self.budget
    }

    /// The steps taken so far.
    #[cfg(test)]
    pub fn spent(&self) -> u64 {
        self.spent.get()
    }

    /// A query cursor's progress callback: each report takes [`QUERY_REPORT_STEPS`], and the
    /// cursor stops once the budget is spent.
    pub fn query_progress(&self) -> impl FnMut(&QueryCursorState) -> ControlFlow<()> + '_ {
        |_| self.take(QUERY_REPORT_STEPS)
    }

    /// Take a step for every [`DEPTH_STEP`] levels of depth summed over `tree`'s nodes, which a
    /// query's cursor costs beside what its reports count, walking no further once the budget is
    /// spent; whether it is.
    pub fn depths(&self, tree: &Tree) -> bool {
        let mut cursor = tree.walk();
        let (mut depth, mut levels) = (0u64, 0u64);
        loop {
            levels += depth;
            if levels >= DEPTH_STEP {
                if self.take(levels / DEPTH_STEP).is_break() {
                    return true;
                }
                levels %= DEPTH_STEP;
            }
            if cursor.goto_first_child() {
                depth += 1;
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return self.over();
                }
                depth -= 1;
            }
        }
    }
}

/// The levels of depth, summed over a tree's nodes, that make one step. A query cursor's time
/// grows with the depth of each node it visits, so a tree as deep as its file is long costs time
/// quadratic in the file though its progress reports stay few.
pub const DEPTH_STEP: u64 = 64;

/// The text of a file's nodes as a query's text predicates read it ([`Work::predicates`]): every
/// [`READ_CHUNK`] bytes it hands over take a step of the file's work, and past the budget every
/// node's text is empty, so a predicate costs no more and the query stops at its next progress
/// report.
pub struct Predicates<'a> {
    work: &'a Work,
    text: &'a [u8],
}

impl<'a> TextProvider<&'a [u8]> for Predicates<'a> {
    type I = std::iter::Once<&'a [u8]>;

    fn text(&mut self, node: Node<'_>) -> Self::I {
        let bytes = self.text.get(node.byte_range()).unwrap_or_default();
        if self.work.over() || self.work.read(bytes.len()) {
            return std::iter::once(&[]);
        }
        std::iter::once(bytes)
    }
}

/// The bytes of heap one file's facts may hold: [`super::CAPTURE_FLOOR`] plus
/// [`super::CAPTURE_PER_BYTE`] a byte of the file. It depends on the file alone, never on the
/// files beside it.
pub fn capture_budget(len: usize) -> u64 {
    let len = u64::try_from(len).unwrap_or(u64::MAX);
    super::CAPTURE_FLOOR.saturating_add(super::CAPTURE_PER_BYTE.saturating_mul(len))
}

/// What an extractor's run holds in facts over one repo, against one bound for the repo: a
/// coverage cut like the walk's. Each file that gives facts adds its charge, which depends on
/// that file alone, its path included, in sorted path order; the first file whose charge would take the total past
/// the bound, and every file after it, give no fact.
pub struct Held {
    total: u64,
    max: u64,
    cut: bool,
}

impl Held {
    /// Nothing held yet, under a bound of `max` bytes.
    pub fn new(max: u64) -> Held {
        Held { total: 0, max, cut: false }
    }

    /// Whether an earlier file reached the bound, so no later file gives facts.
    pub fn cut(&self) -> bool {
        self.cut
    }

    /// Add a file's `charge`, unless the cut has fallen or the charge would take the total past
    /// the bound: then the cut falls here, and it returns false.
    pub fn add(&mut self, charge: u64) -> bool {
        match self.total.checked_add(charge) {
            Some(total) if total <= self.max && !self.cut => {
                self.total = total;
                true
            }
            _ => {
                self.cut = true;
                false
            }
        }
    }
}

/// `text` parsed by `parser`, taking steps from `work`: `None` when the parse failed or took more
/// than the budget, which [`Work::over`] then tells. Past the budget the text reads as ended, and
/// the next progress report stops the parse; the parser is reset, so it starts the next file
/// afresh.
pub fn parse(parser: &mut Parser, text: &str, work: &Work) -> Option<Tree> {
    let bytes = text.as_bytes();
    let mut read = |at: usize, _: Point| {
        if at >= bytes.len() || work.take(1).is_break() {
            &bytes[..0]
        } else {
            &bytes[at..bytes.len().min(at + READ_CHUNK)]
        }
    };
    let mut progress = |_: &ParseState| work.take(1);
    let options = ParseOptions::new().progress_callback(&mut progress);
    let tree = parser.parse_with_options(&mut read, None, Some(options));
    if work.over() {
        parser.reset();
        return None;
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(language: &str, text: &str) -> (Option<Tree>, Work) {
        let mut parser = Parser::new();
        parser.set_language(&crate::declarative::grammar::language(language).unwrap()).unwrap();
        let work = Work::for_len(text.len());
        (parse(&mut parser, text, &work), work)
    }

    #[test]
    fn a_parse_within_its_budget_reads_the_whole_text() {
        let text = "fn main() { let x = \"http://a/b\"; }\n".repeat(100);
        let (tree, work) = parsed("rust", &text);
        let tree = tree.expect("parsed");
        assert!(!tree.root_node().has_error());
        assert_eq!(tree.root_node().end_byte(), text.len());
        assert!(!work.over());
        assert!(work.spent() >= (text.len() / READ_CHUNK) as u64, "every chunk is a step");
    }

    #[test]
    fn predicate_text_takes_a_step_per_chunk_and_is_empty_past_the_budget() {
        let text = format!("x = \"{}\";\n", "a".repeat(100));
        let (tree, _) = parsed("javascript", &text);
        let tree = tree.expect("parsed");
        let string = tree.root_node().descendant_for_byte_range(4, 4).and_then(|n| n.parent()).unwrap();
        assert_eq!(string.kind(), "string");
        let work = Work::for_len(0);
        let mut predicates = work.predicates(text.as_bytes());
        let read: Vec<u8> = predicates.text(string).flatten().copied().collect();
        assert_eq!(read, &text.as_bytes()[string.byte_range()], "within the budget, the node's text");
        assert_eq!(work.spent(), 1, "a step per 64 bytes, 38 carried");
        let _ = predicates.text(string);
        assert_eq!(work.spent(), 3, "each time the text is handed over, with what was carried");
        // Spend the budget: from then on every node's text is empty, and nothing more is counted.
        assert!(work.take(crate::declarative::WORK_FLOOR).is_break());
        let spent = work.spent();
        assert!(predicates.text(string).flatten().next().is_none(), "past the budget, nothing");
        assert_eq!(work.spent(), spent);
    }

    #[test]
    fn a_files_capture_budget_is_its_own_floor_and_share() {
        use crate::declarative::{CAPTURE_FLOOR, CAPTURE_PER_BYTE};
        assert_eq!(capture_budget(0), CAPTURE_FLOOR);
        assert_eq!(capture_budget(1_000), CAPTURE_FLOOR + 1_000 * CAPTURE_PER_BYTE);
    }

    #[test]
    fn the_repo_bound_cuts_at_the_first_file_past_it_and_stays_cut() {
        let mut held = Held::new(100);
        assert!(held.add(60) && held.add(40), "up to the bound");
        assert!(!held.cut());
        assert!(!held.add(1), "past it");
        assert!(held.cut());
        assert!(!held.add(0), "every file after the cut, however little it holds");
    }

    #[test]
    fn a_parse_past_its_budget_stops_and_the_parser_starts_the_next_text_afresh() {
        let mut parser = Parser::new();
        parser.set_language(&crate::declarative::grammar::language("go").unwrap()).unwrap();
        let costly = "/* ".repeat(3_000);
        let work = Work::for_len(costly.len());
        assert!(parse(&mut parser, &costly, &work).is_none());
        assert!(work.over());
        // The same steps every time: the count depends on the text alone.
        let again = Work::for_len(costly.len());
        assert!(parse(&mut parser, &costly, &again).is_none());
        assert_eq!(work.spent(), again.spent());
        let fine = "package main\n\nfunc main() {}\n";
        let tree = parse(&mut parser, fine, &Work::for_len(fine.len())).expect("parsed");
        assert!(!tree.root_node().has_error(), "{}", tree.root_node().to_sexp());
    }
}
