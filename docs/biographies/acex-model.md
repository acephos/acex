# Biography: acex-model

| | |
|--|--|
| **Path** | `crates/acex-model/` |
| **Purpose** | Pure session store, event reduce, filters, waits, `Intent` surface. |
| **Origin** | Phase 0 store; Phase 1 intents and board helpers. |
| **Status** | No I/O; unit-tested hot path for apply_event / apply_snapshot. |
| **How to change** | Keep pure; new Intent variants stay owned data; tests call shipped methods. |
