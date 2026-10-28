; Python: imports, definitions and calls. Each pattern sets its own captures, and each emit in
; languages/python.toml names a capture only its patterns set. A capture's text is the source with
; its comments and docstrings blanked, and a match that captures a comment or a docstring is dropped. python_runtime.scm reads the
; hints a deployment joins.

; Imports: import a.b, import a.b as c, from a.b import c, from . import c.
(import_statement name: (dotted_name) @import)
(import_statement name: (aliased_import name: (dotted_name) @import))
(import_from_statement module_name: [(dotted_name) (relative_import)] @import)

; Definitions, with their parameters.
(function_definition name: (identifier) @def parameters: (parameters) @def.params)

; String constants at the top of a module, which a queue key or a URL may name.
(module
  (expression_statement
    (assignment
      left: (identifier) @const
      right: (string . (string_start) . (string_content) @const.value . (string_end)))))

; Calls, by the name they call, with their arguments.
(call
  function: [
    (identifier) @call
    (attribute attribute: (identifier) @call)
  ]
  arguments: [(argument_list) (generator_expression)] @call.args)
