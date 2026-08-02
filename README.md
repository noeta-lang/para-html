# para/html

Server-side reactive HTML — LiveView/LiveWire-style reactive templating, in pure Noeta. A page is an `@html { … }` template over `std.reactive` signals; each `${…}` hole becomes a `computed`, and a client event pushes a **minimal diff** of only the holes that changed over a websocket. No client build step, no virtual DOM — the reactive graph decides what to patch.

## What it provides

One pure-Noeta module, `para.html`:

| symbol | kind | purpose |
| --- | --- | --- |
| `render` | `@html` tier handler | importing it brings the `@html { … }` expression tier into scope |
| `Html` | struct | a compiled template: the static `skeleton`, the per-hole computeds (`holes` + `ids`), the inline event table (`handlers`), and the keyed-list `regions` |
| `handle(req, title, render_page)` | fn | the request/websocket handler that serves the page, the client shim, and the diff-push session |
| `DomEvent` | enum | the typed event kinds a template binds: `Click`, `Input`, `Submit` |
| `Binding` | struct | one inline handler paired with its `DomEvent` |
| `on_click` / `on_input` / `on_submit` | fn | typed inline event binders, each returning a `Binding` |
| `keyed(source, key_of, render_row)` | fn | keyed-list regions with minimal structural patches (insert / remove / move) and per-row handlers |
| `keyed_op_stream(prev, next)` | fn | names the minimal structural ops between two key orders — an introspection and testing aid |
| `reconcile_region(reg, v, handlers)` | fn | reconcile one `KeyedRegion` against its current keys; returns a `Reconcile` (the wire frame plus the updated handler table) |

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
use std.http.server
use std.http.{Request, Response}

count = signal(0)

fn page() use (count): Html {
    return @html {
        <h1>count: <b>${count.get()}</b> &nbsp; doubled: <b>${count.get() * 2}</b></h1>
        <button ${on_click(fn() => count.update(fn(n) => n + 1))}>+1</button>
        <button ${on_click(fn() => count.set(0))}>reset</button>
    }
}

fn fetch(req: Request): Response {
    return handle(req, "Counter", page)
}
```

Run it: `noeta serve counter.noe`, open the page, click `+1` — the count **and** its double update in place from one signal change, glitch-free. Both holes compiled to `computed`s that read `count`, so both recompute; the transport pushes only what changed.

## The template language — holes are typed expressions

A hole `${expr}` is an ordinary Noeta expression, checked in scope. What its **value** is decides how it renders:

- **A scalar** (`string` / `int` / `bool` / …) renders as **escaped text** — XSS-safe by default. `${user_input}` can never inject markup.
- **An `Html`** (a nested `@html { … }`) or a **`List<Html>`** (a loop, e.g. `${rows.map(row)}`) embeds as **raw markup**. This is the JSX rule: `{child}` composes, `{text}` is escaped.
- **A hole in attribute position** (`class="${…}"`) is detected from the preceding text and **inlined** (escaped, quote included) rather than wrapped in a marker `<span>` — its value re-renders with the enclosing region (a row), not on its own.
- **A hole in attribute position holding a `Binding`** (an `on_*(…)`) is not rendered as text at all: it registers its closure and emits the event's wire marker (see below).

There is no `v-for` or template-directive syntax — `@html` is lightweight interpolation, not a template compiler, so loops and conditionals are ordinary Noeta expressions (`.map`, `.filter`, `if … then … else`) over `Html` values:

```noeta
fn row(t: Todo): Html {
    return @html { <li class="${if t.done then "done" else "todo"}">${t.title}</li> }
}

fn page() use (todos): Html {
    return @html {
        <h1>Todos — ${remaining()} of ${todos.get().len()} left</h1>
        <ul>${todos.get().map(row)}</ul>
        <p>${if remaining() == 0 then "All done!" else "Keep going."}</p>
    }
}
```

Nested `@html` bodies are verbatim text, **not** strings, so a `${…}` hole inside one may contain double quotes (`${if t.done then "done" else "todo"}`) with none of string interpolation's nested-quote limitation.

### Reactivity — read the signal *inside* the hole

Each hole is wrapped in a `computed`, so it tracks exactly the signals its expression reads **when the hole evaluates** and recomputes only when they change:

```noeta
<h1>${todos.get().len()} items</h1>        // reactive — the hole reads `todos`
```

> [!WARNING]
> Pre-computing a value in the enclosing function breaks reactivity — the read happens outside any hole, so the hole captures a plain value:
>
> ```noeta
> n = todos.get()                            // read happens here, outside any hole
> return @html { <h1>${n.len()} items</h1> } // NOT reactive — the hole captured a value
> ```

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

## Keyed lists — per-row reactivity and structural patches

A plain `${todos.get().map(row)}` loop is **one** reactive region: any change re-renders the whole list into a single `innerHTML`. For a large list where one item changes, that is a lot of wire traffic. `keyed` makes **each row its own persistent reactive region**:

```noeta
<ul>${keyed(todos, fn(t) => "${t.id}", row)}</ul>
```

`keyed(source, key_of, render_row)` takes the **signal itself** (not a snapshot), a stable string key per item, and the row renderer. Each row becomes a persistent `computed` that re-derives *its* item from the signal by key: on any change every row recomputes, but a row whose rendered markup is unchanged produces a value-equal string, so the transport sends nothing for it. Toggling one todo in a 1000-row list pushes exactly one row.

**Structural changes patch in place too.** When the *set* or *order* of keys changes — a row appended, prepended, removed, or reordered — the session reconciles the key sequence after every event and pushes the **minimal** structural ops rather than re-rendering the parent:

- a **new** key renders alone and is inserted at its position, carrying its markup inline in the op;
- a **removed** key's row is torn down alone, and its per-row reactive scope is reclaimed on the spot (`view.unexpose` disposes the row `computed`), so a churning list leaves residency flat;
- a **reordered** key `move`s its existing DOM element — the row's content is *not* re-rendered, so its DOM identity, form state, and focus survive the reorder.

The diff is the standard keyed-children algorithm (Vue 3's `patchKeyedChildren`, the same shape as Solid's and Inferno's reconcile): surviving rows are anchored on their **longest increasing subsequence**, so a reorder moves the *fewest* rows. `keyed_op_stream(prev, next)` names the exact plan, which makes the algorithm directly testable:

```noeta
use para.html.keyed_op_stream

echo keyed_op_stream(["a", "b", "c"], ["a", "b", "c", "d"])   // ins d @end
echo keyed_op_stream(["a", "b", "c", "d"], ["a", "c", "b", "d"])   // mv b d — an adjacent swap is ONE move
```

> [!WARNING]
> A keyed list's keys must be unique — two rows sharing a key would collide on one DOM id, so the per-row diff could never tell them apart. A duplicate key panics loudly (`keyed: duplicate key '…'`) rather than silently corrupting the page.

Use `keyed` for any list a client edits — rows mutating in place *or* rows coming and going; a plain `.map` is fine for small or rarely-changing lists that render whole.

## Serving a page — `handle`, the session, and the wire

`handle(req, title, render_page)` is the whole server surface. An app's `fn fetch(req)` delegates to it, so it runs under `noeta serve` like any `std.http.server` app. It routes three paths:

- **any page path** — the server-rendered document: the template's static skeleton with every hole filled with its initial value (a no-flash first paint), plus a `<script src="/live.js">` tag;
- **`/live.js`** — the bundled client shim: a dependency-free script that connects the websocket, applies patches, and reports events (it reconnects automatically if the socket drops);
- **`/ws`** — the websocket session that drives the live page.

The session renders the page, builds a `view()` over the per-hole computeds, and sends a full `snapshot`. From then on, each client event looks up the element's inline handler by id in the page's dispatch table, runs it with the event payload, reconciles every keyed region (pushing structural frames first, so the content diff never re-pushes a fresh row or a gone one), and finally pushes the minimal `patch` of the holes whose rendered value changed:

| frame | direction | meaning |
| --- | --- | --- |
| `snapshot` | server → client | every exposed hole with its current value, once on connect |
| `patch` | server → client | only the holes whose rendered value changed |
| `keyed` | server → client | structural ops for one region: `ins` / `rm` / `mv`, each addressed by row id |
| `event` | client → server | the bound handler's id plus the payload (empty for a click, the field value for an input, the form body for a submit) |

On the client, the marker attribute alone decides how a value lands: a **text** hole (`data-live`) patches via `textContent` (escaped), a **markup** hole or keyed row (`data-live-html`) via `innerHTML` — so a reactive list of rows updates in place.

A session also wakes on an **idle tick** (`poll_ms()`, 500ms) and diffs anyway, so a change no client event caused still reaches the page — another user's click, a finished job. `handle_every(req, title, page, every_ms, on_tick)` runs your own pull on each wake, for state that has to be *drained* rather than merely observed (a Postgres `NOTIFY` queue via `LiveRepository.pump()`, a p2p log via `sync()`).

> [!NOTE]
> Signal state lives in the worker isolate that handled the connection, so under `noeta serve --parallel N` it is not shared across the fleet. An app whose source of truth is a **database** serves fine on all cores (each worker drains its own notifications); an app whose source of truth is an **in-memory signal** wants a single worker.

## What crosses the wire, and what does not

Worth being precise about, because the framing this model invites is often wrong.

**No application state is ever serialized to the client.** This is not LiveWire's model, where a component's public properties ride in the DOM as a snapshot and round-trip on every request — the reason LiveWire needs `#[Locked]` and hidden properties. Here signals stay on the server and only *rendered output* moves:

- **server → client:** a map of hole id → **rendered string**, plus keyed structural ops (`insert` / `remove` / `move`). The client shim does `textContent = value` or `innerHTML = value` and nothing else. First paint is the same values baked into the skeleton.
- **client → server:** `{type, name, value}` — a handler **id** and a payload string. Empty for a click, the field value for an input, the urlencoded body for a submit.
- **the closures** stay in the session's own table. The client names an id; it never ships code or state.

So a `User` in scope never leaves the process. There is no property to hide, because no property is sent. Two consequences do deserve care:

**A struct in a hole discloses every field.** `${user}` is not markup, so it renders as escaped text — via the value's `to_string` if it has one, and otherwise as `User {id: 1, email: "…", password_hash: "…"}`. Same for `${users}`. Give a type an `impl Display` and `${user}` is safe by construction, once, everywhere.

**Every registered handler is reachable for the session's life.** The table is built from one `render_page()` at socket open, so an id survives the element being hidden, disabled, or scrolled away — and survives a permission being revoked. Rendering is not authorization; `.only_if(...)` is:

```noeta
<button ${on_click(fn() => todos.remove(t.id)).only_if(fn() => user.can_edit(t))}>delete</button>
```

The guard is re-checked on the wake the event arrives on, against current state. A refused event is dropped exactly as an unknown id is — silently, with no reply.

**Two limits apply to every page, unconditionally.** A payload cap (`max_payload_chars`, 256k) and a frame-rate cap (`max_frames_per_second`, 120) are enforced in the session loop itself, with no way to opt out. They are deliberately not something a framework layers on: a limit that only existed for apps which adopted some other package would leave the plain one-file page — the most common way these get written — with no protections at all.

For cross-cutting concerns that need more than this (per-frame identity, per-action rate budgets, tracing), `handle_all` takes an optional interceptor over each `Wake`; [para/aether_html](https://github.com/noeta-lang/para-aether-html) is the onion built on it.

## Testing without a browser

An `Html` value is a plain, inspectable struct — the skeleton is a string and the dispatch table is a `Map` — so the whole event model exercises under `noeta run` / `noeta test` with no browser and no socket. Simulating a client event is exactly what the websocket session does: look up the closure by id, run it with the payload.

```noeta
use para.html.{render, Html, on_click}
use std.reactive.signal

count = signal(0)

fn page() use (count): Html {
    return @html { <button ${on_click(fn() => count.update(fn(n) => n + 1))}>+1</button> }
}

fn main() use (count): void {
    p = page()
    echo p.skeleton.contains("data-live-click=\"e0\"")   // true — the binding's wire marker
    p.handlers.get_or("e0", fn(_: string) {})("")        // fire the click
    echo count.get()                                     // 1
}

main()
```

Binding ids follow hole order (`e0`, `e1`, …); a keyed row's handlers are namespaced by its key (the row for id `1` registers its first click as `1-e0`). One level up, the structural side tests the same way: build a `view()` over `p.holes` / `p.ids`, mutate the source signal, and call `reconcile_region(p.regions[0], v, p.handlers)` to assert the exact wire frame — [`examples/liveview-structural/`](examples/liveview-structural) does both, including asserting that a reorder leaves row content untouched.

> [!TIP]
> `noeta test` never runs top-level statements, so a file that ends in `server.serve(…)` or `main()` is safe to `noeta check` / `noeta test` — the browser examples rely on exactly this.

## Examples

- [`examples/liveview-counter/`](examples/liveview-counter) — the counter above.
- [`examples/liveview-todos/`](examples/liveview-todos) — a keyed list with per-row toggles and a "complete all" button.
- [`examples/liveview-events/`](examples/liveview-events) — the event model exercised without a browser (top-level clicks, an input with payload, and keyed per-row handlers), runnable with `noeta run`.
- [`examples/liveview-structural/`](examples/liveview-structural) and [`examples/liveview-structural-app/`](examples/liveview-structural-app) — keyed-region structural patches (insert / remove / move), tested headless and served live.

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
