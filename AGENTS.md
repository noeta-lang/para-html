# AGENTS.md

Guidance for coding agents working in this repo — the standalone repo of the **para/html** Noeta package (server-side reactive HTML / LiveView, pure Noeta), extracted from the noeta monorepo. Toolchain issues (the language, the `noeta` binary, `std.reactive`, `std.http`, expression tiers) belong in the monorepo at github.com/noeta-lang/noeta, not here.

## Repo layout

- `noeta.toml` — the package manifest (`name = "para/html"`). No `native` key: the runtime surface is pure Noeta. It does carry `dev-native = "native"` — the trust-free, dev-toolchain-only tier-body formatters, which contribute nothing at run time and need no `[trust] native` from a consumer.
- `crates/noeta-para-html/` + `native/` — those formatters (`"html"` reflow, `"css"` via malva) and their extension entry crate. Each is its own Cargo workspace root; both depend on the published contract crate `noeta-ext-abi = "0.5"`.
- `html.noe` — the whole surface: the `@html` tier handler (`render`), the `Html` template value, `handle` (the websocket diff-push session), typed event binding (`DomEvent`/`Binding`/`on_click`/`on_input`/`on_submit`), and keyed-list reconciliation (`keyed`, `reconcile_region`).
- `docs/LiveView.md` — the design write-up (GitHub-Wiki-style page, originally from the monorepo wiki).
- `examples/*/` — each a standalone package depending on this repo via `para = { path = "../..", package = "para/html" }`, plus a `[directives]` table binding `html` (and, in `liveview-styled`, `css`) to `"para/html"`. A tier is **bound, never imported**: without that binding an `@html { … }` block is not an expression, whatever the file imports. Their `noeta.lock`s are gitignored, not committed.
- `.github/workflows/` — CI (`ci.yml`) and the tag-triggered registry publish (`release.yml`).

## Build & test

The runtime surface is pure Noeta; the only cargo is the dev-only formatter crates.

- `cargo +1.97.0 fmt --check` / `clippy --all-targets -- -D warnings` / `test` in `crates/noeta-para-html/` and `native/`, plus `cargo test --features fmt` — the formatters are behind the non-default `fmt` feature, so a bare `cargo test` in the impl crate runs **zero** tests and gates nothing.
- `noeta check <file>.noe` / `noeta test <file>.noe` in each `examples/*` directory is the test suite. `noeta test` never runs top-level statements, so browser-serving examples are safe to test.
- `liveview-events` is the browserless exercise of the event model (runnable with `noeta run`); the browser examples are served with `noeta serve <file>.noe`, which blocks on a real socket — don't run them in an automated session unless you intend to.

## Conventions

- A **package root** `noeta.lock` is committed; `examples/*/noeta.lock` are **not** — they are gitignored and regenerate on every run. (This file previously claimed the opposite; `.gitignore` and the git history were always the rule, and this now matches them.)
- Markdown never hard-wraps lines.
- **American English** throughout — code, comments, and docs (`behavior`, not `behaviour`).
- **Conventional commits** for all commit titles. Commit each green slice as it completes, but **never `git push` without explicit authorization**. Never move a published `v*` tag — a release is a new tag.
- Implement in full — no stubs or TODOs; new functionality lands with tests.
- Keep `README.md` and this file up to date when layout or behavior changes.

## CI

`ci.yml` gates the Rust formatter crates (fmt / clippy / test, plus the `fmt`-feature build) and checks and tests every example with a pinned released `noeta`; `release.yml` publishes the tag to the hosted registry (`noeta publish`, keyless Sigstore provenance via GitHub OIDC). Both go green only once the toolchain repo is published under github.com/noeta-lang/noeta.
