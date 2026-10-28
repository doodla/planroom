; Python: the hints a deployment joins: environment reads, routes served and called, queue
; operations and database opens. Each pattern sets its own captures, and each emit in
; languages/python.toml names a capture only its patterns set. A capture's text is the source with
; its comments and docstrings blanked, and a match that captures a comment or a docstring is dropped.

; Environment reads: os.environ.get("X"), os.getenv("X"), os.environ["X"], and the same on a
; mapping named environ or env; a second argument is the default.
(call
  function: (attribute object: (_) @_environ attribute: (identifier) @_get)
  arguments: (argument_list . (string . (string_start) . (string_content) @env . (string_end)) .)
  (#match? @_environ "^(os\\.environ|environ|env|os)$")
  (#match? @_get "^(get|getenv)$"))
(call
  function: (attribute object: (_) @_environ attribute: (identifier) @_get)
  arguments: (argument_list
    .
    (string . (string_start) . (string_content) @env . (string_end))
    .
    [(string (string_content) @env.default) (identifier) @env.default (integer) @env.default (attribute) @env.default]
    .)
  (#match? @_environ "^(os\\.environ|environ|env|os)$")
  (#match? @_get "^(get|getenv)$"))
(subscript
  value: (_) @_environ
  subscript: (string . (string_start) . (string_content) @env . (string_end))
  (#match? @_environ "^(os\\.environ|environ|env)$"))

; Routes a server serves: @app.get("/path"), @router.post("/path").
(decorator
  (call
    function: (attribute attribute: (identifier) @serve.method)
    arguments: (argument_list . (string . (string_start) . (string_content) @serve.path . (string_end))))
  (#match? @serve.method "^(get|post|put|patch|delete|head|options)$")
  (#match? @serve.path "^/"))

; Routes a client calls: requests.get(url), self._session.post(url), and urllib's Request with
; its method; the method or the path may be a parameter, which a linker binds per call site.
(call
  function: (attribute object: (_) @_client attribute: (identifier) @client.method)
  arguments: (argument_list . [(string) (identifier) (attribute) (binary_operator) (call)] @client.path)
  (#match? @_client "(^|[._])(requests|httpx|session|client|http)$")
  (#match? @client.method "^(get|post|put|patch|delete|head)$"))
(call
  function: (attribute object: (_) @_urllib attribute: (identifier) @_request)
  arguments: (argument_list
    .
    (_) @fetch.path
    (keyword_argument name: (identifier) @_method value: (_) @fetch.method))
  (#match? @_urllib "(^|\\.)request$")
  (#eq? @_request "Request")
  (#eq? @_method "method"))

; Queue operations, by the key or the list of keys they name first.
(call
  function: (attribute attribute: (identifier) @_op)
  arguments: (argument_list . [(string (string_content) @queue.publish) (identifier) @queue.publish])
  (#match? @_op "(?i)^(lpush|rpush|publish|xadd)$"))
(call
  function: (attribute attribute: (identifier) @_op)
  arguments: (argument_list
    .
    [
      (string (string_content) @queue.consume)
      (identifier) @queue.consume
      (list . [(string (string_content) @queue.consume) (identifier) @queue.consume])
    ])
  (#match? @_op "(?i)^(brpop|blpop|rpop|lpop|brpoplpush|blmove|subscribe|xread|xreadgroup)$"))

; Database connections opened: psycopg.connect(...), asyncpg.create_pool(...).
(call
  function: (attribute object: (identifier) @db.driver attribute: (identifier) @_open)
  (#match? @db.driver "^(psycopg|psycopg2|asyncpg|aiopg|pymysql|MySQLdb|sqlite3|pymongo)$")
  (#match? @_open "^(connect|create_pool|MongoClient)$"))
