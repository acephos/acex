# Artifacts

Durable outputs referenced by `docs/tracker.html` and biographies.

| Artifact | Location | Biography |
|----------|----------|-----------|
| Herdr API schema | `crates/herdr-types/schemas/herdr-api.schema.json` | [biographies/schema.md](../biographies/schema.md) |
| Living tracker | `docs/tracker.html` | [biographies/tracker.md](../biographies/tracker.md) |
| Architecture | `docs/ARCHITECTURE.md` | self |
| Extension guide | `docs/EXTENDING.md` | self |
| Verify gates | `docs/VERIFY.md` | self |
| Agent skill | `skills/acex-dev/SKILL.md` | [biographies/skill.md](../biographies/skill.md) |

Refresh schema:

```bash
herdr api schema --json > crates/herdr-types/schemas/herdr-api.schema.json
```

Then note protocol version + date in the tracker Artifacts / Changelog sections.
