# AGENTS.md

Guidance for coding agents working in this repo — the standalone repo of the **para/html** Noeta package (server-driven reactive HTML / LiveView, pure Noeta), extracted from the noeta monorepo. Toolchain issues (the language, the `noeta` binary, `std.reactive`, `std.http`, expression tiers) belong in the monorepo at github.com/noeta-lang/noeta, not here.

## Versions that have to agree

- The package is at `0.6.1` and `noeta.toml` declares `toolchain = ">=0.6"`; check and test with a `noeta` of at least 0.6 (`noeta --version`).
- `crates/noeta-para-html` depends on `noeta-ext-abi = "0.6"` — deliberately a range, so a patch toolchain release costs no edit. A minor does. The `Toolchain pin` workflow proposes that bump on a noeta release; the point of that job is the build it runs, not the PR it opens.
- Never bump the version or move a tag as part of ordinary work — a release is its own commit plus a new `v*` tag.

## Behavior that changed recently — do not restate the old shape

- `serves(base, path)` under a root mount (`""` or `"/"`) claims exactly `/`, `/ws` and `/live.js`, nothing else. It used to answer true for *every* path, which made a root-mounted page swallow a host's `/health`. That is the 0.6.0 break.
- The client shim learns its socket URL from the `data-live-ws` attribute the server writes onto its own `<script>` tag; it does not assume `/ws`.
- The idle-tick cadence is an argument: `handle(req, "Name", page, every_ms: 250, on_tick: drain)`. There is no `handle_every`.

## Build & test

- `noeta test html.noe` at the repo root runs the package's own `@test`s (mount URLs, `serves`, the shim's socket attribute). **CI does not run them** — its `examples` job only walks `examples/*/` — so run it by hand after touching `html.noe`.
- Per example: `noeta check <file>.noe && noeta test <file>.noe` from inside `examples/<name>/`. `noeta test` never runs top-level statements, so browser-serving examples are safe to test.
- `noeta serve <file>.noe` blocks on a real socket — don't start one in an automated session unless you mean to. `examples/liveview-events` is the browserless exercise of the event model (`noeta run`).
- The only cargo here is the dev-only tier-body formatters. `crates/noeta-para-html/` and `native/` are each their own workspace root, so run per directory: `cargo +1.97.0 fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Then, in `crates/noeta-para-html/`: `cargo clippy --all-targets --features fmt -- -D warnings && cargo test --features fmt`. The formatters sit behind the non-default `fmt` feature, so a bare `cargo test` there runs **zero** tests and gates nothing.
- To build those crates against an unreleased toolchain, add a `[patch.crates-io] noeta-ext-abi = { path = … }` in your own checkout only — it hardcodes a machine-specific path and must not be committed.

## Conventions

- A tier is **bound, never imported**: `use para.html` brings in the functions, but without `html = "para/html"` in the consumer's `[directives]` an `@html { … }` block is not an expression. Same for `css`.
- No lockfile in this repo is committed — `examples/*/noeta.lock` and the crates' `Cargo.lock` are gitignored and regenerate. The package root has none at all; it has no dependencies.
- **American English** throughout, in code, comments and docs (`behavior`, not `behaviour`). Markdown never hard-wraps lines.
- **Conventional commits** for every title. Commit each green slice as it completes, but **never `git push` without explicit authorization**.
- Implement in full — no stubs or TODOs; new functionality lands with tests.
- Keep `README.md` and this file true when layout or behavior changes.

## CI

`ci.yml` gates the Rust crates (fmt / clippy / test, plus the `fmt`-feature build) and checks and tests every example against a released `noeta` pinned by the org-level `NOETA_VERSION` variable; it is also `workflow_dispatch`-able, because a dropped push webhook must not be able to block a release. `release.yml` publishes a `v*` tag to the hosted registry with keyless Sigstore provenance. Both are green today.
