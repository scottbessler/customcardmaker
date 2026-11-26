# Working in this repo

Rust (axum) backend in `src/`, vanilla-JS PWA frontend in `public/`, Playwright
e2e in `tests/e2e/`. `SPEC.md` is the source of truth for what the app does;
`STORAGE.md` says how data is kept. This repo deliberately follows the same stack
and principles as [two-seven](https://github.com/scottbessler/two-seven): when
in doubt about how something should be done, look at how it is done there.

## Setup

```sh
./scripts/setup.sh
```

Idempotent: enables the repo git hooks, installs JS deps, resolves a Chromium
for Playwright, and warms the Rust build. `SKIP_BUILD=1` skips the slow build,
`SKIP_BROWSERS=1` skips browser setup. Claude Code web sessions run this
automatically via `.claude/hooks/session-start.sh`.

Toolchain: Rust 1.90+ (edition 2024), bun 1.3.13, node 22 — see `.mise.toml`.

## Commands

| Task | Command |
| --- | --- |
| Rust tests | `cargo test --locked` |
| Rust lint | `cargo fmt --check && cargo clippy --locked --all-targets --all-features` |
| JS lint | `bun run lint` (`bun run lint:fix` to autofix) |
| e2e | `bun run test:e2e` (fast local loop; skips image comparison — see Notes) |
| Run server | `cargo run` (:8080), or `./dev.sh` to restart on file change |
| Everything CI runs | `mise run check` |

## Principles

- **No build step.** The frontend is native ES modules served from `public/`,
  with Preact + htm vendored in `public/vendor/`. Pages are a server-rendered
  shell (`src/render.rs`) plus one island per page. Shared modules are listed
  in the import map in `render.rs` so a release versions them too.
- **One source for each rule.** Card validation lives in `src/card.rs`; what a
  mana cost *means* (colours, frame) lives in `public/frame.js`, pure and
  DOM-free, so the editor and every other view agree.
- **Documents stay documents.** A card is one JSON blob in one row; columns exist
  only for what is ordered or joined on. Every `Card` field is `#[serde(default)]`
  so the shape can grow without a migration.
- **Schema changes are appended to `MIGRATIONS` in `src/db.rs`.** A migration
  that has shipped is never edited.
- **Tokens, not literals.** Every colour is defined in `public/css/01-tokens.css`;
  `tests/frontend_assets.rs` fails on a raw colour anywhere else. `04-cards.css`
  is the one file exempt from the UI type scale: card faces size everything in
  `cqw`, a fraction of the card's own width.
- **Prefer a geometry snapshot to an image one.** `expectLayout` in
  `tests/e2e/layout.ts` records boxes and computed styles as JSON that diffs in
  review; images are for what only pixels catch.

## Notes

- `warnings = "deny"` and `clippy::all = "deny"` are set in `Cargo.toml`; a
  warning fails the build, so fix rather than `#[allow]`.
- Accounts are passkey-only (`src/auth.rs`, ported from two-seven). Passkeys
  can't be driven headlessly, so `PASSKEY_DISABLED=1` makes registering and
  signing in work by username alone; `dev.sh`, the Playwright web server and the
  Claude session-start hook all set it. **Never set it in production.** In
  production set `SESSION_SECRET` (64+ bytes) so sessions survive a restart, and
  `RP_ID` / `RP_ORIGIN` to the site's host and origin or passkeys will refuse to
  register.
- Every set belongs to one account. Store methods take the owner and filter on
  it, so someone else's set answers exactly like one that does not exist (404);
  keep it that way for any new query.
- e2e tests sign in with `registerUser` from `tests/e2e/session.ts`; both
  projects share one server and database, so every name must be unique.
- e2e snapshots live beside their specs: geometry baselines under
  `tests/e2e/*-snapshots/json/`, image baselines under
  `tests/e2e/*-snapshots/images/`. **`bun run test:e2e` skips image comparison**
  unless `CI` or `E2E_IMAGES` is set, because a comparison against another
  host's fonts reports failures that mean nothing. On Linux
  `E2E_IMAGES=1 bun run test:e2e` opts in; macOS will not match (CoreText vs
  FreeType) and needs the container or CI.
- Every same-repository pull request regenerates image baselines in the pinned
  Playwright image and commits image-only changes back to its branch
  (`.github/workflows/update-snapshots.yml`); JSON geometry changes stay
  explicit and reviewable. Never run `--update-snapshots` over a spec you are
  not regenerating: it rewrites correct baselines with this host's numbers.
- The `chromium-mobile` project emulates an iPhone 15/16 Pro, safe-area insets
  included — don't relax the geometry or image tolerance to make a snapshot pass.
- The UI font is Bitter, vendored as variable woff2 subsets in
  `public/vendor/bitter-v42-*.woff2`. The version lives in the filename because
  the `@font-face` src sits in static CSS that `asset()` never rewrites — to
  update the face, drop in new files under a new version and change
  `01-tokens.css` and the preload in `src/render.rs` together.
