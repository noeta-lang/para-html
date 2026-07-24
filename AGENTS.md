# AGENTS.md

Guidance for coding agents working in this repo — the standalone repo of the **para/html** Noeta package (server-side reactive HTML / LiveView, pure Noeta), extracted from the noeta monorepo. Toolchain issues (the language, the `noeta` binary, `std.reactive`, `std.http`, expression tiers) belong in the monorepo at github.com/noeta-lang/noeta, not here.

## Repo layout

- `noeta.toml` — the package manifest (`name = "para/html"`). No `native` key: this package is pure Noeta.
- `html.noe` — the whole surface: the `@html` tier handler (`render`), the `Html` template value, `handle` (the websocket diff-push session), typed event binding (`DomEvent`/`Binding`/`on_click`/`on_input`/`on_submit`), and keyed-list reconciliation (`keyed`, `reconcile_region`).
- `docs/LiveView.md` — the design write-up (GitHub-Wiki-style page, originally from the monorepo wiki).
- `examples/*/` — each a standalone package depending on this repo via `para = { path = "../.." }`, with its own committed `noeta.lock`.
- `.github/workflows/` — CI (`ci.yml`) and the tag-triggered registry publish (`release.yml`).

## Build & test

Pure Noeta — no cargo anywhere in this repo.

- `noeta check <file>.noe` / `noeta test <file>.noe` in each `examples/*` directory is the test suite. `noeta test` never runs top-level statements, so browser-serving examples are safe to test.
- `liveview-events` is the browserless exercise of the event model (runnable with `noeta run`); the browser examples are served with `noeta serve <file>.noe`, which blocks on a real socket — don't run them in an automated session unless you intend to.

## Conventions

- `noeta.lock` files under `examples/` **are committed** — leave resolved locks in place.
- Markdown never hard-wraps lines.
- **American English** throughout — code, comments, and docs (`behavior`, not `behaviour`).
- **Conventional commits** for all commit titles. Commit each green slice as it completes, but **never `git push` without explicit authorization**. Never move a published `v*` tag — a release is a new tag.
- Implement in full — no stubs or TODOs; new functionality lands with tests.
- Keep `README.md` and this file up to date when layout or behavior changes.

## CI

`ci.yml` checks and tests every example with a pinned released `noeta`; `release.yml` publishes the tag to the hosted registry (`noeta publish`, keyless Sigstore provenance via GitHub OIDC). Both go green only once the toolchain repo is published under github.com/noeta-lang/noeta.
