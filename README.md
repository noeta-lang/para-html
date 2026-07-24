# para/html

Server-side reactive HTML — LiveView/LiveWire-style reactive templating, in pure Noeta. A page is an `@html { … }` template over `std.reactive` signals; each `${…}` hole becomes a `computed`, and a client event pushes a **minimal diff** of only the holes that changed over a websocket. No client build step, no virtual DOM — the reactive graph decides what to patch.

## What it provides

One pure-Noeta module, `para.html`:

- **`render`** — the `@html` expression-tier handler; importing it brings the `@html { … }` tier into scope.
- **`Html`** — a compiled template: the static skeleton, the per-hole `computed`s, the inline event table, and the keyed-list regions.
- **`handle(req, title, page)`** — the request/websocket handler that serves a page and drives the diff-push session.
- **`DomEvent` + `Binding` + `on_click` / `on_input` / `on_submit`** — typed inline event binding at the element (no magic event strings).
- **`keyed(list, key_fn, row_fn)`** — keyed-list regions with minimal structural patches (insert / remove / move) and per-row handlers.

## Installation

```toml
[dependencies]
para = { version = "^0.1", package = "para/html" }
```

The package is keyed `para`, so its module addresses as `para.html`. It is pure Noeta — no `[trust]` entry needed.

## Usage

```noeta
use para.html.{render, Html, handle, on_click}
use std.reactive.signal
use std.http.{Request, Response}

count = signal(0)

fn page(): Html {
    return @html {
        <h1>count: <b>${count.get()}</b></h1>
        <button ${on_click(fn() => count.update(fn(n) => n + 1))}>+1</button>
    }
}

fn fetch(req: Request): Response {
    return handle(req, "Counter", page)
}
```

Run it: `noeta serve counter.noe`.

## Event handling — inline, typed, no magic strings

Handlers bind **inline at the element** with `on_click` / `on_input` / `on_submit`. The `@html` handler registers each closure in the page's dispatch table under an auto id and emits the matching `data-live-*` marker, so a client event routes straight to the closure — there is no hand-written event name and no central `on_event` match to keep in sync.

```noeta
<button ${on_click(fn() => count.update(fn(n) => n + 1))}>+1</button>
<button ${on_click(fn() => count.set(0))}>reset</button>
<input  ${on_input(fn(v) => query.set(v))} placeholder="search">
<form   ${on_submit(fn(body) => submit(body))}> … </form>
```

The closures capture the page's signals directly, so a handler just mutates state and the reactive diff does the rest. Because a handler is server-side (the click travels over the websocket, the server runs it and pushes the diff back), each binding is addressed by an id on the wire — but that id is generated for you, never written by hand.

### `DomEvent` — one source of truth for the event kind

The event *kind* is the typed enum `DomEvent { Click, Input, Submit }`, not a string. It is the single spine of the event path: each variant maps to exactly one `data-live-*` wire marker and one client-side listener. `on_click` / `on_input` / `on_submit` are the typed doors that pair a kind with its correctly-typed handler:

| binder | `DomEvent` | handler signature | payload |
| --- | --- | --- | --- |
| `on_click(fn() => …)` | `Click` | `() -> void` | — |
| `on_input(fn(value) => …)` | `Input` | `(string) -> void` | field value, per keystroke |
| `on_submit(fn(body) => …)` | `Submit` | `(string) -> void` | URL-encoded form body; default prevented |

Each returns a `Binding` carrying its `DomEvent`, so no event kind is ever a bare string anywhere in the path.

### Per-item handlers in lists

A `keyed` row is its own reactive region, and a per-row `on_click` captures **that row's** data. The row's handler is hoisted into the page table under the row's key, so per-item actions route correctly even as the row re-renders:

```noeta
fn row(t: Todo): Html {
    return @html { <li ${on_click(fn() => toggle(t.id))}>${t.title}</li> }
}
// … <ul>${keyed(todos, fn(t) => "${t.id}", row)}</ul>
```

## Examples

- [`examples/liveview-counter/`](examples/liveview-counter) — the counter above.
- [`examples/liveview-todos/`](examples/liveview-todos) — a keyed list with per-row toggles and a "complete all" button.
- [`examples/liveview-events/`](examples/liveview-events) — the event model exercised without a browser (top-level clicks, an input with payload, and keyed per-row handlers), runnable with `noeta run`.
- [`examples/liveview-structural/`](examples/liveview-structural) and [`examples/liveview-structural-app/`](examples/liveview-structural-app) — keyed-region structural patches (insert / remove / move).

The full design write-up is in [`docs/LiveView.md`](docs/LiveView.md).

## Requirements

None beyond the `noeta` toolchain — this package is pure Noeta.

## Development

Each directory under `examples/` is its own small package depending on this repo by path; run `noeta check` / `noeta test` there. See [AGENTS.md](AGENTS.md) for the repo layout and environment details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
