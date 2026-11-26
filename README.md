# customcardmaker

A webapp for designing sets of custom Magic cards — build a set, add cards, and
see each one as it will print. Think MTG Nexus, as an installable PWA.

```sh
./scripts/setup.sh   # hooks, JS deps, browser, warm Rust build
cargo run            # http://localhost:8080
```

See [AGENTS.md](AGENTS.md) for commands and conventions, [SPEC.md](SPEC.md) for
what the app does, and [STORAGE.md](STORAGE.md) for how data is kept.
