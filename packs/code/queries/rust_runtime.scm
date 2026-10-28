; Rust: the hints a deployment joins: environment reads, routes served and called, queue
; operations and database opens; and the test code inside a file, which no join reads. Each pattern sets its own captures, and each emit in
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

; Test code inside a file: an item `#[cfg(test)]` marks (a module, a function, an impl, a use, a
; const, a statement, a match arm or any other), from the attribute's line to the item's last, whatever attributes and
; comments stand between them; and the module body or the file an inner `#![cfg(test)]` marks.
; The emit's `$end` is the last line the match reaches, which is the item's. A match whose
; `@test.item` is one of the attributes between gives a shorter span inside the item's; the emit
; keys each span by its last line, so the item's own is never merged into a shorter one.
((attribute_item
   (attribute
     (identifier) @_cfg
     arguments: (token_tree . (identifier) @test .)))
 .
 [(attribute_item) (line_comment) (block_comment)]*
 .
 (_) @test.item
 (#eq? @_cfg "cfg")
 (#eq? @test "test"))
((source_file
   (inner_attribute_item
     (attribute
       (identifier) @_cfg
       arguments: (token_tree . (identifier) @test .)))) @test.item
 (#eq? @_cfg "cfg")
 (#eq? @test "test"))
((declaration_list
   (inner_attribute_item
     (attribute
       (identifier) @_cfg
       arguments: (token_tree . (identifier) @test .)))) @test.item
 (#eq? @_cfg "cfg")
 (#eq? @test "test"))
; A test function: an item `#[test]` marks, or an attribute whose path's last segment is `test` does, such as
; `#[tokio::test]`. Rust builds one only into its test harness, as it builds `cfg(test)` code.
((attribute_item
   (attribute [(identifier) @test (scoped_identifier name: (identifier) @test)]))
 .
 [(attribute_item) (line_comment) (block_comment)]*
 .
 (_) @test.item
 (#eq? @test "test"))
; A module `#[cfg(test)]` marks at a file's top level: runtime_wiring reads every file Rust reads
; it from as test code. Out of line, that is the file its name gives, `@test.module`, or the file
; its `#[path = "..."]` names, `@test.path`, beside `#[cfg(test)]` on either side of it. Inline,
; `@test.inline`, the module has no file of its own, and only the files of the modules it declares
; out of line, below the directory its name gives, are read. Each match ends on the module's name,
; so a declaration's scopes share their `$end`.
((source_file
   (attribute_item
     (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @_test .)))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (mod_item name: (identifier) @test.module !body))
 (#eq? @_cfg "cfg")
 (#eq? @_test "test"))
((source_file
   (attribute_item
     (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @_test .)))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (mod_item name: (identifier) @test.inline body: (declaration_list)))
 (#eq? @_cfg "cfg")
 (#eq? @_test "test"))
((source_file
   (attribute_item
     (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @_test .)))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (attribute_item
     (attribute
       (identifier) @_path
       value: [(string_literal (string_content) @test.path) (raw_string_literal (string_content) @test.path)]))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (mod_item name: (identifier) @test.module !body))
 (#eq? @_cfg "cfg")
 (#eq? @_test "test")
 (#eq? @_path "path"))
((source_file
   (attribute_item
     (attribute
       (identifier) @_path
       value: [(string_literal (string_content) @test.path) (raw_string_literal (string_content) @test.path)]))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (attribute_item
     (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @_test .)))
   .
   [(attribute_item) (line_comment) (block_comment)]*
   .
   (mod_item name: (identifier) @test.module !body))
 (#eq? @_cfg "cfg")
 (#eq? @_test "test")
 (#eq? @_path "path"))
; A match arm or a field's initializer holds its own attributes, so the arm or the initializer is
; the item.
([(match_arm
    (attribute_item
      (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @test .))))
  (field_initializer
    (attribute_item
      (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @test .))))
  (shorthand_field_initializer
    (attribute_item
      (attribute (identifier) @_cfg arguments: (token_tree . (identifier) @test .))))] @test.item
 (#eq? @_cfg "cfg")
 (#eq? @test "test"))
