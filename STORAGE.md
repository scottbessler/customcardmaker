# Storage

Everything lives in one SQLite file, `DATA_PATH/customcardmaker.db` (default
`data/`), opened by `src/db.rs`. This is the arrangement two-seven arrived at
(see its `STORAGE.md`), adopted from the start here rather than migrated to.

- **One connection, on the blocking pool, behind a mutex.** A single writer; WAL;
  `synchronous = NORMAL`; `busy_timeout`. Readers queue behind it, which costs
  nothing at this size. A read pool is the answer when that stops being true.
- **Migrations are an append-only list** in `src/db.rs`; `PRAGMA user_version`
  counts how many have been applied. A migration that has shipped is never edited.
- **Documents stay documents.** A card is one JSON blob; `set_id` and `position`
  are columns because they are ordered and joined on.
- **Durability past the machine** is Litestream shipping the WAL to object
  storage behind the process. Not wired up yet — it belongs with deployment.

## Tables

| Table | Holds |
| --- | --- |
| `users` | username (unique, case-insensitive), display name, passkeys as one JSON document |
| `sets` | name, code, `owner_id`, timestamps |
| `art` | one row per uploaded image: owner, real content type, size |
| `cards` | one JSON document per card, with `set_id` and collector `position` |

## Files

Uploaded art is **not** in the database. The bytes are files under
`DATA_PATH/art/<id>`, beside `customcardmaker.db` on the same volume, and the
`art` row says whose each one is. A few megabytes read through the single
connection would stall every other request, which is the one thing the
connection arrangement above cannot absorb.

The file is written before its row, so a crash leaves a stray file, never a row
with nothing behind it. Nothing deletes art yet: removing it from a card only
drops the reference, so the file stays. **These files are not replicated.** The Litestream plan above covers the
database only; until the files are backed up too, a lost volume loses every image.

Deleting an account cascades to its sets, and a set to its cards. Deleting a card renumbers the ones after
it, so collector numbers are always `1..=n`.
