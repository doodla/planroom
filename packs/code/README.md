# The code pack

The code pack is the base every other pack builds on. It names the parts of a software system's code: repos, containers, directories and files, and the relations between them. It requires no other pack.

Planroom embeds it in the binary, so every workspace loads it first.

## What it declares

| Table | What the code pack puts there |
|---|---|
| `[[kinds]]` | `workspace`, `repo`, `service`, `datastore`, `queue`, `library`, `tooling`, `package`, `module`, `directory`, `file`, `api_spec`, `endpoint`, `person`, `commit`. Every Container-level kind uses the kernel's `container` ID prefix. |
| `[[relations]]` | `http`, `queue_publish`, `queue_consume`, `db`, `imports`, `depends_on_package`, `co_change`, `owned_by` |
| `[[languages]]` | TypeScript, TSX, JavaScript, Python, Go, Rust, YAML and JSON, by file extension |
| `[[image_roles]]` | `postgres*` and `mysql*` are a `datastore`; `redis*` and `rabbitmq*` are a `queue` |
| `[[hints]]` | `url`, `env_read`, `env_set`, `depends_on`, `route_serve`, `route_call`, `queue_op`, `db_open`, `sql`, `def`, `call` |
| `[[extractors]]` | the `files` and `compose` built-ins |
| `[[lenses]]` | `runtime` (the default), `flow`, `dependencies`, `history` |

A compose service whose image no image role matches is a `service`.

## Extend it

You don't edit this pack to add a language or a domain. You write a pack of your own that requires `code`.

This build loads only the built-in packs, so a workspace can't load your pack yet. Today you check it with `planroom doctor --pack DIR`, as [Test it](#test-it) shows for this pack. The person-only `planroom pack add DIR`, which adds a pack to a workspace, is planned for M1 (see [M1](../../docs/contributing/architecture.md#m1-kernel-store-code-and-plan-packs-mod-and-canvas-basics-122-h) in Architecture).

Your pack may:

- name the code pack's kinds and relations, qualified, such as `code.file`, and its lenses by ID, such as `runtime`, in a question step, a rule, a decision effect or a `when` predicate, under the keys its shape, rule type, effect or the predicate declares;
- name its lenses and IDs in a suggestion's action, whose arguments must parse as the `planroom` CLI parses them;
- write a literal node ID whose kind-short is one of the code pack's ID prefixes, such as `web:file:src/app.ts`;
- add a language whose extensions no loaded pack claims;
- add an image role for a glob no loaded pack claims;
- emit the code pack's hint kinds, such as `code.url`. The code pack declares no `[[linkers]]` yet, and this build has no built-in linker, so nothing joins those hints today. A hint kind your own pack declares is refused, because no loaded linker would join it.

A key no table declares, anywhere in your pack's TOML files, is refused, and the error names that key's line.

## Test it

Run the conformance kit from the repository root:

```sh
planroom doctor --pack packs/code
```

Build Planroom from this checkout first: the kit checks a directory under the code pack's ID only when its files are the ones the binary embeds, and refuses any other. It validates the pack, runs `files` and `compose` over each repo in `tests/inputs/`, and compares the facts with `tests/expected.jsonl`. It then runs them again and checks that the facts match byte for byte.

When you change what the pack emits on purpose, rewrite the expected facts:

```sh
planroom doctor --pack packs/code --update
```

`crates/planroom/tests/packs.rs` runs the same kit on every pack, and checks the rules every pack follows.
