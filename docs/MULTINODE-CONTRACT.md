# AIDA Multi-Node Synchronization Behavioral Contract

This document defines the canonical invariants and user-experience contract for AIDA multi-node synchronization across distributed Git clones sharing an `aida-store` orphan branch.

---

## 1. Repository Bootstrap & Clone Attachment

### 1.1 Primary Seed / Root Clone
1. `aida init` initializes `.aida-store` (orphan branch `aida-store`) and commits AIDA scaffolding on `main`.
2. MCP server registration (`codex mcp add`, Antigravity MCP config) **must** only run when `--with-mcp` is explicitly passed (`BUG-MN-007`).
3. Pushing both `main` and `aida-store` to a bare hub (`git push -u origin main && git -C .aida-store push -u origin aida-store`) publishes the canonical store.

### 1.2 Secondary Clones (Auto-Attachment & Identity)
1. **Never run `aida init` on a clone of an already-initialized AIDA repo:** Doing so creates a disjoint orphan `aida-store` history.
2. **Automatic store worktree attachment:** Running any read or status command (`aida db status`, `aida list`, `aida node acquire`) in a fresh clone that has `origin/aida-store` automatically attaches `.aida-store/` to `origin/aida-store`.
3. **Node identity acquisition (`aida node acquire`):**
   - By default on a non-authoritative clone, `aida node acquire` bootstraps a 32-hex UUID work-node binding at `.git/aida/work-binding.toml`.
   - Operators can bind a short, human-friendly node alias (`aida node alias set <LABEL>`) so 41-char node-local IDs (`STORY-<32-hex>-001`) render and accept short aliases (`STORY-NODEA-001`) in 80-column tables.

---

## 2. Author & Actor Provenance (`BUG-MN-006`)

All CLI write paths (`aida add`, `aida edit`, `aida comment add`, `aida queue`, `aida mailbox`, and fallback `write_event` history entries) must resolve the default author using a single consistent precedence chain:

1. `AIDA_AUTHOR` (explicit harness/agent override, e.g. `ai:codex:joe`)
2. `AIDA_USER` (multi-node / per-seat operator identity documented in `docs/advanced/multi-node.md`)
3. `USER` (POSIX login name)
4. `USERNAME` (Windows fallback)
5. `"Unknown"`

---

## 3. Online vs. Offline `aida add` & ID Allocation (S1 & S5)

1. **Collision-free node-local IDs:** Each node allocates monotonic sequence numbers within its own node namespace (`<TYPE>-<NODE_ID>-<SEQ>`), guaranteeing zero ID collisions when multiple clones file specs concurrently or offline.
2. **Online auto-publication:** When `origin` is reachable (and `AIDA_ADD_NO_REMOTE_SYNC` is not set), `aida add` immediately pushes the new spec commit and `oplog.yaml` entry to `origin/aida-store` (`publish_new_spec`).
3. **Offline graceful fallback:** When `origin` is unreachable, `aida add` succeeds locally, records the spec in `.aida-store`, and informs the user that publication is deferred until the next online `aida add` or `aida db sync --push`.

---

## 4. Field-Level Edit History (`BUG-MN-004`)

Every `aida edit` invocation that mutates one or more fields on a requirement **must** append a `HistoryEntry` with explicit `FieldChange { field_name, old_value, new_value }` items for every modified field (`title`, `description`, `status`, `priority`, `type`, `owner`, `feature`, `tags`, `weight`, `verify`).
- Fallback write events (`changes: []`) are reserved strictly for programmatic mutations that have no field-change builder (`STORY-1674`).

---

## 5. Store Rebase & 3-Way Auto-Merge Invariants (S3 & S4)

### 5.1 Hermetic Git Rebase Execution (`BUG-MN-001`)
- Internal Git plumbing commands (`git rebase --continue`, `git commit --no-edit`) must explicitly set `GIT_EDITOR=true` in the subprocess environment so caller environments that export `GIT_EDITOR=false` (such as non-interactive agent harnesses) never abort an automated store rebase.

### 5.2 Disjoint Spec Edits & Fast-Forward Pulls (S3, `BUG-MN-003`)
- Concurrent edits to **different** specs (`objects/*/*.yaml`) and concurrent appends to `oplog.yaml` auto-merge cleanly via `rebase_auto_merge_onto`.
- A clean fast-forward pull (where local `.aida-store` had no unpushed commits on a spec) **must never** trigger a `Conflicts detected:` warning simply because `remote.modified_at > local.modified_at`.
- Auto-merge status lines must print a single `auto-merged <target>: ...` prefix, never `auto-merged auto-merged ...`.

### 5.3 Same-Spec Concurrent Edits (S4a & S4b, `BUG-MN-002`)
When two nodes concurrently edit the **same** spec against common ancestor `base`:
1. **Append-only collections (`history`, `comments`, `processing_record`):** Unioned by UUID `id` and sorted deterministically by `(timestamp, id)`.
2. **Removable sets (`tags`, `relationships`, `dependencies`):** Merged via 3-way set merge against `base` (`(ours ∪ theirs) − ((base − ours) ∪ (base − theirs))`), so single-side additions and single-side removals both survive.
3. **Scalar fields (`title`, `description`, `status`, `priority`, `owner`, `feature`, `weight`, `execution_mode`, `origin`, `verify`):**
   - **Single-side change (`ours != base && theirs == base`, or vice versa):** The side that changed the field relative to `base` **always wins**, even under `merge_spec_upstream`. Editing `title` on Node A and `description` on Node B preserves both edits.
   - **True same-field collision (`ours != base && theirs != base && ours != theirs`):**
     - Under `merge_spec_upstream` (rebase onto `origin/aida-store`), `upstream` wins the colliding scalar field (`BUG-2501`), **and** the merge returns a `ScalarOverrideReport` listing every colliding field where `local`'s edit was overridden by `upstream`.
     - Both `aida db sync --push` (when push-retry rebases) and `aida db sync --pull` / `aida pull` must surface any non-empty `ScalarOverrideReport` so no scalar edit is ever silently lost.

---

## 6. SQLite Cache Freshness Across Sync Commands (S5 & S6, `BUG-MN-005`)

Whenever `.aida-store` `HEAD` advances during `aida pull`, `aida db sync --pull`, or `aida db sync --push` (via push-retry rebase), AIDA must synchronously refresh `.aida/cache-db/cache.db` (if present) before the sync command exits.
- Subsequent `aida list`, `aida show`, `aida cache status`, and `aida db status` calls must immediately observe `stale: false` and include all newly synced specs on the very first read.

---

## 7. Multi-Node Mailbox Addressing (S7, `BUG-MN-007`)

- `aida mailbox send --to <user>` must either resolve `<user>` from known store activity / work-node bindings or, if refusing an unknown handle, provide the exact command to register the user in the store (`aida node team set-role --role <role> <user>`) alongside `--broadcast`.
