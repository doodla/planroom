//! The declarative runner: regex and tree-sitter extractors read from pack data, emitting nodes,
//! edges and hints with evidence (architecture.md, "The extractor contract"; DR-9, DR-46, DR-47).
//!
//! A pack declares an extractor with `files`, a `regex` or a `language` and `query`, and `emit`
//! templates. [`Declarative::compile`] reads one declaration against the loaded vocabulary, the
//! merged `[[languages]]` table and the declared hint kinds, and refuses what it couldn't run with
//! an [`Invalid`], which the pack loader reports as `pack_invalid`. The loader compiles every
//! regex extractor this way at load, and every query extractor this way once in a process that
//! extracts, before it reads a file, so a pack whose extractors compile runs.
//!
//! [`Declarative::run`] keeps the extractor contract:
//!
//! - **Deterministic.** Files are read in sorted path order and matches in text order; nodes,
//!   edges and hints come out sorted by ID, and a fact two matches emit holds both lines.
//! - **Tolerant.** A file that isn't UTF-8 or wasn't read, a parse with error nodes, a query that
//!   reached its match limit, and a captured prop that doesn't fit its schema each make the file a
//!   `partial` entry; the rest of its facts stand. A file a bound refuses, its nesting, its work,
//!   the heap its facts would hold, or the repo's bound on what an extractor holds, is a
//!   `partial` entry with that bound as its reason, and gives no fact. Where a grammar's parse
//!   has errors, no fact comes from an error region: an error node, or around a missing node the
//!   smallest statement-level node. A regex match or a query capture that overlaps one is
//!   dropped.
//! - **Bounded.** It reads only the files it is given, which the walk took within its bounds, in
//!   sorted order; the regex crate runs in linear time, and a query's in-progress matches are
//!   capped. A file in a language with a grammar is parsed only when its brackets and its
//!   indentation nest no deeper than [`NESTING_LIMIT`]; a deeper one is partial and gives no fact.
//!   Its parse and its query take steps from a budget that grows with its size ([`WORK_PER_BYTE`],
//!   [`WORK_FLOOR`]), counted from tree-sitter's callbacks, the tree's summed depth and the text a
//!   query's text predicates read, never a clock ([`work`]); a file that takes more is stopped
//!   there, partial with reason `work_limit`, and gives no fact. The heap a file's matches make
//!   their facts hold, their templates' text, and each new fact, each further line of evidence
//!   and each props or attrs map with its structure ([`HINT_COST`], [`NODE_COST`],
//!   [`EVIDENCE_COST`], [`MAP_COST`]), is charged against the file's own budget,
//!   [`CAPTURE_PER_BYTE`] a byte of it plus [`CAPTURE_FLOOR`], and so is what the run keeps of
//!   each distinct match until it emits, and of each fact's ID and cited lines to find their
//!   repeats, as it keeps it ([`MATCH_COST`], [`MATCH_VALUE_COST`], [`FACT_COST`],
//!   [`FACT_VALUE_COST`], [`CITE_COST`]). A file whose matches would charge more is partial with
//!   reason `work_limit` and gives no fact. Which facts a file gives, and whether it gives any,
//!   depends on the file alone, never on the files beside it: on its text, and on its path and
//!   its repo's name, which its facts' evidence and IDs copy, so the same text at a longer path
//!   charges more and can pass a budget it kept within at a shorter one. What a run
//!   holds over the whole repo is bounded once more, by a coverage cut like the walk's: in sorted
//!   path order each file that gives facts adds its charge to a total, and the first file whose
//!   charge would take the total past [`REPO_CAPTURE_MAX`], and every file after it, are partial
//!   with reason `repo_limit` and give no fact.
//!   Each match finds its line, the one error region it could overlap, the one prose span that
//!   could hold it and its fact's earlier evidence by a lookup, never a pass over the file, the
//!   fact or a node's ancestors, so a file costs time linear in its size and no input stalls a
//!   scan.
//! - **Code, not prose.** A regex runs over the file with its comments and docstrings blanked. In a
//!   language with a built-in grammar the parse names them: every comment node, and Python's
//!   docstrings ([`prose`]), outside the error regions no fact comes from. In any other language
//!   the `[[languages]]` comment marks drive a lexer that errs toward blanking ([`comments`]). A
//!   file in no declared language has no comment marks, so a regex reads it whole. A tree-sitter
//!   match that captures a node in a comment or a docstring is dropped, and a capture's text, and
//!   what a text predicate reads, is the file with its prose blanked, so a capture that holds a
//!   comment, such as a call's arguments, carries none of its text. So a comment or a docstring
//!   never creates a node, an edge or a hint, nor puts its text in one.
//! - **Evidence.** Every fact cites the line its match starts on, as `Evidence::Line` with the
//!   exact source line at the commit read.

pub(crate) mod charge;
mod coerce;
mod comments;
pub mod grammar;
mod nesting;
mod prose;
mod template;
mod work;

pub use template::{TEMPLATE_MAX_BYTES, TEMPLATE_MAX_VARS};

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use planroom_contract::{
    EmitTemplate, ErrorCode, Evidence, ExtractorDecl, FACTS_FORMAT, FactBatch, Hint, Kind, LanguageDecl,
    Layer, Level, Node, NodeId, PartialFile, PartialReason, Provenance, Tier, Vocabulary,
};
use serde_json::Value;
use tree_sitter::{Parser, Query, QueryCursor, QueryCursorOptions, StreamingIterator};

use crate::ids::{edge_id, hint_id, node_id};
use crate::{RepoPath, RepoView, compile_regex, lines};
use coerce::Prop;
pub use coerce::{Fault, Form, breaks, fault};
pub use comments::Marks;
use template::{Env, Names, RefTemplate, Template, Val, Var};

/// At most this many in-progress matches per query run, so a query can't grow without bound on a
/// pathological file. Reaching it makes the file partial; the cut depends only on the file.
pub const MATCH_LIMIT: u32 = 1024;

/// The deepest nesting of brackets, and the widest indentation in columns, of a file the runner
/// parses with a grammar. A deeper file isn't parsed: it is partial, with
/// [`PartialReason::NestingLimit`], and gives the extractor no fact. The bound keeps Python's
/// scanner within the state it may save, which an indentation over about 380 levels overflows,
/// and bounds the depth of every parse tree.
pub const NESTING_LIMIT: usize = 256;

/// The steps a grammar may take over a file, per byte of it, beside [`WORK_FLOOR`]: a step is a
/// 64-byte chunk the parse reads, one of its progress reports, 64 levels of the tree's depth summed
/// over its nodes, a sixteenth of a query progress report, or 64 bytes of the text a query's text
/// predicates read ([`work`]). Of some 20,000 files of Go, Python, JavaScript, TypeScript and Rust
/// from their standard libraries and packages, none took more than 1.7 steps a byte for a parse
/// and a query without predicates, the most a Go test file built to nest deeply; JavaScript,
/// minified included, took at most two thirds. A `#match?` on every call or every block cut no
/// file of some 900 Rust crates, 180 JavaScript packages or Go's or Python's standard library.
/// A file that takes more than its budget is partial,
/// with [`PartialReason::WorkLimit`], and gives the extractor no fact. The count depends only on
/// the file, so the cut is the same on every machine.
pub const WORK_PER_BYTE: u64 = 2;

/// The steps every file may take whatever its size, so a small file is never cut.
pub const WORK_FLOOR: u64 = 1 << 14;

/// The bytes of heap a file's facts may hold, per byte of the file, beside [`CAPTURE_FLOOR`]. A
/// match is charged every template's expanded text and each entry it fills, and each fact it
/// makes that no earlier match of its file made, with the fact's evidence, its path, its cited
/// line and its commit, its structure ([`HINT_COST`], [`NODE_COST`]) and its props or attrs map
/// ([`MAP_COST`]); a node's or an edge's further line of evidence is charged that line and
/// [`EVIDENCE_COST`]. A match that repeats an earlier one's values on its line makes no new fact
/// and is charged nothing. Each distinct match is also charged what the run keeps of it until
/// the file's matches are emitted ([`MATCH_COST`], [`MATCH_VALUE_COST`]), and each new fact and
/// each further cited line what the run keeps to find their repeats ([`FACT_COST`],
/// [`FACT_VALUE_COST`], [`CITE_COST`]), as it keeps it, so what the run keeps of a file's
/// matches and facts together never passes its budget. Nested captures each hold their
/// children's text, so a file's matches can hold many times its size. A file whose matches
/// would charge more than its budget is partial, with [`PartialReason::WorkLimit`], and gives no
/// fact. The budget is the file's own, so whether a file gives facts depends on the file alone:
/// its text, and its path and its repo's name, which its facts copy. Before kept matches were
/// charged, a query keeping every string was charged 2.6 a byte over some 900 Rust crates, 3.1
/// over Go's standard library, 3.6 over some 180 JavaScript packages and 5.7 over Python's
/// standard library, and one keeping every call, member access and string, nested ones
/// included, 7.4 over the crates, 13.7 over Python's, 14.5 over Go's and 18.1 over the
/// JavaScript packages. Dense everyday files charge more, kept matches and the facts' index
/// included: a URL on every line under a hint and an edge 22 to 28 a byte, two strings a line
/// under a strings query 29 to 45, a URL with its port and path on every line under a hint with
/// four attributes 60, and every member access of a line of chained calls 58 to 82. Of 20,000 files of Rust crates, 3,000 of Python packages and 3,400 of
/// JavaScript packages, under a query keeping every call, member access and string, none passed
/// its budget. The budget bounds one file, at most 128 MiB for the walk's largest;
/// [`REPO_CAPTURE_MAX`] bounds what a repo's run holds. So the ratio keeps such files, with
/// room, and refuses what grows past any file's size, such as captures that each hold one long
/// string again.
pub const CAPTURE_PER_BYTE: u64 = 128;

/// The bytes of heap every file's facts may hold whatever its size.
pub const CAPTURE_FLOOR: u64 = 1 << 14;

/// The most heap an extractor's run over one repo charges for its facts, a coverage cut like the
/// walk's (DR-47): in sorted path order each file that gives facts adds its charge, and the first
/// file whose charge would take the total past it, and every file after it, are partial with
/// [`PartialReason::RepoLimit`] and give no fact. The walk's 256 MiB of files at the 128 bytes a
/// byte of [`CAPTURE_PER_BYTE`] could hold 32 GiB.
pub const REPO_CAPTURE_MAX: u64 = 1 << 28;

/// The bytes the capture budget charges each new hint a run's matches make, beside the text they
/// copy: its structure, its evidence's, its place in the batch's map and what the allocator adds
/// to each allocation. A hint whose texts are a few bytes held 503 such bytes in its batch.
pub const HINT_COST: u64 = 512;

/// What [`HINT_COST`] is for a new node or edge. A node held 802 such bytes in its batch and an
/// edge 854.
pub const NODE_COST: u64 = 896;

/// The bytes the capture budget charges each line of evidence a match adds to a node or an edge
/// that already has its ID, beside its text: its structure, room for its list to grow, and what
/// the allocator adds. A line held 155 such bytes in a list with little room left.
pub const EVIDENCE_COST: u64 = 288;

/// The bytes the capture budget charges a new fact for each node of its props' or its attrs' map,
/// beside the entries' text ([`ENTRY_COST`]): a map's first entry allocates a B-tree node with
/// room for eleven keys and values, 1,072 bytes for a leaf and 1,168 for an inner node, and what
/// the allocator adds. A map of more than eleven entries is charged a node for every five, the
/// fewest a node holds after a split, and one more for every five of those.
pub const MAP_COST: u64 = 1216;

/// The bytes a JSON value takes in its place in a map or a list, as [`fixed_size`] counts it.
const VALUE_SIZE: usize = 72;

/// What [`MAP_COST`] is for a node's list of aliases: its first allocation holds four, which held
/// 112 bytes; each further alias's room is in its [`ENTRY_COST`].
const LIST_COST: u64 = 128;

/// The bytes [`Declarative::copies`] counts for each prop, attribute or alias an emit fills, and
/// each optional text it gives (a hint's value, a node's name or parent, an edge's via), beside
/// its text: the allocation that holds it, and its room in the fact's list.
const ENTRY_COST: usize = 64;

/// The heap a map of `entries` entries holds beside them: [`MAP_COST`] for each of its nodes.
fn map_heap(entries: usize) -> u64 {
    let nodes = match entries {
        0 => 0,
        1..=11 => 1,
        _ => {
            let leaves = entries.div_ceil(5);
            leaves + leaves.div_ceil(5)
        }
    };
    MAP_COST.saturating_mul(u64::try_from(nodes).unwrap_or(u64::MAX))
}

/// The longest values, in bytes, of a match or of a fact's ID that the runner compares with
/// earlier ones to find a repeat; longer ones count as new, as their text alone costs more than
/// a fact's structure.
const SAME_MAX: usize = 1 << 10;

/// The bytes the capture budget charges each distinct match a file's run keeps until it emits,
/// beside its values ([`MATCH_VALUE_COST`]): its entry in the list of kept matches, 32 bytes with
/// room for the list to grow and to move as it grows, and its entry in the index that finds a
/// repeat, 40 bytes and a control byte with the table's spare room and its old table while it
/// grows, and the two allocations of its values. A match whose values are too long to compare
/// has no index entry, and is charged [`MATCH_COST`] all the same.
pub const MATCH_COST: u64 = 288;

/// The bytes the capture budget charges for each value, a regex group or a query capture, of a
/// distinct match the run keeps: 16 bytes in the kept match's list of values and 16 in the
/// index's copy of it.
pub const MATCH_VALUE_COST: u64 = 32;

/// The bytes the capture budget charges each fact a file's run tells apart from the others by
/// the values its ID takes ([`Declarative::keep`]), beside those values ([`FACT_VALUE_COST`]):
/// its entry in the index of the file's facts, 40 bytes and a control byte with the table's
/// spare room and its old table while it grows, and the allocation of its values. Charged to the
/// file's budget alone: a node's or an edge's until the file is read, a hint's while the run
/// reads its line, since a hint's ID holds its line and the run drops the line's index after it.
pub const FACT_COST: u64 = 160;

/// The bytes the capture budget charges for each value a fact's ID takes in that index: one for
/// each variable its templates name, however many times they name it.
pub const FACT_VALUE_COST: u64 = 24;

/// The bytes the capture budget charges each line a node or an edge cites in the file being
/// read, in the set that finds a repeated line: 16 bytes and a control byte, with the table's
/// spare room and its old table while it grows.
pub const CITE_COST: u64 = 64;

/// A hint's ID: 12 hex characters.
const HINT_ID_LEN: usize = 12;

/// An edge's ID: `e:` and 12 hex characters.
const EDGE_ID_LEN: usize = 14;

/// What one declarative extractor is compiled against.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// The ID of the pack that declares it: its name is `<pack>/<id>`.
    pub pack: &'a str,
    pub decl: &'a ExtractorDecl,
    /// The loaded vocabulary: each kind's ID prefix, level and prop schema, each relation's.
    pub vocabulary: &'a Vocabulary,
    /// The merged `[[languages]]` table: each file's comment marks, and a query's grammar.
    pub languages: &'a [LanguageDecl],
    /// Every loaded `[[hints]]` kind, qualified, with the attributes it declares.
    pub hints: &'a BTreeMap<String, Vec<String>>,
    /// A query extractor's `.scm` text, which the pack holds under `queries/`.
    pub query: Option<&'a str>,
}

/// One step of where a refused value sits under its `[[extractors]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Key(String),
    Index(usize),
}

/// Why a declaration can't run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// It is a built-in's declaration, which [`crate::builtin`] runs.
    NotDeclarative,
    /// A reference that is none of `@alias:`, `@ext:`, `@id:KIND:KEY` or an ID, such as `$file`.
    BareRef,
    /// A `parent` or a hint's `scope` must name a node by its ID, never an alias.
    NotAnId,
    UnknownKind(String),
    UnknownRelation(String),
    UnknownHint(String),
    /// An attribute its hint kind doesn't declare.
    HintAttr {
        hint: String,
        declared: Vec<String>,
    },
    /// A `$` that names no variable.
    UnknownVariable(String),
    /// A template longer than [`template::TEMPLATE_MAX_BYTES`] or naming more than
    /// [`template::TEMPLATE_MAX_VARS`] variables.
    TemplateTooLong,
    /// A `$N` the regex has no group for.
    UnknownGroup {
        group: usize,
        groups: usize,
    },
    /// The regex doesn't compile, and why, in one line.
    Regex(String),
    /// The query doesn't compile against its grammar, and why, in one line.
    Query(String),
    /// The pack holds no `.scm` file for the query.
    QueryMissing,
    /// A `[[languages]]` ID no loaded pack declares.
    UnknownLanguage(String),
    /// A loaded language this build has no grammar for.
    NoGrammar(String),
    /// A confidence outside 0 to 1.
    Confidence,
    /// A prop given by a capture whose schema declares only types no capture fills, such as
    /// `object`: the kind or relation, the prop and its declared types.
    PropType {
        kind: String,
        prop: String,
        types: String,
    },
    /// A prop written as a plain value that breaks its schema: the kind or relation, the prop and
    /// the keyword it breaks (`type`, `enum`, `minimum`, `maximum` or `items`).
    PropValue {
        kind: String,
        prop: String,
        keyword: &'static str,
    },
}

/// A declaration the runner refuses: `pack_invalid` at the value `at` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    /// Where the value sits under its `[[extractors]]` entry, such as `emit[0].scope`.
    pub at: Vec<Step>,
    /// The value itself, as the pack wrote it.
    pub reference: String,
    pub why: Why,
}

impl Invalid {
    /// Always `pack_invalid`: a declaration is pack data.
    pub fn code(&self) -> ErrorCode {
        ErrorCode::PackInvalid
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.reference, self.why)
    }
}

impl std::error::Error for Invalid {}

/// How a declaration matches a file.
enum Matcher {
    Regex(regex::Regex),
    Query {
        language: tree_sitter::Language,
        /// Whether the language is Python, whose docstrings are prose.
        python: bool,
        query: Query,
    },
}

/// What a regex reads about one file extension's language: its `[[languages]]` ID, its comment
/// marks, and its grammar when this build has one.
#[derive(Clone)]
struct FileLanguage {
    id: String,
    marks: Marks,
    grammar: Option<tree_sitter::Language>,
}

/// A prop as a template gives it.
enum PropValue {
    /// A TOML value as written.
    Fixed(Value),
    /// Text with variables, coerced to what the prop schema asks of it.
    Text(Template, Prop),
}

type Props = Vec<(String, PropValue)>;

enum Emit {
    Node {
        kind: Kind,
        level: Level,
        prefix: String,
        key: Template,
        name: Option<Template>,
        parent: Option<RefTemplate>,
        props: Props,
        aliases: Vec<Template>,
    },
    Edge {
        kind: Kind,
        from: RefTemplate,
        to: RefTemplate,
        via: Option<Template>,
        confidence: f32,
        props: Props,
    },
    Hint {
        kind: Kind,
        scope: RefTemplate,
        key: Template,
        value: Option<Template>,
        attrs: Vec<(String, Template)>,
    },
}

/// One compiled declarative extractor.
pub struct Declarative {
    name: String,
    files: Vec<String>,
    tier: Tier,
    matcher: Matcher,
    emits: Vec<Emit>,
    /// Each emit's identity ([`identity`]): the variables whose values fix its fact's ID.
    identities: Vec<Vec<Var>>,
    /// Each file extension's language, for a regex.
    languages: BTreeMap<String, FileLanguage>,
}

impl fmt::Debug for Declarative {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Declarative").field("name", &self.name).field("files", &self.files).finish()
    }
}

fn key(k: &str) -> Step {
    Step::Key(k.to_owned())
}

fn invalid(at: &[Step], reference: &str, why: Why) -> Invalid {
    Invalid { at: at.to_vec(), reference: reference.to_owned(), why }
}

impl Declarative {
    /// Read `spec`'s declaration, or say what in it can't run.
    pub fn compile(spec: &Spec<'_>) -> Result<Declarative, Invalid> {
        let (id, files, emit) = match spec.decl {
            ExtractorDecl::Builtin { id, .. } => {
                return Err(invalid(&[key("builtin")], id, Why::NotDeclarative));
            }
            ExtractorDecl::Regex { id, files, emit, .. } | ExtractorDecl::Query { id, files, emit, .. } => {
                (id, files, emit)
            }
        };
        let (matcher, tier) = match spec.decl {
            ExtractorDecl::Regex { regex, .. } => {
                // The same function the pack loader checks a pattern with, so a pattern that
                // loads runs.
                let compiled =
                    compile_regex(regex).map_err(|why| invalid(&[key("regex")], regex, Why::Regex(why)))?;
                (Matcher::Regex(compiled), Tier::Pattern)
            }
            ExtractorDecl::Query { language, query, .. } => {
                if !spec.languages.iter().any(|l| &l.id == language) {
                    return Err(invalid(
                        &[key("language")],
                        language,
                        Why::UnknownLanguage(language.clone()),
                    ));
                }
                let grammar = grammar::language(language)
                    .ok_or_else(|| invalid(&[key("language")], language, Why::NoGrammar(language.clone())))?;
                let text = spec.query.ok_or_else(|| invalid(&[key("query")], query, Why::QueryMissing))?;
                let compiled = Query::new(&grammar, text).map_err(|e| {
                    let why = e
                        .to_string()
                        .lines()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or_default()
                        .trim()
                        .to_owned();
                    invalid(&[key("query")], query, Why::Query(format!("line {}: {why}", e.row + 1)))
                })?;
                let python = language == "python";
                (Matcher::Query { language: grammar, python, query: compiled }, Tier::Syntactic)
            }
            ExtractorDecl::Builtin { .. } => unreachable!("refused above"),
        };
        let captures: Vec<&str> = match &matcher {
            Matcher::Query { query, .. } => query.capture_names().to_vec(),
            Matcher::Regex(_) => Vec::new(),
        };
        let names = match &matcher {
            Matcher::Regex(re) => Names::Regex { groups: re.captures_len() - 1 },
            Matcher::Query { .. } => Names::Query { captures: &captures },
        };
        let emits = emit
            .iter()
            .enumerate()
            .map(|(k, template)| compile_emit(spec, template, names, &[key("emit"), Step::Index(k)]))
            .collect::<Result<Vec<Emit>, Invalid>>()?;
        let mut languages = BTreeMap::new();
        for l in spec.languages {
            let marks = Marks {
                line: l.line_comment.clone(),
                block: l.block_comment.clone().map(|[open, close]| (open, close)),
            };
            let language = FileLanguage { id: l.id.clone(), marks, grammar: grammar::language(&l.id) };
            for e in &l.extensions {
                // The merged table's first claim on an extension holds, as the loader merges it.
                languages.entry(e.trim_start_matches('.').to_owned()).or_insert_with(|| language.clone());
            }
        }
        Ok(Declarative {
            name: format!("{}/{id}", spec.pack),
            files: files.clone(),
            tier,
            matcher,
            identities: emits.iter().map(identity).collect(),
            emits,
            languages,
        })
    }

    /// Pack-qualified, such as `ruby/ruby-http`: what its facts' provenance names.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The `files` globs it was declared with, for [`crate::select`].
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// Its facts over `files` of `repo`, read in sorted path order.
    ///
    /// Each file's facts, and whether it gives any, depend only on that file, the extractor and
    /// the packs, with one exception: the repo's coverage cut at [`REPO_CAPTURE_MAX`], which falls
    /// at the first file, in sorted path order, whose charge would take the total that the files
    /// before it hold past the bound. "That file" is its text, its path and its repo's name and
    /// commit, which its facts' evidence and IDs copy: the same blob at another path gives other
    /// facts, and charges more at a longer path, so it can be refused there where it was kept at
    /// a shorter one. Each file's charge depends on that file alone, and the run computes every
    /// file's charge, and the cut from them, afresh each time; neither leaves the run. A per-blob
    /// fact cache, when one is built, must store each file's charge beside its facts and apply
    /// the cut again over the stored charges, never store the cut's outcome; this needs the
    /// runner to expose a file's charge then, which it does not yet.
    pub fn run(&self, repo: &RepoView, files: &[RepoPath]) -> FactBatch {
        self.run_within(repo, files, REPO_CAPTURE_MAX)
    }

    /// [`Self::run`] with the repo's bound at `max` in place of [`REPO_CAPTURE_MAX`], so a test can
    /// reach the cut without a repo whose facts hold 256 MiB. Builds without debug assertions
    /// leave it out, so no release can lower the bound.
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn run_with_repo_capture_max(&self, repo: &RepoView, files: &[RepoPath], max: u64) -> FactBatch {
        self.run_within(repo, files, max)
    }

    fn run_within(&self, repo: &RepoView, files: &[RepoPath], max: u64) -> FactBatch {
        let mut files: Vec<&RepoPath> = files.iter().collect();
        files.sort();
        files.dedup();
        let mut held = work::Held::new(max);
        let mut out = Collected::default();
        let mut partial = Vec::new();
        let mut parser = Parser::new();
        if let Matcher::Query { language, .. } = &self.matcher {
            // The grammar loaded into a parser when it compiled the query.
            parser.set_language(language).expect("a grammar this build holds");
        }
        // A regex's parsers, one per language with a grammar, made as its first file comes.
        let mut parsers: BTreeMap<&str, Parser> = BTreeMap::new();
        for file in files {
            let full = format!("{}/{}", repo.name, file.0);
            if held.cut() {
                // Past the repo's cut: not read at all.
                partial.push(PartialFile { path: full, errors: 1, reason: Some(PartialReason::RepoLimit) });
                continue;
            }
            let bytes = repo.read(file);
            let Some(text) = bytes.and_then(|b| std::str::from_utf8(b).ok()) else {
                partial.push(PartialFile { path: full, errors: 1, reason: None });
                continue;
            };
            if self.parses(&file.0) && nesting::deeper_than(text, NESTING_LIMIT) {
                let reason = Some(PartialReason::NestingLimit);
                partial.push(PartialFile { path: full, errors: 1, reason });
                continue;
            }
            let index = lines::Index::new(text);
            let at = At { repo, file: &file.0, full: &full, text, lines: &index };
            let work = work::Work::for_len(text.len());
            let bound = Bound { file: work::capture_budget(text.len()), repo: &mut held };
            let read = match &self.matcher {
                Matcher::Regex(re) => self.regex(re, &mut parsers, &at, &work, bound, &mut out),
                Matcher::Query { .. } => self.query(&mut parser, &at, &work, bound, &mut out),
            };
            match read {
                Ok(0) => {}
                Ok(errors) => partial.push(PartialFile { path: full, errors, reason: None }),
                Err(reason) => partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) }),
            }
        }
        FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: self.name.clone(),
            tier: self.tier,
            nodes: out.nodes.into_values().collect(),
            edges: out.edges.into_values().collect(),
            hints: out.hints.into_values().collect(),
            partial,
        }
    }

    /// The language a regex reads `file` in, by its extension.
    fn language_of(&self, file: &str) -> Option<&FileLanguage> {
        let extension = file.rsplit('/').next().and_then(|n| n.rsplit_once('.')).map(|(_, e)| e);
        extension.and_then(|e| self.languages.get(e))
    }

    /// Whether running over `file` parses it with a grammar: a query always does, a regex when
    /// the file's language has one.
    fn parses(&self, file: &str) -> bool {
        match &self.matcher {
            Matcher::Query { .. } => true,
            Matcher::Regex(_) => self.language_of(file).is_some_and(|l| l.grammar.is_some()),
        }
    }

    /// Every match of `re` in the file with its prose blanked and outside its parse's error
    /// regions; the errors it counted: the parse's error nodes, and the props it couldn't coerce.
    /// A parse past its `work` budget, or matches that would charge more than the file's budget or
    /// take the repo past its `bound`, emit nothing.
    fn regex<'a>(
        &'a self,
        re: &regex::Regex,
        parsers: &mut BTreeMap<&'a str, Parser>,
        at: &At<'_>,
        work: &work::Work,
        bound: Bound<'_>,
        out: &mut Collected,
    ) -> Result<u32, PartialReason> {
        let read = match self.language_of(at.file) {
            Some(language) => read(language, parsers, at.text, work)?,
            None => Read::default(),
        };
        let blanked = comments::blank(at.text, &read.prose);
        let mut matches = Matches::default();
        for caps in re.captures_iter(&blanked) {
            let whole = caps.get(0).map_or(0..0, |m| m.range());
            if read.errors.overlap(&whole) {
                continue;
            }
            let groups: Vec<Option<&str>> = caps.iter().map(|m| m.map(|m| m.as_str())).collect();
            // Blanking keeps every byte offset, so the text's line index places the match.
            let line = at.lines.line_of(whole.start);
            if !self.keep(at, &mut matches, line, groups, Vars::Groups, bound.file) {
                return Err(PartialReason::WorkLimit);
            }
        }
        if !bound.repo.add(matches.charged) {
            return Err(PartialReason::RepoLimit);
        }
        Ok(read.errors.count + self.emit_all(&matches, Vars::Groups, at, out))
    }

    /// Keep one match of a file, `line` and its groups' or captures' `values`, unless the file's
    /// matches would then charge more than its `budget`, when it returns false. A match whose values
    /// and line an earlier one had makes the same facts, so it is only counted. Any other is
    /// charged what its emits copy ([`Self::copies`]), and each fact it makes that no earlier
    /// match of the file made, by the values its ID takes, its structure ([`HINT_COST`],
    /// [`NODE_COST`]), its ID's further copies, its kind, its extractor's name and its evidence,
    /// or, for a node or an edge it cites on a further line, that line ([`EVIDENCE_COST`]). Values
    /// longer than [`SAME_MAX`] are never compared: such a match, or such a fact, counts as new.
    /// What the run keeps of a distinct match until it emits, its line, its values and their
    /// index entry ([`MATCH_COST`], [`MATCH_VALUE_COST`]), and of each new fact's ID and each
    /// cited line to find their repeats ([`FACT_COST`], [`FACT_VALUE_COST`], [`CITE_COST`]), is
    /// charged to the file's budget too, as it is kept, but not to the repo's total, since the
    /// file's matches are dropped once it is read. Each variable's value counts once in an ID,
    /// however many times the templates name it.
    fn keep<'t>(
        &self,
        at: &At<'_>,
        matches: &mut Matches<'t>,
        line: u32,
        values: Vec<Option<&'t str>>,
        vars: Vars,
        budget: u64,
    ) -> bool {
        use std::collections::hash_map::Entry;
        let compared = values.iter().flatten().map(|v| v.len()).sum::<usize>() <= SAME_MAX;
        let key = (line, values);
        if compared && let Some(&i) = matches.index.get(&key) {
            matches.kept[i].2 = matches.kept[i].2.saturating_add(1);
            return true;
        }
        let (line, values) = key;
        let (groups, captures): (&[Option<&'t str>], &[Option<&'t str>]) = match vars {
            Vars::Groups => (&values, &[]),
            Vars::Captures => (&[], &values),
        };
        let env = Env { repo: &at.repo.name, file: at.file, line, groups, captures };
        let size = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
        // Each emit's ID values, or `None` for an emit this match doesn't fire: one whose key,
        // scope, ends or via name a group or a capture the match leaves unset, as a query's
        // pattern leaves every capture of the others' unset. Such an emit makes no fact and
        // copies nothing, so it is charged nothing.
        let ids: Vec<Option<Vec<Val<'t>>>> = self
            .identities
            .iter()
            .map(|identity| {
                let id: Vec<Val<'t>> = identity.iter().map(|v| v.value(line, groups, captures)).collect();
                (!id.iter().any(|v| matches!(v, Val::Text(None)))).then_some(id)
            })
            .collect();
        let mut charge = self.copies(&env, &ids);
        let text = at.lines.line(at.text, line).unwrap_or_default();
        let cited =
            text.char_indices().nth(planroom_contract::CONTAINS_MAX_CHARS).map_or(text.len(), |(i, _)| i);
        let evidence = size(at.full.len() + cited + at.repo.commit.len());
        let mut kept = MATCH_COST.saturating_add(MATCH_VALUE_COST.saturating_mul(size(values.len())));
        if matches.hint_line != Some(line) {
            // A hint's ID holds its line, so a hint repeats only one made on its own line: the set
            // that finds them starts afresh with each line. Matches come in text order, so a line
            // seldom comes back; when one does, its hints count as new again, which charges more,
            // never less.
            matches.line_hints = HashSet::new();
            matches.line_charge = 0;
            matches.hint_line = Some(line);
        }
        for (i, (emit, id)) in self.emits.iter().zip(ids).enumerate() {
            // Each variable's value once, however many times the templates name it.
            let Some(id) = id else { continue };
            // What a new fact holds beside its templates' text: its structure, its ID's further
            // copies, its kind, its provenance and its evidence, which a node's or an edge's set of
            // cited lines holds again.
            let new = size(self.name.len())
                + match emit {
                    Emit::Node { kind, prefix, key, props, aliases, .. } => {
                        let ids = at.repo.name.len() + prefix.len() + 2 + key.size(&env);
                        let list = if aliases.is_empty() { 0 } else { LIST_COST };
                        NODE_COST + size(kind.0.len() + 2 * ids) + 2 * evidence + map_heap(props.len()) + list
                    }
                    Emit::Edge { kind, props, .. } => {
                        NODE_COST
                            + size(kind.0.len() + 3 * EDGE_ID_LEN)
                            + 2 * evidence
                            + map_heap(props.len())
                    }
                    Emit::Hint { kind, attrs, .. } => {
                        HINT_COST + size(kind.0.len() + 2 * HINT_ID_LEN) + evidence + map_heap(attrs.len())
                    }
                };
            let further = EVIDENCE_COST + 2 * evidence;
            let compared = id.iter().map(Val::len).sum::<usize>() <= SAME_MAX;
            // What the run keeps of a new fact's ID to find its repeats, charged to the file alone.
            let known = FACT_COST.saturating_add(FACT_VALUE_COST.saturating_mul(size(id.len())));
            let number = matches.facts.len();
            let (fact, cites) = match emit {
                _ if !compared => (new, 0),
                Emit::Hint { .. } => {
                    if matches.line_hints.insert((i, id)) {
                        matches.line_charge = matches.line_charge.saturating_add(known);
                        (new, 0)
                    } else {
                        (0, 0)
                    }
                }
                Emit::Node { .. } | Emit::Edge { .. } => match matches.facts.entry((i, id)) {
                    Entry::Vacant(v) => {
                        v.insert(number);
                        matches.lines.insert((number, line));
                        (new, known.saturating_add(CITE_COST))
                    }
                    Entry::Occupied(o) => {
                        if matches.lines.insert((*o.get(), line)) {
                            (further, CITE_COST)
                        } else {
                            (0, 0)
                        }
                    }
                },
            };
            charge = charge.saturating_add(fact);
            kept = kept.saturating_add(cites);
        }
        matches.charged = matches.charged.saturating_add(charge);
        matches.kept_charge = matches.kept_charge.saturating_add(kept);
        if matches.charged.saturating_add(matches.kept_charge).saturating_add(matches.line_charge) > budget {
            return false;
        }
        if compared {
            matches.index.insert((line, values.clone()), matches.kept.len());
        }
        matches.kept.push((line, values, 1));
        true
    }

    /// Emit every match `matches` kept, in text order, into `out`; the props they couldn't
    /// coerce, a repeated match's counted each time it came.
    fn emit_all(&self, matches: &Matches<'_>, vars: Vars, at: &At<'_>, out: &mut Collected) -> u32 {
        let mut errors = 0u32;
        for (line, values, times) in &matches.kept {
            let (groups, captures): (&[Option<&str>], &[Option<&str>]) = match vars {
                Vars::Groups => (values, &[]),
                Vars::Captures => (&[], values),
            };
            let env = Env { repo: &at.repo.name, file: at.file, line: *line, groups, captures };
            errors = errors.saturating_add(self.emit(&env, at, out).saturating_mul(*times));
        }
        // Every line of evidence names its file, so no later file's can repeat one of these.
        // Dropped, not cleared: a cleared set keeps its room, which every later file would sweep.
        out.node_cites = HashMap::new();
        out.edge_cites = HashMap::new();
        errors
    }

    /// At least the bytes one match's emits copy, whether or not the facts they make are new:
    /// every template's text and each prop, attribute or alias entry it fills ([`ENTRY_COST`]),
    /// of each emit the match fires, which `fired` gives in emit order.
    fn copies<T>(&self, env: &Env<'_>, fired: &[Option<T>]) -> u64 {
        let texts = |props: &Props| -> usize {
            props
                .iter()
                .map(|(name, v)| match v {
                    PropValue::Text(t, _) => ENTRY_COST + name.len() + t.size(env),
                    PropValue::Fixed(value) => ENTRY_COST + name.len() + fixed_size(value),
                })
                .sum()
        };
        let copied: usize = self
            .emits
            .iter()
            .zip(fired)
            .filter(|(_, fired)| fired.is_some())
            .map(|(emit, _)| match emit {
                Emit::Node { key, name, parent, props, aliases, .. } => {
                    let key = key.size(env);
                    // The ID holds the key, and the name is the key when no template gives it.
                    key + name.as_ref().map_or(key, |n| ENTRY_COST + n.size(env))
                        + parent.as_ref().map_or(0, |p| ENTRY_COST + p.size(env))
                        + texts(props)
                        + aliases.iter().map(|a| ENTRY_COST + a.size(env)).sum::<usize>()
                }
                Emit::Edge { from, to, via, props, .. } => {
                    from.size(env)
                        + to.size(env)
                        + via.as_ref().map_or(0, |v| ENTRY_COST + v.size(env))
                        + texts(props)
                }
                Emit::Hint { scope, key, value, attrs, .. } => {
                    scope.size(env)
                        + key.size(env)
                        + value.as_ref().map_or(0, |v| ENTRY_COST + v.size(env))
                        + attrs.iter().map(|(name, t)| ENTRY_COST + name.len() + t.size(env)).sum::<usize>()
                }
            })
            .sum();
        u64::try_from(copied).unwrap_or(u64::MAX)
    }

    /// Every match of `query` in the file's parse that captures no comment or docstring and
    /// nothing in an error region; the errors it counted: the parse's error nodes, one for a match
    /// limit reached, and the props it couldn't coerce. A parse or a query past its `work` budget,
    /// its text predicates' reads included, or matches that would charge more than the file's
    /// budget or take the repo past its `bound`, emit nothing.
    fn query(
        &self,
        parser: &mut Parser,
        at: &At<'_>,
        work: &work::Work,
        bound: Bound<'_>,
        out: &mut Collected,
    ) -> Result<u32, PartialReason> {
        let Matcher::Query { query, python, .. } = &self.matcher else { return Ok(0) };
        let Some(tree) = work::parse(parser, at.text, work) else {
            return if work.over() { Err(PartialReason::WorkLimit) } else { Ok(1) };
        };
        let regions = Errors::of(tree.root_node());
        // Read once per file, so each capture finds its span by a binary search.
        let prose = prose::spans(tree.root_node(), *python);
        // What a capture's text and a text predicate read: the file with its prose blanked, at
        // the same offsets, so a capture that holds a comment, such as a call's arguments with
        // one among them, never carries the comment's text into a fact.
        let source: std::borrow::Cow<'_, str> = if prose.is_empty() {
            std::borrow::Cow::Borrowed(at.text)
        } else {
            std::borrow::Cow::Owned(comments::blank(at.text, prose.ranges()))
        };
        let mut errors = regions.count;
        if work.depths(&tree) {
            return Err(PartialReason::WorkLimit);
        }
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(MATCH_LIMIT);
        let mut progress = work.query_progress();
        let options = QueryCursorOptions::new().progress_callback(&mut progress);
        // Text predicates read the file only through what counts their reads as work.
        let predicates = work.predicates(source.as_bytes());
        let mut found = cursor.matches_with_options(query, tree.root_node(), predicates, options);
        let count = query.capture_names().len();
        // Each kept match's line and captures, emitted once the query has run within its budget.
        let mut matches = Matches::default();
        while let Some(m) = found.next() {
            if work.over() {
                break;
            }
            let dropped = |c: &tree_sitter::QueryCapture<'_>| {
                let bytes = c.node.byte_range();
                prose.hold(&bytes) || regions.overlap(&bytes)
            };
            if m.captures().is_empty() || m.captures().iter().any(dropped) {
                continue;
            }
            let mut captures: Vec<Option<&str>> = vec![None; count];
            for c in m.captures() {
                let slot = &mut captures[c.index as usize];
                if slot.is_none() {
                    *slot = c.node.utf8_text(source.as_bytes()).ok();
                }
            }
            let row = m.captures().iter().map(|c| c.node.start_position().row).min().unwrap_or(0);
            let line = u32::try_from(row + 1).unwrap_or(u32::MAX);
            // What the emits copy: overlapping captures could make it the file's square.
            if !self.keep(at, &mut matches, line, captures, Vars::Captures, bound.file) {
                return Err(PartialReason::WorkLimit);
            }
        }
        drop(found);
        if work.over() {
            return Err(PartialReason::WorkLimit);
        }
        if !bound.repo.add(matches.charged) {
            return Err(PartialReason::RepoLimit);
        }
        errors += self.emit_all(&matches, Vars::Captures, at, out);
        if cursor.did_exceed_match_limit() {
            errors += 1;
        }
        Ok(errors)
    }

    /// Every emit for one match; the props it couldn't coerce.
    fn emit(&self, env: &Env<'_>, at: &At<'_>, out: &mut Collected) -> u32 {
        let evidence = Evidence::Line {
            path: at.full.to_owned(),
            line: env.line,
            contains: at.lines.line(at.text, env.line).map(lines::cut),
            commit: Some(at.repo.commit.clone()),
            // A snapshot is keyed by its commit, never a branch (DR-85).
            branch: None,
        };
        let provenance = Provenance::Extracted { extractor: self.name.clone(), tier: self.tier };
        let mut errors = 0;
        for emit in &self.emits {
            match emit {
                Emit::Node { kind, level, prefix, key, name, parent, props, aliases } => {
                    let Some(key) = key.expand(env).filter(|k| !k.is_empty()) else { continue };
                    let parent = match parent {
                        Some(p) => match p.expand_id(env) {
                            Some(id) => Some(id),
                            None => continue,
                        },
                        None => None,
                    };
                    let id = node_id(&at.repo.name, prefix, &key);
                    let name = name.as_ref().and_then(|n| n.expand(env)).unwrap_or(key);
                    let (props, bad) = fill(props, env);
                    errors += bad;
                    let node = Node {
                        id: id.clone(),
                        kind: kind.clone(),
                        level: *level,
                        parent,
                        name,
                        aliases: aliases
                            .iter()
                            .filter_map(|a| a.expand(env))
                            .filter(|a| !a.is_empty())
                            .collect(),
                        props,
                        layer: Layer::Actual,
                        provenance: provenance.clone(),
                        evidence: vec![evidence.clone()],
                    };
                    let cited = out.node_cites.entry(id.clone()).or_default();
                    merge(out.nodes.entry(id), node, |n| &mut n.evidence, cited);
                }
                Emit::Edge { kind, from, to, via, confidence, props } => {
                    let (Some(from), Some(to)) = (from.expand(env), to.expand(env)) else { continue };
                    let via = match via {
                        Some(v) => match v.expand(env) {
                            Some(v) => Some(v),
                            None => continue,
                        },
                        None => None,
                    };
                    let (props, bad) = fill(props, env);
                    errors += bad;
                    let id = edge_id(kind, &from, &to, via.as_deref());
                    let edge = planroom_contract::Edge {
                        id: id.clone(),
                        kind: kind.clone(),
                        from,
                        to,
                        via,
                        via_ref: None,
                        props,
                        evidence: vec![evidence.clone()],
                        confidence: *confidence,
                        layer: Layer::Actual,
                        provenance: provenance.clone(),
                    };
                    let cited = out.edge_cites.entry(id.0.clone()).or_default();
                    merge(out.edges.entry(id.0), edge, |e| &mut e.evidence, cited);
                }
                Emit::Hint { kind, scope, key, value, attrs } => {
                    let Some(scope) = scope.expand_id(env) else { continue };
                    let Some(key) = key.expand(env).filter(|k| !k.is_empty()) else { continue };
                    let value = value.as_ref().and_then(|v| v.expand(env));
                    let attrs: BTreeMap<String, Value> = attrs
                        .iter()
                        .filter_map(|(name, t)| Some((name.clone(), Value::String(t.expand(env)?))))
                        .collect();
                    let evidence = vec![evidence.clone()];
                    let id = hint_id(kind, &scope, &key, &evidence);
                    let hint = Hint {
                        id: id.clone(),
                        kind: kind.clone(),
                        scope,
                        key,
                        value,
                        attrs,
                        evidence,
                        provenance: provenance.clone(),
                    };
                    // One hint, its kind, scope, key and line, that two matches give, each with
                    // the attributes its variables set, as `process.env.X` and `process.env.X ??
                    // 'd'` both read X: the first gives each attribute and the value, and a later
                    // one fills in what the first left out. A match is charged what its emits
                    // copy whether or not its facts are new ([`Declarative::copies`]), so a fill
                    // holds no more than its match was charged.
                    match out.hints.entry(id) {
                        std::collections::btree_map::Entry::Vacant(v) => {
                            v.insert(hint);
                        }
                        std::collections::btree_map::Entry::Occupied(o) => {
                            let kept = o.into_mut();
                            for (name, value) in hint.attrs {
                                kept.attrs.entry(name).or_insert(value);
                            }
                            if kept.value.is_none() {
                                kept.value = hint.value;
                            }
                        }
                    }
                }
            }
        }
        errors
    }
}

/// What one file's facts may hold: the file's own budget, and what the run holds for the repo.
struct Bound<'a> {
    file: u64,
    repo: &'a mut work::Held,
}

/// One file as the runner reads it.
struct At<'a> {
    repo: &'a RepoView,
    /// Repo-relative.
    file: &'a str,
    /// `<repo>/<path>`, as evidence names it.
    full: &'a str,
    text: &'a str,
    /// The text's line starts, read once, so each match finds its line by a binary search.
    lines: &'a lines::Index,
}

/// Facts by ID, so they come out sorted and a fact two matches emit is one fact.
#[derive(Default)]
struct Collected {
    nodes: BTreeMap<NodeId, Node>,
    edges: BTreeMap<String, planroom_contract::Edge>,
    hints: BTreeMap<String, Hint>,
    /// Each node's and each edge's evidence so far in the file being read, so a repeated line is
    /// found without a pass over what the fact already cites.
    node_cites: HashMap<NodeId, HashSet<Evidence>>,
    edge_cites: HashMap<String, HashSet<Evidence>>,
}

/// Keep the first emission of a fact, and add each later one's evidence to it once; `cited` is
/// the evidence the fact holds so far.
fn merge<K: Ord, T>(
    entry: std::collections::btree_map::Entry<'_, K, T>,
    fact: T,
    evidence: impl Fn(&mut T) -> &mut Vec<Evidence>,
    cited: &mut HashSet<Evidence>,
) {
    use std::collections::btree_map::Entry;
    let mut fact = fact;
    let items = std::mem::take(evidence(&mut fact));
    let kept = match entry {
        Entry::Vacant(v) => evidence(v.insert(fact)),
        Entry::Occupied(o) => evidence(o.into_mut()),
    };
    for item in items {
        if cited.insert(item.clone()) {
            kept.push(item);
        }
    }
}

/// Whether a match's values are a regex's groups or a query's captures.
#[derive(Clone, Copy)]
enum Vars {
    Groups,
    Captures,
}

/// One file's matches as the runner keeps them before it emits them, and what they charge its
/// capture budget ([`Declarative::keep`]).
#[derive(Default)]
struct Matches<'t> {
    /// Each distinct match in text order: its line, its values and how many times it came.
    kept: Vec<(u32, Vec<Option<&'t str>>, u32)>,
    /// Where each kept match whose values are short enough to compare sits in `kept`.
    index: HashMap<(u32, Vec<Option<&'t str>>), usize>,
    /// Each node and edge emit's facts so far in the file, by the values their IDs take, each
    /// numbered in the order it came; and each line a node or an edge cites, by its number.
    facts: HashMap<(usize, Vec<Val<'t>>), usize>,
    lines: HashSet<(usize, u32)>,
    /// The line of the last match kept, and each hint emit's hints on it, by the values their IDs
    /// take beside the line.
    hint_line: Option<u32>,
    line_hints: HashSet<(usize, Vec<Val<'t>>)>,
    /// What the kept matches' facts charge: the file's charge to the repo's total.
    charged: u64,
    /// What the run keeps of the distinct matches and of the facts' IDs until it emits, charged
    /// to the file's budget alone.
    kept_charge: u64,
    /// What `line_hints` holds, charged to the file's budget alone while it holds it.
    line_charge: u64,
}

/// At least the heap a clone of a prop written as a plain value holds: a string's text, and an
/// array's or an object's room for its items and theirs, each with its allocation.
fn fixed_size(value: &Value) -> usize {
    let alloc = ENTRY_COST / 2;
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
        Value::String(s) => alloc + s.len(),
        Value::Array(items) => alloc + items.iter().map(|v| 2 * VALUE_SIZE + fixed_size(v)).sum::<usize>(),
        Value::Object(map) => {
            // An object's entries, each a key of 24 bytes, a value and a hash, and its index of
            // them, 8 bytes an entry, each with room to grow.
            let entry = VALUE_SIZE + 40;
            2 * alloc + map.iter().map(|(k, v)| 2 * entry + alloc + k.len() + fixed_size(v)).sum::<usize>()
        }
    }
}

/// The props for one match, and how many captured ones didn't fit their schema and were left out.
fn fill(props: &Props, env: &Env<'_>) -> (BTreeMap<String, Value>, u32) {
    let mut out = BTreeMap::new();
    let mut bad = 0;
    for (name, value) in props {
        match value {
            PropValue::Fixed(v) => {
                out.insert(name.clone(), v.clone());
            }
            PropValue::Text(t, prop) => {
                let Some(text) = t.expand(env) else { continue };
                match coerce::coerce(&text, prop) {
                    Some(v) => {
                        out.insert(name.clone(), v);
                    }
                    None => bad += 1,
                }
            }
        }
    }
    (out, bad)
}

/// One emit template, checked against the vocabulary and the hint kinds.
fn compile_emit(
    spec: &Spec<'_>,
    template: &EmitTemplate,
    names: Names<'_>,
    at: &[Step],
) -> Result<Emit, Invalid> {
    let under = |k: &str| [at, &[key(k)]].concat();
    let prefix =
        |kind: &str| spec.vocabulary.kinds.iter().find(|k| k.kind.0 == kind).map(|k| k.id_prefix.clone());
    let text =
        |value: &str, k: &str| Template::parse(value, names).map_err(|why| invalid(&under(k), value, why));
    let reference = |value: &str, k: &str| {
        RefTemplate::parse(value, names, prefix).map_err(|why| invalid(&under(k), value, why))
    };
    let props = |props: &BTreeMap<String, Value>, kind: &Kind, schema: &Value| -> Result<Props, Invalid> {
        props
            .iter()
            .map(|(name, value)| {
                let here = [at, &[key("props"), key(name)]].concat();
                // A plain value is emitted as written, so it must meet its schema at load.
                let fixed = |value: &Value| match coerce::breaks(schema, name, value) {
                    None => Ok(PropValue::Fixed(value.clone())),
                    Some(keyword) => {
                        let written = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
                        let why = Why::PropValue { kind: kind.0.clone(), prop: name.clone(), keyword };
                        Err(invalid(&here, &written, why))
                    }
                };
                let compiled = match value {
                    Value::String(s) => {
                        let t = Template::parse(s, names).map_err(|why| invalid(&here, s, why))?;
                        if t.is_text() {
                            fixed(value)?
                        } else {
                            let prop = coerce::prop(schema, name).map_err(|types| {
                                let why = Why::PropType { kind: kind.0.clone(), prop: name.clone(), types };
                                invalid(&here, s, why)
                            })?;
                            PropValue::Text(t, prop)
                        }
                    }
                    other => fixed(other)?,
                };
                Ok((name.clone(), compiled))
            })
            .collect()
    };
    match template {
        EmitTemplate::Node { node, key: k, name, parent, props: p, aliases } => {
            let vocab = spec
                .vocabulary
                .kinds
                .iter()
                .find(|v| &v.kind.0 == node)
                .ok_or_else(|| invalid(&under("node"), node, Why::UnknownKind(node.clone())))?;
            let parent = parent.as_deref().map(|p| reference(p, "parent")).transpose()?;
            if let Some(p) = &parent
                && !p.is_id()
            {
                return Err(invalid(&under("parent"), template_parent(template), Why::NotAnId));
            }
            Ok(Emit::Node {
                kind: vocab.kind.clone(),
                level: vocab.level,
                prefix: vocab.id_prefix.clone(),
                key: text(k, "key")?,
                name: name.as_deref().map(|n| text(n, "name")).transpose()?,
                parent,
                props: props(p, &vocab.kind, &vocab.props)?,
                aliases: aliases
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        Template::parse(a, names)
                            .map_err(|why| invalid(&[at, &[key("aliases"), Step::Index(i)]].concat(), a, why))
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
        EmitTemplate::Edge { edge, from, to, via, confidence, props: p } => {
            let vocab = spec
                .vocabulary
                .relations
                .iter()
                .find(|r| &r.kind.0 == edge)
                .ok_or_else(|| invalid(&under("edge"), edge, Why::UnknownRelation(edge.clone())))?;
            let confidence = confidence.unwrap_or(1.0);
            if !(0.0..=1.0).contains(&confidence) {
                return Err(invalid(&under("confidence"), &confidence.to_string(), Why::Confidence));
            }
            Ok(Emit::Edge {
                kind: vocab.kind.clone(),
                from: reference(from, "from")?,
                to: reference(to, "to")?,
                via: via.as_deref().map(|v| text(v, "via")).transpose()?,
                confidence,
                props: props(p, &vocab.kind, &vocab.props)?,
            })
        }
        EmitTemplate::Hint { hint, scope, key: k, value, attrs } => {
            let declared = spec
                .hints
                .get(hint)
                .ok_or_else(|| invalid(&under("hint"), hint, Why::UnknownHint(hint.clone())))?;
            let scope_ref = reference(scope, "scope")?;
            if !scope_ref.is_id() {
                return Err(invalid(&under("scope"), scope, Why::NotAnId));
            }
            let attrs = attrs
                .iter()
                .map(|(name, value)| {
                    let at = [at, &[key("attrs"), key(name)]].concat();
                    if !declared.contains(name) {
                        let why = Why::HintAttr { hint: hint.clone(), declared: declared.clone() };
                        return Err(invalid(&at, name, why));
                    }
                    Template::parse(value, names)
                        .map(|t| (name.clone(), t))
                        .map_err(|why| invalid(&at, value, why))
                })
                .collect::<Result<_, _>>()?;
            Ok(Emit::Hint {
                kind: Kind(hint.clone()),
                scope: scope_ref,
                key: text(k, "key")?,
                value: value.as_deref().map(|v| text(v, "value")).transpose()?,
                attrs,
            })
        }
    }
}

/// The variables whose values, with the file, fix the ID of the fact `emit` makes, each once and
/// in order: a node's key's, an edge's ends' and via's, and a hint's scope's and key's, which
/// with the line fix a hint's, since a hint's ID holds its evidence. Two matches that give them
/// the same values, and for a hint on the same line, make the same fact, however many times a
/// template names a variable.
fn identity(emit: &Emit) -> Vec<Var> {
    let mut vars = Vec::new();
    match emit {
        Emit::Node { key, .. } => key.vars(&mut vars),
        Emit::Edge { from, to, via, .. } => {
            from.vars(&mut vars);
            to.vars(&mut vars);
            if let Some(via) = via {
                via.vars(&mut vars);
            }
        }
        Emit::Hint { scope, key, .. } => {
            scope.vars(&mut vars);
            key.vars(&mut vars);
        }
    }
    vars.sort_unstable();
    vars.dedup();
    vars
}

/// A node template's `parent`, as written.
fn template_parent(template: &EmitTemplate) -> &str {
    match template {
        EmitTemplate::Node { parent: Some(p), .. } => p,
        _ => "",
    }
}

/// A parse's errors: how many error and missing nodes it holds, which make a file partial, and
/// the error regions no fact comes from. A region is an error node's bytes, or, for a missing
/// node, the bytes of the smallest statement-level node around it, since the parser made up what
/// it recovered there.
#[derive(Default)]
struct Errors {
    count: u32,
    /// Sorted and disjoint, so a match finds the one region it could overlap by a binary search.
    /// An empty region counts as the byte it stands at.
    regions: Vec<std::ops::Range<usize>>,
}

/// An empty range as the byte it stands at, so a zero-width match or region still overlaps.
fn wide(r: &std::ops::Range<usize>) -> std::ops::Range<usize> {
    r.start..r.end.max(r.start + 1)
}

/// `regions` widened, sorted, and merged where they overlap or touch: the same bytes, as disjoint
/// ranges in order.
fn disjoint(regions: &[std::ops::Range<usize>]) -> Vec<std::ops::Range<usize>> {
    let mut sorted: Vec<_> = regions.iter().map(wide).collect();
    sorted.sort_by_key(|r| r.start);
    let mut out: Vec<std::ops::Range<usize>> = Vec::with_capacity(sorted.len());
    for r in sorted {
        match out.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

impl Errors {
    fn of(root: tree_sitter::Node<'_>) -> Errors {
        let mut errors = Errors::default();
        if !root.has_error() {
            return errors;
        }
        // Each node, with its depth and the bytes of its smallest statement-level ancestor, carried
        // down the walk, so a missing node never walks up its ancestors.
        let mut todo = vec![(root, 0usize, root.byte_range())];
        while let Some((node, depth, around)) = todo.pop() {
            if node.is_error() {
                errors.count += 1;
                errors.regions.push(node.byte_range());
            } else if node.is_missing() {
                errors.count += 1;
                errors.regions.push(around.clone());
            }
            if node.has_error() {
                let around = if statement_level(node, depth) { node.byte_range() } else { around };
                let mut cursor = node.walk();
                todo.extend(node.children(&mut cursor).map(|c| (c, depth + 1, around.clone())));
            }
        }
        errors.regions = disjoint(&errors.regions);
        errors
    }

    /// The whole of a text whose parse failed outright.
    fn whole(text: &str) -> Errors {
        Errors { count: 1, regions: vec![wide(&(0..text.len()))] }
    }

    /// Whether `bytes` overlaps an error region. An empty range counts as the byte it stands at,
    /// so a zero-width match inside a region is dropped too.
    fn overlap(&self, bytes: &std::ops::Range<usize>) -> bool {
        let bytes = wide(bytes);
        // The first region that ends after the match starts is the only one it can overlap.
        let first = self.regions.partition_point(|r| r.end <= bytes.start);
        self.regions.get(first).is_some_and(|r| r.start < bytes.end)
    }
}

/// Whether `node`, `depth` levels below the root, is statement-level: a statement, a
/// declaration, a definition or a Rust item, as every built-in grammar names them, or the file's
/// root or a child of it.
fn statement_level(node: tree_sitter::Node<'_>, depth: usize) -> bool {
    let kind = node.kind();
    ["statement", "declaration", "definition", "_item"].iter().any(|s| kind.ends_with(s)) || depth <= 1
}

/// What a regex may read of one file: the spans it never reads, and the parse's errors.
#[derive(Default)]
struct Read {
    /// Comments and docstrings, blanked before the regex runs.
    prose: Vec<std::ops::Range<usize>>,
    errors: Errors,
}

/// What a regex may read of `text`, in a file of `language`. A language with a grammar takes its
/// prose from its parse, comments and Python's docstrings, and its error regions, which no match
/// may overlap; a text it can't parse at all is one error region, and one whose parse takes more
/// than its `work` budget gives the regex nothing. A language with no grammar takes its prose
/// from its comment marks.
fn read<'a>(
    language: &'a FileLanguage,
    parsers: &mut BTreeMap<&'a str, Parser>,
    text: &str,
    work: &work::Work,
) -> Result<Read, PartialReason> {
    let Some(grammar) = &language.grammar else {
        return Ok(Read { prose: comments::spans(text, &language.marks), errors: Errors::default() });
    };
    let parser = parsers.entry(language.id.as_str()).or_insert_with(|| {
        let mut parser = Parser::new();
        parser.set_language(grammar).expect("a grammar this build holds");
        parser
    });
    let Some(tree) = work::parse(parser, text, work) else {
        if work.over() {
            return Err(PartialReason::WorkLimit);
        }
        return Ok(Read { prose: Vec::new(), errors: Errors::whole(text) });
    };
    let root = tree.root_node();
    Ok(Read { prose: prose::spans(root, language.id == "python").into_ranges(), errors: Errors::of(root) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fact_index_charges_at_least_what_its_entries_hold() {
        let size = |n: usize| u64::try_from(n).unwrap();
        assert!(size(std::mem::size_of::<Val<'_>>()) <= FACT_VALUE_COST);
        // An entry, its control byte, room for the table to reach its load and its old table
        // while it doubles, and the allocator's share of its values.
        let entry = size(std::mem::size_of::<((usize, Vec<Val<'_>>), usize)>() + 1);
        assert!(entry * 7 / 2 + 16 <= FACT_COST, "{entry}");
        let cite = size(std::mem::size_of::<(usize, u32)>() + 1);
        assert!(cite * 7 / 2 <= CITE_COST, "{cite}");
    }

    #[test]
    fn lines_count_from_one_at_each_line_start() {
        let index = lines::Index::new("ab\ncd\n\ne");
        assert_eq!([0, 1, 2, 3, 6, 7].map(|i| index.line_of(i)), [1, 1, 1, 2, 3, 4]);
    }

    #[test]
    fn a_match_overlaps_a_region_as_a_scan_of_every_region_finds() {
        let regions = [3..5, 4..9, 12..12, 20..30, 25..26, 40..41];
        let settled = Errors { count: 1, regions: disjoint(&regions) };
        for start in 0..45 {
            for end in start..45 {
                let m = start..end;
                let w = wide(&m);
                let scan = regions.iter().map(wide).any(|r| r.start < w.end && w.start < r.end);
                assert_eq!(settled.overlap(&m), scan, "{m:?}");
            }
        }
    }
}
