# `.acex/packages/` — drop-in capability packages

Place a directory here with `acex-package.toml` to be discovered at startup:

```
.acex/packages/my-pack/
  acex-package.toml
  README.md          # optional progressive detail for agents
```

See [docs/PHILOSOPHY_PI.md](../../docs/PHILOSOPHY_PI.md) and [docs/EXTENDING.md](../../docs/EXTENDING.md).

Discovery is **manifest-only** (no runtime script loading). New Intent behavior still requires a code change per EXTENDING.md.
