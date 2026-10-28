; Go: the hints a deployment joins: environment reads, routes served and called, queue
; operations and database opens. Each pattern sets its own captures, and each emit in
; languages/go.toml names a capture only its patterns set. A capture's text is the source with
; its comments blanked, and a match that captures a comment is dropped.
; Routes served as "POST /orders" strings are read by go.toml's regex, which splits the method
; from the path.

; Environment reads: os.Getenv("X"), os.LookupEnv("X").
(call_expression
  function: (selector_expression operand: (identifier) @_os field: (field_identifier) @_getenv)
  arguments: (argument_list . (interpreted_string_literal (interpreted_string_literal_content) @env))
  (#eq? @_os "os")
  (#match? @_getenv "^(Getenv|LookupEnv)$"))

; Routes a router serves by its method's name: r.Get("/path", h), as chi and echo write them.
(call_expression
  function: (selector_expression field: (field_identifier) @serve.method)
  arguments: (argument_list . (interpreted_string_literal (interpreted_string_literal_content) @serve.path))
  (#match? @serve.method "^(Get|Post|Put|Patch|Delete|Head|Options|GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)$")
  (#match? @serve.path "^/"))

; Routes a client calls: http.Get(url), http.Post(url, ...), and a request built with its
; method; the method or the path may be a parameter, which a linker binds per call site.
(call_expression
  function: (selector_expression operand: (identifier) @_http field: (field_identifier) @client.method)
  arguments: (argument_list . (_) @client.path)
  (#eq? @_http "http")
  (#match? @client.method "^(Get|Post|Head|PostForm)$"))
(call_expression
  function: (selector_expression operand: (identifier) @_http field: (field_identifier) @_new)
  arguments: (argument_list . (_) @fetch.method . (_) @fetch.path)
  (#eq? @_http "http")
  (#eq? @_new "NewRequest"))
(call_expression
  function: (selector_expression operand: (identifier) @_http field: (field_identifier) @_new)
  arguments: (argument_list . (_) . (_) @fetch.method . (_) @fetch.path)
  (#eq? @_http "http")
  (#eq? @_new "NewRequestWithContext"))

; Queue operations, as go-redis takes them: the context, then the key.
(call_expression
  function: (selector_expression field: (field_identifier) @_op)
  arguments: (argument_list . (_) . [(identifier) @queue.publish (selector_expression) @queue.publish (interpreted_string_literal (interpreted_string_literal_content) @queue.publish)])
  (#match? @_op "^(LPush|RPush|LPushX|RPushX|Publish|SPublish)$"))
(call_expression
  function: (selector_expression field: (field_identifier) @_op)
  arguments: (argument_list . (_) . [(identifier) @queue.consume (selector_expression) @queue.consume (interpreted_string_literal (interpreted_string_literal_content) @queue.consume)])
  (#match? @_op "^(LPop|RPop|Subscribe|PSubscribe|SSubscribe|RPopLPush|LMove)$"))
(call_expression
  function: (selector_expression field: (field_identifier) @_op)
  arguments: (argument_list . (_) . (_) . [(identifier) @queue.consume (selector_expression) @queue.consume (interpreted_string_literal (interpreted_string_literal_content) @queue.consume)])
  (#match? @_op "^(BRPop|BLPop|BRPopLPush|BLMove)$"))

; Database connections opened: pgxpool.New(ctx, url), pgx.Connect(ctx, url), sql.Open(d, dsn).
(call_expression
  function: (selector_expression operand: (identifier) @db.driver field: (field_identifier) @_open)
  (#match? @db.driver "^(pgxpool|pgx|sql|sqlx|gorm|mongo)$")
  (#match? @_open "^(New|NewWithConfig|Connect|ConnectConfig|Open)$"))
