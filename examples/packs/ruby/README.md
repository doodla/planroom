# The Ruby pack

An out-of-tree pack that maps Ruby scripts with no Ruby in the engine. Planroom has no built-in
Ruby extractor, so this pack is data only: one `[[languages]]` entry and two regex extractors.

## What it covers

| Part | What it gives the map |
|---|---|
| `[[languages]]` `ruby` | each `.rb` file's `language`, so a repo with Ruby files lists `ruby` among its languages |
| extractor `ruby-script` | a `code.tooling` container per script, named by the script's stem, under its repo; the script's `code.file` under it; and a `code.imports` edge for each `require '...'` |
| extractor `ruby-http` | a `code.url` hint for each quoted `http://host:port/path` literal, which the code pack's `runtime_wiring` linker joins to the service that serves that host and path |

So the fixture's `platform/tools/healthcheck.rb` becomes the `healthcheck` container, with an
`http` edge, `GET /healthz`, to `orders` and to `storefront`.

## What it requires

`code`, the built-in base pack: the kinds, the relation and the `code.url` hint kind it emits are
the code pack's.

## How to use it

Check it, with its conformance kit over `tests/inputs/`:

```sh
planroom doctor --pack examples/packs/ruby
```

Add it to a workspace, which copies it into the state directory and records it; a person's alone:

```sh
planroom pack add examples/packs/ruby
planroom scan
```

`planroom pack remove ruby` takes it out again. After an edit to this directory, the next scan
deals one card that asks whether to update the workspace's copy or keep it.

## How to test it

`tests/inputs/platform/tools/healthcheck.rb` is the fixture's script, and `tests/expected.jsonl` the
facts it gives, written by `planroom doctor --pack examples/packs/ruby --update`.
