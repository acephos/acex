# Architecture — acex

> Linked from [AGENTS.md](../AGENTS.md). Ownership rules are non-negotiable (see [SOUL.md](../SOUL.md)).

## Biography

| | |
|--|--|
| **Purpose** | Name crate boundaries, data flow, and extension seams so agents change the right layer. |
| **Origin** | Scaffolded with Phase 0 cargo workspace; refined through F01–F14 control-plane work. |
| **Status** | Production-oriented control plane: unary RPC + long-lived subscribe + intent worker. |
| **Change** | Edit this file when ownership or data-flow changes; note decision in `docs/tracker.html`. |

---

## Crate map

```
acex (bin)           compose: bootstrap, live loop, intent worker, CLI flags
  ├── acex-ui        ratatui ONLY — keys, palette, board, modals
  ├── acex-model     pure store + reducers + Intent enum (no I/O)
  ├── acex-config    env/defaults (socket, editor, peek lines)
  ├── acex-editor    EditorBridge trait + Zed CLI adapter
  ├── herdr-client   Transport, NDJSON, subscribe, unary ops, spawn
  └── herdr-types    wire models, AgentState, SessionSnapshot (no I/O)
```

### Ownership invariants

| Concern | Owner crate | Must not |
|---------|-------------|----------|
| NDJSON / socket / pipes | `herdr-client` | UI or model open sockets |
| Drawing / keys | `acex-ui` | model import ratatui |
| Session reduce | `acex-model` | block on network |
| Spawn editor | `acex-editor` | hardcode paths in UI |
| Protocol shapes | `herdr-types` | depend on tokio/UI |

---

## Runtime data flow

```
bootstrap: ping → session.snapshot → Store::apply_snapshot
           optional agent.list merge

live:      events.subscribe (long-lived pipe/UDS)
           → SubscriptionPush → Event → Store::apply_event

unary:     UI Intent → mpsc → worker → HerdrClient::request (connect-per-call)
           → Store patch (toast / peek / agents)

resync:    stream drop → resync_with_backoff → apply_resnapshot → resubscribe
```

### Transport rules

- **Unary RPC:** connect → one request/response → disconnect (Windows named pipes close after response).
- **Subscribe:** dedicated long-lived connection; push lines `{event, data}` with underscore event names.
- **Windows pipe path:** `\\.\pipe\{absolute path to herdr.sock}` (sock file is a pid:token marker).

---

## Hot paths (performance)

1. **`Store::apply_event`** — pure, no alloc storms; O(agents) for status update. Unit-test without I/O.
2. **NDJSON line framing** — byte-at-a-time with 64 MiB guard; keep on IO edge.
3. **Shared store** — `Arc<Mutex<Store>>` between UI thread and async tasks; hold locks briefly (patch then release).
4. **Intent worker** — never block UI; all Herdr I/O async on tokio runtime.

---

## Extension seams (summary)

Full steps: [EXTENDING.md](./EXTENDING.md).

| Seam | Type | Location |
|------|------|----------|
| Operator action | `Intent` + palette + worker arm | `acex-model/intent.rs`, `acex-ui/palette.rs`, `acex/worker.rs` |
| Transport | `Transport` trait | `herdr-client/transport.rs` |
| Editor | `EditorBridge` trait | `acex-editor` |
| Protocol types | serde models + schema artifact | `herdr-types`, `schemas/herdr-api.schema.json` |

---

## Related

- [EXTENDING.md](./EXTENDING.md) — how to add a capability  
- [VERIFY.md](./VERIFY.md) — production hygiene commands  
- [biographies/INDEX.md](./biographies/INDEX.md) — artifact lineage  
- [tracker.html](./tracker.html) — living status  
