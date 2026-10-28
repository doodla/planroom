; JavaScript, TypeScript and TSX: the hints a deployment joins: environment reads, routes served
; and called, queue operations and database opens. Each pattern sets its own captures, and each
; emit in languages/*.toml names a capture only its patterns set. A capture's text is the source
; with its comments blanked, and a match that captures a comment is dropped.

; Environment reads: process.env.NAME, process.env['NAME'], and env.NAME on a parameter that
; stands for it; a default after ?? or ||.
(member_expression
  object: (member_expression object: (identifier) @_process property: (property_identifier) @_env)
  property: (property_identifier) @env
  (#eq? @_process "process")
  (#eq? @_env "env"))
(subscript_expression
  object: (member_expression object: (identifier) @_process property: (property_identifier) @_env)
  index: (string (string_fragment) @env)
  (#eq? @_process "process")
  (#eq? @_env "env"))
(member_expression
  object: (identifier) @_env
  property: (property_identifier) @env
  (#eq? @_env "env"))
(binary_expression
  left: [
    (member_expression
      object: (member_expression object: (identifier) @_process property: (property_identifier) @_env)
      property: (property_identifier) @env)
    (member_expression object: (identifier) @_env property: (property_identifier) @env)
  ]
  operator: ["??" "||"]
  right: [(string (string_fragment) @env.default) (number) @env.default]
  (#match? @_env "^env$"))

; Routes a server serves: app.get('/path', ...), router.post('/path', ...).
(call_expression
  function: (member_expression object: (identifier) @_server property: (property_identifier) @serve.method)
  arguments: (arguments . (string (string_fragment) @serve.path))
  (#match? @_server "^(app|server|fastify|api|router|[A-Za-z_$]*Router)$")
  (#match? @serve.method "^(get|post|put|patch|delete|head|options|all)$")
  (#match? @serve.path "^/"))

; Routes a client calls: fetch with a method, fetch alone (a GET), and axios.
(call_expression
  function: (identifier) @_fetch
  arguments: (arguments
    .
    [(string) (template_string) (identifier) (member_expression) (binary_expression)] @fetch.path
    .
    (object (pair key: (property_identifier) @_method value: [(string) (identifier) (member_expression)] @fetch.method)))
  (#eq? @_fetch "fetch")
  (#eq? @_method "method"))
(call_expression
  function: (identifier) @_fetch
  arguments: (arguments . [(string) (template_string) (identifier) (member_expression) (binary_expression)] @get.path .)
  (#eq? @_fetch "fetch"))
(call_expression
  function: (member_expression object: (identifier) @_axios property: (property_identifier) @client.method)
  arguments: (arguments . [(string) (template_string) (identifier) (member_expression) (binary_expression)] @client.path)
  (#eq? @_axios "axios")
  (#match? @client.method "^(get|post|put|patch|delete|head)$"))

; Queue operations, by the key or the list of keys they name first.
(call_expression
  function: (member_expression property: (property_identifier) @_op)
  arguments: (arguments . [(string (string_fragment) @queue.publish) (identifier) @queue.publish])
  (#match? @_op "(?i)^(lpush|rpush|publish|xadd)$"))
(call_expression
  function: (member_expression property: (property_identifier) @_op)
  arguments: (arguments
    .
    [
      (string (string_fragment) @queue.consume)
      (identifier) @queue.consume
      (array . [(string (string_fragment) @queue.consume) (identifier) @queue.consume])
    ])
  (#match? @_op "(?i)^(brpop|blpop|rpop|lpop|brpoplpush|blmove|subscribe|xread|xreadgroup)$"))

; Database clients opened.
(new_expression
  constructor: [(identifier) (member_expression)] @db.driver
  (#match? @db.driver "^((pg\\.)?Pool|pg\\.Client|PrismaClient|Sequelize|MongoClient)$"))
(call_expression
  function: (member_expression object: (identifier) @db.driver property: (property_identifier) @_open)
  (#match? @db.driver "^(mysql|mysql2|mongoose|knex|postgres)$")
  (#match? @_open "^(createPool|createConnection|connect)$"))
