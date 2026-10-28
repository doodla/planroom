//! The tree-sitter grammars this build holds, by the `[[languages]]` ID a query extractor names:
//! TypeScript, TSX, JavaScript, Python, Go and Rust, one runtime for all six [DR-83].

use tree_sitter::Language;
use tree_sitter_language::LanguageFn;

/// Every built-in grammar, by the language ID the code pack declares for it.
const GRAMMARS: &[(&str, LanguageFn)] = &[
    ("typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
    ("tsx", tree_sitter_typescript::LANGUAGE_TSX),
    ("javascript", tree_sitter_javascript::LANGUAGE),
    ("python", tree_sitter_python::LANGUAGE),
    ("go", tree_sitter_go::LANGUAGE),
    ("rust", tree_sitter_rust::LANGUAGE),
];

/// The language IDs with a built-in grammar, in a fixed order.
pub fn ids() -> impl Iterator<Item = &'static str> {
    GRAMMARS.iter().map(|(id, _)| *id)
}

/// The grammar of the language `id`, or `None` when this build has none for it.
pub fn language(id: &str) -> Option<Language> {
    GRAMMARS.iter().find(|(g, _)| *g == id).map(|(_, f)| Language::new(*f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grammar_loads_into_a_parser() {
        for id in ids() {
            let language = language(id).unwrap();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&language).unwrap_or_else(|e| panic!("{id}: {e}"));
            let tree = parser.parse("x", None).unwrap();
            assert!(tree.root_node().child_count() > 0 || tree.root_node().is_named(), "{id}");
        }
        assert!(language("yaml").is_none());
    }
}
