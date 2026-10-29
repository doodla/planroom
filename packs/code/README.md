# The code pack

The code pack is the base every other pack builds on. It names the parts of a software system's code: repos, containers, directories and files, and the relations between them. It requires no other pack.

Planroom embeds it in the binary, so every workspace loads it first.

## What it declares

| Table | What the code pack puts there |
|---|---|
| `[[levels]]` | the ladder of abstraction levels, top first: `system`, `container`, `component`, `code`; another pack may put a level below one of them, never below `code`. That level is checked when the pack loads, but reads show only these four until they load the workspace's pack set ([Levels and roll-ups](../../docs/contributing/architecture.md#levels-and-roll-ups)) |
| `[[kinds]]` | `workspace`, `repo`, `service`, `datastore`, `queue`, `library`, `tooling`, `package`, `module`, `directory`, `file`, `api_spec`, `endpoint`, `person`, `commit`. The kinds at the `container` level use the kernel's `container` ID prefix and have a tone, the block color surfaces draw it in: `service` for a service, `store` for a datastore or a queue, `library` for a library, `infra` for tooling. |
| `[[relations]]` | `http`, `queue_publish`, `queue_consume`, `db`, `imports`, `depends_on_package`, `co_change`, `owned_by` |
| `[[languages]]` | TypeScript, TSX, JavaScript, Python, Go, Rust, YAML and JSON, by file extension. Each of the six with a grammar has its own file in `languages/`, with its extractors and the globs of its test files, `tests` (see [Test files](#test-files)) |
| `[[image_roles]]` | `postgres*` and `mysql*` are a `datastore`; `redis*` and `rabbitmq*` are a `queue` |
| `[[hints]]` | `url`, `env_read`, `env_set`, `depends_on`, `route_serve`, `route_call`, `queue_op`, `db_open`, `sql`, `def`, `call`, `compose_service`, `const`, `test_scope`, `import` (the specifier as written, with Python's `.` or Rust's `::` as its `separator`) |
| `[[extractors]]` | the `files`, `compose`, `manifest` and `openapi` built-ins; `urls` and `sql`, regexes over the code of the five languages; and in each language's file, a query for its imports (as `import` hints), definitions and calls and one for the hints a deployment joins, from `queries/` |
| `[[linkers]]` | the `runtime_wiring` built-in, which joins the hints of every repo into containers, endpoints and the `http`, `queue_publish`, `queue_consume` and `db` edges between them, binding a wrapper's method and path at its call sites; and the `imports` built-in, which joins each import of another workspace package to that package as an `imports` edge, and gives the importing manifest's `depends_on_package` edge the import lines that use it. Relative and same-package imports join nothing yet |
| `[[lenses]]` | `runtime` (the default), `flow`, `dependencies`, `history` |
| `[[questions]]` | `path A B`, a shortest path over `flow`; `why A B`, every edge between the two with its evidence and confidence; `impact X`, what depends on X and what it depends on, over `runtime` and `dependencies`. `planroom ask` lists them with every loaded kind and lens |
| `[[suggestions]]` | the pane's next action: open the containers when the map has any, else the components, else the files |

A compose service whose image no image role matches is a `service`. A repo whose root manifest, `package.json`, `go.mod`, `pyproject.toml` or `Cargo.toml`, names its package is a `library` until a compose service runs it.

## Languages and queries

| Path | What it holds |
|---|---|
| `languages/<id>.toml` | one language with a built-in grammar: its `[[languages]]` entry and the extractors that read it |
| `queries/<language>.scm` | the imports, definitions, string constants and calls of a language, which TypeScript and TSX read with JavaScript's |
| `queries/<language>_runtime.scm` | the hints a deployment joins: environment reads, routes served and called, queue operations and database opens; and Rust's test scopes |

A query never reads a comment or a docstring: a match that captures one is dropped, and a capture's text has them blanked. Each emit in a language's file names only captures its query sets.

### Test files

A test exercises your runtime; it isn't your runtime's wiring. Each language's `tests` globs name its test files, and no linker reads a hint from one: a test's queue push, call, database open or environment read joins nothing, and a test that calls your code is never cited as its caller. The definitions, imports and calls in a test file are still extracted. A file is a test file when its extension gives the language and one of that language's globs matches its path in the repo.

Test code inside a runtime file counts too, where the pack names it. In Rust, each item `#[cfg(test)]` marks, such as `#[cfg(test)] mod tests { ... }`, a test function, an impl, a `use`, a `const`, a statement or a match arm, is a test scope from the attribute's line to the item's last line, and so is a module body or a file an inner `#![cfg(test)]` marks, and a function `#[test]` marks, or an attribute whose path's last segment is `test`, such as `#[tokio::test]`. No linker reads a hint on a line a test scope holds, as none reads one from a test file. A module `#[cfg(test)]` marks at a file's top level is test code in whole, in the files Rust reads it from (a module declared inline has none of its own, so only the files of the modules it declares out of line, below `<name>/`, are its): the file its `#[path]` names, relative to the declaring file's folder; or else `<name>.rs` and every file below `<name>/`, in the declaring file's folder when that file is a `mod.rs`, `lib.rs`, `main.rs` or `build.rs` or a binary in `src/bin/` or `examples/`, and in the folder named for its stem otherwise. So `#[cfg(test)] mod test;` in `src/lib.rs` makes `src/test.rs` and `src/test/` test code. The file of a test module declared inside another module, and of a module a `#[path]` test module declares, counts only as the test globs and its own test scopes say. Of the `cfg` attributes, only a bare `cfg(test)` marks a scope: `#[cfg(all(test, unix))]` and `#[cfg(not(test))]` don't. Inline test code in the other languages, such as Vitest's in-source `if (import.meta.vitest)` block, is read as runtime code.

| Language | Its test files |
|---|---|
| Python | `**/tests/**`, `**/test/**`, `**/e2e/**`, `**/test_*.py`, `**/*_test.py`, `**/tests.py`, `**/conftest.py` |
| Go | `**/*_test.go`, `**/testdata/**`, `**/e2e/**` |
| TypeScript, TSX and JavaScript | `**/*.test.*`, `**/*.spec.*`, `**/*.cy.*`, `**/__tests__/**`, `**/__mocks__/**`, `**/test/**`, `**/tests/**`, `**/e2e/**`, `**/cypress/**` |
| Rust | `**/tests/**`, `**/benches/**`, `**/tests.rs` |

YAML and JSON name no test files, so a compose file is read wherever it sits. A glob that doesn't parse is refused when the pack loads.

## Extend it

You don't edit this pack to add a language or a domain. You write a pack of your own that requires `code`.

Check your pack with `planroom doctor --pack DIR`, as [Test it](#test-it) shows for this pack. Then add it to a workspace with `planroom pack add DIR`, which only a person runs: it copies the pack into the workspace's state directory, and every later scan loads that copy. `planroom pack remove ID` takes it out again. [`examples/packs/ruby`](../../examples/packs/ruby/README.md) is such a pack.

Your pack may:

- name the code pack's kinds and relations, qualified, such as `code.file`, and its lenses by ID, such as `runtime`, in a question step, a rule, a decision effect or a `when` predicate, under the keys its shape, rule type, effect or the predicate declares;
- name its lenses and IDs in a suggestion's action, whose arguments must parse as the `planroom` CLI parses them;
- declare a lens of its own, under any ID but `all`, which `--lens all` already takes for every relation of every loaded pack;
- write a literal node ID whose kind-short is one of the code pack's ID prefixes, such as `web:file:src/app.ts`;
- add a language whose extensions no loaded pack claims;
- add an image role for a glob no loaded pack claims;
- name the test files of a language you add in its `tests`, which `runtime_wiring` then reads no hint from;
- emit the code pack's hint kinds, such as `code.url`, which `runtime_wiring` joins. A hint kind your own pack declares is refused, because no loaded linker would join it.

A question step's `$NAME`, whole or inside an ID, must be one of the question's `args`. A key that names nothing, such as `depth` or `direction`, takes a number, a word or text, or a list of them, never a table.

A key no table declares, anywhere in your pack's TOML files, is refused, and the error names that key's line.

## Test it

Run the conformance kit from the repository root:

```sh
planroom doctor --pack packs/code
```

Build Planroom from this checkout first: the kit checks a directory under the code pack's ID only when its files are the ones the binary embeds, and refuses any other. It validates the pack, runs every extractor over each repo in `tests/inputs/`, and compares the facts with `tests/expected.jsonl`. It then runs them again and checks that the facts match byte for byte.

When you change what the pack emits on purpose, rewrite the expected facts:

```sh
planroom doctor --pack packs/code --update
```

`crates/planroom/tests/packs.rs` runs the same kit on every pack, and checks the rules every pack follows.
