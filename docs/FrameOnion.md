# The per-frame onion — design note

**Status:** shipped — para/html 0.3.0 (the seam) and 0.4.0 (prefix mounting), para/aether_html 0.1.0. Spans three packages: `para/html` (the seam), `para/aether` (unchanged), and a new companion `para/aether_live` (the onion).

## The problem

para/html owns a dispatch loop. `session` holds a table of generated ids mapped to closures and invokes one on every websocket frame, with the id and the payload both chosen by the client:

```noeta
if handlers.has(name) {
    handler = handlers.get_or(name, fn(_: string) {})
    handler(value)
}
```

Nothing between the frame and the closure asks whether the frame should run. Today the only gate is *render time* — an app writes `if user.can_edit(t) then @html { <button ${on_click(del)}> }` and treats the button's absence as the boundary. It isn't. The table is built from one `render_page()` at socket open and lives for the session's lifetime, so anything ever registered stays reachable: hidden, `disabled`, scrolled off, or authorized under a permission since revoked.

The natural place to put that check is aether's middleware onion, whose documented purpose already includes answering without dispatching. It structurally cannot reach:

```
GET /ws ──▶ [ onion: mw₁ → mw₂ → serve_request ] ──▶ Response(hijack)   ← runs ONCE
                                                          │
                                                          └─▶ session loop
                                                                frame … frame … frame …   ← hours
                                                                (onion has already unwound)
```

`server.websocket(handler)` is a connection-hijack response: the whole session runs *inside* one `Request → Response` cycle. aether's `Middleware.handle(req, next): Response` runs per request, so it fires once at upgrade and never again. Authorizing the upgrade and nothing after is exactly today's hole.

## The shape

Generalize the onion from *one layer per HTTP request* to *one layer per unit of client-originated work*, where a websocket wake is such a unit — and put that generalization in a **third package**, so neither existing package grows a dependency on the other.

This mirrors `para/aether_db`, whose own README states the rule this arc reuses: *"a database package must not depend on the web framework,"* and folding it into aether-core *"would make para/db's native footprint mandatory for every aether app."* Both halves map:

- **para/html must not depend on aether.** It is std-only (`std.reactive`, `std.http`, `std.json`) and standalone-servable. A dependency on the web framework would cost both.
- **aether must not depend on para/html.** A REST-only app should not pull in the LiveView engine, its tier handler, or its reconciler.

So:

| package | gains | depends on |
| --- | --- | --- |
| `para/html` | one neutral seam + unconditional hard limits | std only (unchanged) |
| `para/aether` | nothing | unchanged |
| `para/aether_live` (new) | `Frame`, `FrameMiddleware`, `FrameNext`, `mount_live`, the identity/policy layers | `para/aether` + `para/html` |

An app opts in by adding the third package to the `para` scope it already lists the other two under — the same array-form scope resolution `para/aether_db` uses.

## What para/html gains

One seam, phrased in nothing but its own vocabulary. It never names a user, a session, a permission, or aether.

```noeta
// One wake of a live session: a client frame, or an idle tick.
pub struct Wake {
    // The handler id the client named and the payload it sent. Both empty on an idle tick.
    name: string
    value: string
    // True when no client frame arrived — the session woke on its own timer.
    tick: bool
    // The request that opened the socket. Carried so an interceptor can recover whatever the
    // upgrade carried (a cookie, a header); para/html never reads it.
    origin: Request
}
```

`handle` grows an optional `intercept:` argument, `(Wake, () -> void) -> void`. The second argument is the rest of the work — dispatch, reconcile, diff — as a thunk, exactly as `Next.run` is the rest of the HTTP pipeline. Not calling it drops the wake, and the page simply does not change.

**Decision — the seam wraps the whole wake, not just the dispatch.** The thunk covers the handler call *and* `reconcile_region` *and* `v.diff()`. Preceding the dispatch would be a hook; wrapping the wake is an onion, and it is the only version where a layer can measure what an event actually cost (a click that pushed 40 KB) or short-circuit the real work.

**Decision — a tick is a wake.** `on_tick` runs app code on a wake with no client behind it. Routing it through the seam means tracing and metrics cover it for free; `tick: true` lets an authorization layer no-op on it, since there is no client to authorize.

**Decision — hard limits are unconditional, in para/html.** This is the one that shapes the arc. If payload caps and frame-rate caps were onion layers, they would exist only on the mounted path, and the standalone one-file page — the simplest and most common way these get written — would have none of them. So `session` caps payload size and frame rate itself, always, with no interceptor in sight. The onion carries *app-specific policy*; para/html carries *the limits no app should have to ask for*.

Standalone `handle` keeps working with no interceptor. The 32-line `counter.noe` does not change.

## What para/aether_live gains

The onion, in aether's vocabulary:

```noeta
pub trait FrameMiddleware {
    fn handle(f: Frame, next: FrameNext): void
}
```

Note `void`, not `Response` — the reason this is a sibling trait rather than `Middleware` reused. A frame produces *effects* (signal writes); the reply is whatever the subsequent diff pushes. A layer that wants to say something to the page writes to a signal and lets the reactive graph carry it, which keeps the whole thing inside the existing model instead of inventing a second reply path.

`FrameNext` is the same cursor shape as aether's `Next` — a shared stack plus an advancing index, bottoming out in the thunk para/html handed over.

`mount_live(app, path, title, page)` registers the page route, the `/ws` upgrade, and the shared shim, and wires the onion as the interceptor.

**Resolved — shim-route ownership.** Each mount serves its own copy of the shim at `${base}/live.js` rather than sharing one app-wide. The copies are identical and browser-cached per URL, so the duplication costs nothing measurable; sharing one would have split ownership of a para/html route between this package and whatever mounted it, which is precisely how two modes drift. para/html owns its three routes in both modes, relative to a base, and `serves(base, path)` is the single predicate both its own routing and a host framework's mount gate read.

## What the onion cannot do, and why the binding guard still exists

A central layer sees `c1-e0` — a generated id with no semantics. It cannot authorize on that without a parallel map, which is exactly the magic-string dispatch the inline-binding design deliberately removed. So:

- **Cross-cutting, semantics-free concerns** — rate limiting, tracing, metrics, identity resolution — belong in the onion.
- **Per-action policy** — "may this identity delete *this* todo?" — belongs at the call site, as a guard on the `Binding`, re-evaluated at event time.

They are complementary. The guard closes the live hole in one file; the onion is the architecture that makes LiveView a first-class aether citizen.

## Slices

- **S1** — para/html: unconditional payload + frame-rate caps in `session`.
- **S2** — para/html: `Binding.guard`, the `Map<string, Binding>` handler table, guarded dispatch.
- **S3** — para/html: the `Wake` seam on `handle`, standalone path unchanged.
- **S4** — new package `para/aether_live`: `Frame`, `FrameMiddleware`, `FrameNext`, `mount_live`.
- **S5** — para/aether_live: the layers — per-frame identity via `SessionStore`, authorize, rate limit.
- **S6** — the drift gate: a differential test driving the same page standalone and mounted, asserting identical wire output.
- **S7** — docs: the "what crosses the wire" section both READMEs currently lack.

## Non-goals

- Client-side state. The wire carries rendered strings out and an id plus a payload in; nothing about that changes.
- Replacing render-time conditionals. Not rendering a button is still right for UX; it is simply not a security boundary.
- Making aether depend on para/html, or para/html depend on aether. Neither, ever — that is what the third package is for.
