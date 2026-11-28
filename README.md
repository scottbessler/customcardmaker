# customcardmaker

A webapp for designing sets of custom Magic cards — build a set, add cards, and
see each one as it will print. Think MTG Nexus, as an installable PWA.

```sh
./scripts/setup.sh   # hooks, JS deps, browser, warm Rust build
cargo run            # http://localhost:8080
```

See [AGENTS.md](AGENTS.md) for commands and conventions, [SPEC.md](SPEC.md) for
what the app does, and [STORAGE.md](STORAGE.md) for how data is kept.

## AI tools

Set `OPENROUTER_API_KEY` in the server environment to enable rules cleanup and
art background extension. Restart the server after changing the environment.
The defaults are `openai/gpt-5-mini` for text and
`google/gemini-2.5-flash-image` for image edits. Override them with
`OPENROUTER_TEXT_MODEL` and `OPENROUTER_IMAGE_MODEL`.
