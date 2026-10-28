; Rust: imports, definitions and calls. Each pattern sets its own captures, and each emit in
; languages/rust.toml names a capture only its patterns set. A capture's text is the source with
; its comments, doc comments included, blanked, and a match that captures a comment is dropped. rust_runtime.scm reads the
; hints a deployment joins.

; Imports: use paths, extern crates, and modules declared in other files. Each path a use names
; is its own import, never the braced group: `use a::b;`, `use a::b as c;` and `use a::*;` import
; `a::b`, `a::b` and `a`; `use a::{b, c::d as e, self};` imports `a::b`, `a::c::d` and `a`; and a
; group inside a group joins both prefixes, `use a::{b::{c, self}};` importing `a::b::c` and
; `a::b`. A group nested deeper names no import.
(use_declaration argument: [(identifier) (scoped_identifier)] @import)
(use_declaration argument: (use_as_clause path: (_) @import))
(use_declaration argument: (use_wildcard (_) @import))
(use_declaration argument: (use_list [
  ((identifier) @import)
  ((scoped_identifier) @import)
  (use_as_clause path: (_) @import)
  (use_wildcard (_) @import)
]))
(use_declaration argument: (scoped_use_list path: (_) @import list: (use_list (self))))
(use_declaration argument: (scoped_use_list path: (_) @use.base list: (use_list [
  ((identifier) @use.item)
  ((scoped_identifier) @use.item)
  (use_as_clause path: (_) @use.item)
  (use_wildcard (_) @use.item)
])))
(use_declaration argument: (scoped_use_list path: (_) @use.outer list: (use_list
  (scoped_use_list path: (_) @use.mid list: (use_list (self))))))
(use_declaration argument: (scoped_use_list path: (_) @use.base list: (use_list
  (scoped_use_list path: (_) @use.inner list: (use_list [
    ((identifier) @use.leaf)
    ((scoped_identifier) @use.leaf)
    (use_as_clause path: (_) @use.leaf)
    (use_wildcard (_) @use.leaf)
  ])))))
(extern_crate_declaration name: (identifier) @import)
(mod_item name: (identifier) @import !body)

; Definitions, with their parameters: functions and methods.
(function_item name: (identifier) @def parameters: (parameters) @def.params)

; String constants and statics, which a queue key or a URL may name.
(const_item name: (identifier) @const value: (string_literal (string_content) @const.value))
(static_item name: (identifier) @const value: (string_literal (string_content) @const.value))

; Calls, by the name they call, with their arguments: a function, a method, a path.
(call_expression
  function: [
    (identifier) @call
    (field_expression field: (field_identifier) @call)
    (scoped_identifier name: (identifier) @call)
    (generic_function function: [
      (identifier) @call
      (field_expression field: (field_identifier) @call)
      (scoped_identifier name: (identifier) @call)
    ])
  ]
  arguments: (arguments) @call.args)
