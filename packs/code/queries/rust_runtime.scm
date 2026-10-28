; Rust: the hints a deployment joins: environment reads, routes served and called, queue
; operations and database opens. Each pattern sets its own captures, and each emit in
; languages/rust.toml names a capture only its patterns set. A capture's text is the source with
; its comments, doc comments included, blanked, and a match that captures a comment is dropped.

; Environment reads: std::env::var("X"), env::var_os("X").
(call_expression
  function: (scoped_identifier path: (_) @_env name: (identifier) @_var)
  arguments: (arguments . (string_literal (string_content) @env))
  (#match? @_env "(^|::)env$")
  (#match? @_var "^(var|var_os)$"))

; Routes a router serves: axum's .route("/path", get(handler)), and actix's #[get("/path")].
(call_expression
  function: (field_expression field: (field_identifier) @_route)
  arguments: (arguments
    .
    (string_literal (string_content) @serve.path)
    .
    (call_expression function: (identifier) @serve.method))
  (#eq? @_route "route")
  (#match? @serve.method "^(get|post|put|patch|delete|head|options)$"))
(attribute_item
  (attribute
    (identifier) @serve.method
    arguments: (token_tree . (string_literal (string_content) @serve.path)))
  (#match? @serve.method "^(get|post|put|patch|delete|head|options)$")
  (#match? @serve.path "^/"))

; Routes a client calls: client.get(url), reqwest::get(url).
(call_expression
  function: (field_expression value: (_) @_client field: (field_identifier) @client.method)
  arguments: (arguments . (_) @client.path)
  (#match? @_client "(^|[._])(client|http|reqwest)$")
  (#match? @client.method "^(get|post|put|patch|delete|head)$"))
(call_expression
  function: (scoped_identifier path: (identifier) @_reqwest name: (identifier) @client.method)
  arguments: (arguments . (_) @client.path)
  (#eq? @_reqwest "reqwest")
  (#match? @client.method "^(get|post)$"))

; Queue operations, as redis-rs takes them: the key first.
(call_expression
  function: (field_expression field: (field_identifier) @_op)
  arguments: (arguments . [(string_literal (string_content) @queue.publish) (identifier) @queue.publish])
  (#match? @_op "^(lpush|rpush|publish|xadd)$"))
(call_expression
  function: (field_expression field: (field_identifier) @_op)
  arguments: (arguments . [(string_literal (string_content) @queue.consume) (identifier) @queue.consume])
  (#match? @_op "^(brpop|blpop|rpop|lpop|subscribe|xread)$"))

; Database connections opened: PgPool::connect(url), tokio_postgres::connect(url, tls).
(call_expression
  function: (scoped_identifier path: (_) @db.driver name: (identifier) @_open)
  (#match? @db.driver "(Pg|Postgres|MySql|Mysql|Sqlite|Pool|postgres|mysql|sqlite|mongodb)[A-Za-z_]*$")
  (#match? @_open "^(connect|connect_lazy|connect_with|establish)$"))
