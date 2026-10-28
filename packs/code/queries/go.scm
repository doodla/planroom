; Go: imports, definitions and calls. Each pattern sets its own captures, and each emit in
; languages/go.toml names a capture only its patterns set. A capture's text is the source with
; its comments blanked, and a match that captures a comment is dropped. go_runtime.scm reads the
; hints a deployment joins.

; Imports.
(import_spec path: (interpreted_string_literal (interpreted_string_literal_content) @import))

; Definitions, with their parameters: functions and methods.
(function_declaration name: (identifier) @def parameters: (parameter_list) @def.params)
(method_declaration name: (field_identifier) @def parameters: (parameter_list) @def.params)

; String constants and variables, which a queue key or a URL may name.
(const_spec
  name: (identifier) @const
  value: (expression_list . [
    (interpreted_string_literal (interpreted_string_literal_content) @const.value)
    (raw_string_literal (raw_string_literal_content) @const.value)
  ] .))
(var_spec
  name: (identifier) @const
  value: (expression_list . [
    (interpreted_string_literal (interpreted_string_literal_content) @const.value)
    (raw_string_literal (raw_string_literal_content) @const.value)
  ] .))

; Calls, by the name they call, with their arguments.
(call_expression
  function: [
    (identifier) @call
    (selector_expression field: (field_identifier) @call)
  ]
  arguments: (argument_list) @call.args)
