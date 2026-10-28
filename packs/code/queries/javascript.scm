; JavaScript, TypeScript and TSX: imports, definitions and calls. TypeScript's grammar extends
; JavaScript's, so one query reads all three. Each pattern sets its own captures, and each emit
; in languages/*.toml names a capture only its patterns set, so a match makes only the facts its
; pattern is for. A capture's text is the source with its comments blanked, and a match that
; captures a comment is dropped. javascript_runtime.scm reads the hints a deployment joins.

; Imports: ES modules, re-exports, require() and import().
(import_statement source: (string (string_fragment) @import))
(export_statement source: (string (string_fragment) @import))
(call_expression
  function: (identifier) @_require
  arguments: (arguments . (string (string_fragment) @import) .)
  (#eq? @_require "require"))
(call_expression
  function: (import)
  arguments: (arguments . (string (string_fragment) @import)))

; Definitions, with their parameters; an arrow function's lone parameter is put in parentheses.
(function_declaration name: (identifier) @def parameters: (formal_parameters) @def.params)
(generator_function_declaration name: (identifier) @def parameters: (formal_parameters) @def.params)
(method_definition name: (property_identifier) @def parameters: (formal_parameters) @def.params)
(variable_declarator
  name: (identifier) @def
  value: [
    (arrow_function parameters: (formal_parameters) @def.params)
    (function_expression parameters: (formal_parameters) @def.params)
  ])
(variable_declarator
  name: (identifier) @arrow
  value: (arrow_function parameter: (identifier) @arrow.param))

; String constants at the top of a module, which a queue key or a URL may name.
(program
  [
    (lexical_declaration (variable_declarator name: (identifier) @const value: (string (string_fragment) @const.value)))
    (variable_declaration (variable_declarator name: (identifier) @const value: (string (string_fragment) @const.value)))
    (export_statement
      declaration: (lexical_declaration
        (variable_declarator name: (identifier) @const value: (string (string_fragment) @const.value))))
  ])

; Calls, by the name they call, with their arguments.
(call_expression
  function: [
    (identifier) @call
    (member_expression property: (property_identifier) @call)
  ]
  arguments: (arguments) @call.args)
