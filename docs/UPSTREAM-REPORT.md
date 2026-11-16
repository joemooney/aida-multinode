# Upstream Engineering Report: Multi-Node Store Synchronization & Multi-Hub Forge Fixes

**Date:** 2026-11-15  
**Target Upstream:** `joemooney/aida` (`main` @ `6f0d951dd9b`)  
**Fix Worktree / Branch:** `/home/joe/ai/aida-multinode-fixes` (`fix/multinode-sync-bugs`)  
**Companion Test & Patch Repo:**
- GitHub: `https://github.com/joemooney/aida-multinode.git`
- GitLab: `ssh://git@gitlab.joemooney.com:2222/ai/aida-multinode.git`
- Local checkout: [aida-multinode](file:///home/joe/ai/aida-multinode)

---

## 1. Executive Summary

We executed an end-to-end multi-node synchronization audit of AIDA's distributed `aida-store` orphan branch across isolated clones (`Node A`, `Node B`, `Node C`) and multi-forge hubs (`origin` + `gitlab` mirror).

During baseline testing against `aida 0.16.0`, **5 of 12 automated scenarios failed or warned** under `--strict` verification, and live GitLab mirror onboarding (`S12`) uncovered an additional error-masking bug in `gitlab_project_create.rs` (**8 bugs total: `BUG-MN-001` through `BUG-MN-008`**).

We created an isolated git worktree from `aida` (`/home/joe/ai/aida-multinode-fixes`, branch `fix/multinode-sync-bugs`), implemented root-cause fixes across `aida-core` and `aida-cli-lib`, and verified that:
1. All **12 automated multi-node and multi-hub scenarios (`Setup` + `S1`–`S11`) pass (`12/12 PASS`, `0 FAIL`, `0 WARN`)** under `harness/run_multinode_suite.sh --strict`.
2. All **199 unit tests** across the modified `aida-core` and `aida-cli-lib` modules (`conflict`, `provenance`, `mailbox_identity`, `bug_2501_pull_tests`, `gitlab_project_create`) pass with zero regressions.
3. The fixes are packaged as a clean, 6-commit `git format-patch` series in [patches/](file:///home/joe/ai/aida-multinode/patches) ready for `git am` or cherry-picking into upstream `aida`.

---

## 2. Before vs. After Verification Matrix

Command executed:
```bash
/home/joe/ai/aida-multinode/harness/run_multinode_suite.sh \
  --aida-bin /home/joe/ai/aida-multinode-fixes/target/debug/aida \
  --strict
```

| Scenario | Description | Baseline (`0.16.0`) | Patched (`fix/multinode-sync-bugs`) | Latency | Bug(s) Exercised |
|---|---|---|---|---|---|
| **Setup** | Bare hub + seed `aida init` + Node A/B clone auto-attach + lease acquire + 3 starter specs | `PASS` | **`PASS`** | 28.0 s | `BUG-MN-007` (init MCP gating) |
| **S1** | Add `FR-NODEA-001` on Node A, push to hub, pull on Node B, verify visibility | `PASS` | **`PASS`** | 7.9 s | — |
| **S2** | Add comment on Node B with `AIDA_USER=node-b`, push/pull to Node A, check author | `WARN` | **`PASS`** | 4.5 s | `BUG-MN-006` |
| **S3** | Offline disjoint edits on A & B, push A, push-rebase B under `GIT_EDITOR=false`, FF pull on A | `FAIL` | **`PASS`** | 10.2 s | `BUG-MN-001`, `BUG-MN-003` |
| **S4a** | Concurrent same-spec edits (`FR-NODEA-001`) on A & B; verify 3-way scalar merge & override notice on `--push` | `FAIL` | **`PASS`** | 7.0 s | `BUG-MN-002`, `BUG-MN-004` |
| **S5** | Offline allocation of 5 specs on A + 5 specs on B, reconnect hub, push/rebase/pull with 0 ID collisions | `PASS` | **`PASS`** | 23.2 s | — |
| **S6** | Verify `.aida/cache-db/cache.db` is immediately fresh after `aida db sync --pull` | `WARN` | **`PASS`** | 3.7 s | `BUG-MN-005` |
| **S7** | Direct `aida mailbox send --to node-b` on work-node clone without `team.toml` + publish on `db sync --push` | `WARN` | **`PASS`** | 2.8 s | `BUG-MN-007` |
| **S8** | Configure `gitlab` mirror (`aida remote mirror gitlab --url ...`); verify `db sync --push` fans out `aida-store` | `PASS` | **`PASS`** | 3.3 s | `STORY-MN-003` |
| **S9** | Out-of-band push to `origin` only; verify `aida remote status` exits non-zero and `remote mirror-sync` levels `gitlab` | `PASS` | **`PASS`** | 7.7 s | `STORY-MN-003` |
| **S10** | Node C sets `aida remote push-hubs gitlab`; verify pushes go exclusively to `gitlab` while `origin` diverges | `PASS` | **`PASS`** | 6.9 s | `STORY-MN-003` |
| **S11** | Verify `aida remote reconcile` exits `2` on diverged hubs and `remote reconcile --execute` union-merges both hubs | `PASS` | **`PASS`** | 2.5 s | `STORY-MN-003` |
| **Total** | **`--strict` exit status** | **2 FAIL, 3 WARN (exit 1)** | **12/12 PASS (exit 0)** | **107.7 s** | All resolved |

---

## 3. Root-Cause Analysis & Fixes (`BUG-MN-001` – `BUG-MN-008`)

### `BUG-MN-001` (P0): `GIT_EDITOR=false` in caller environment breaks `git rebase --continue` during store auto-merge
- **Symptom (S3):** When an agent harness, CI runner, or non-interactive shell exports `GIT_EDITOR=false` (to prevent interactive editor hangs), AIDA's automated conflict resolver resolves `oplog.yaml` and `objects/*.yaml`, stages them, and runs `git rebase --continue`. Because `git rebase --continue` spawns `$GIT_EDITOR` to confirm the commit message unless overridden, `GIT_EDITOR=false` causes `git rebase --continue` to fail with:
  ```text
  hint: Waiting for your editor to close the file... false:error: There was a problem with the editor 'false'.
  ```
  leaving `.aida-store` wedged mid-rebase (`.git/rebase-merge`).
- **Root Cause:** [git_ops.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/git_ops.rs#L2252-L2332) (`pull_rebase_auto_merge` and `rebase_onto_auto_merge`) invoked `git(repo, &["rebase", "--continue"])` without overriding `core.editor` or `GIT_EDITOR`.
- **Fix:** Replaced both `git(repo, &["rebase", "--continue"])` calls with:
  ```rust
  git_with_env(
      repo,
      &["-c", "core.editor=true", "rebase", "--continue"],
      &[("GIT_EDITOR", "true"), ("GIT_SEQUENCE_EDITOR", "true")],
  )
  ```
- **Patch:** [0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch](file:///home/joe/ai/aida-multinode/patches/0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch)

---

### `BUG-MN-002` (P1): `merge_spec_upstream` discards non-colliding local scalar edits and hides colliding scalar overrides on `aida db sync --push`
- **Symptom (S4a):** When Node A and Node B concurrently edit the same spec (`FR-NODEA-001`) and Node B runs `aida db sync --push`:
  1. Push is rejected by `origin`, triggering `pull_rebase_store_retry` -> `resolve_spec_conflict` -> `merge_spec_upstream(&base, &upstream, &local)`.
  2. If Node A edited only `title` and Node B edited only `description` (or `priority`), Node B's non-colliding local scalar edit was **silently wiped out** because `merge_spec_upstream` cloned `upstream` and only called `merge_status`.
  3. If both nodes edited `title`, upstream overwrote local `title` with zero warning (`Push complete after rebase.`).
- **Root Cause:**
  1. [conflict.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/conflict.rs#L425-L560): `merge_spec_upstream` started with `let mut out = upstream.clone();` and only 3-way merged `status`, `agreed_id`, `tags`, `comments`, `history`, `relationships`, and `urls`, ignoring `title`, `description`, `priority`, `owner`, `feature`, `req_type`, `implementation_info`, `custom_fields`, `archived`, `weight`, and ` prefix_override`.
  2. [git_ops.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/git_ops.rs#L2752-L2772): `resolve_spec_conflict` only recorded a generic `"auto-merged <ID>"` note without checking if any scalar fields collided.
- **Fix:**
  1. Updated `merge_spec_upstream` in [conflict.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/conflict.rs#L425-L560) to 3-way merge every scalar field via `merge_scalar(&base.field, &upstream.field, &local.field, true)` (preserving single-side local edits while keeping upstream-wins on true two-sided collisions).
  2. Added `overridden_scalar_fields(&base, &upstream, &local)` in [conflict.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/conflict.rs#L510-L560) and wired it into `resolve_spec_conflict` in [git_ops.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/git_ops.rs#L2752-L2772) so colliding fields append `(conflict: upstream overrode local <fields>)` to the rebase note.
- **Patch:** [0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch](file:///home/joe/ai/aida-multinode/patches/0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch)

---

### `BUG-MN-003` (P1): False-positive conflict banner on fast-forward `aida db sync --pull` & duplicate `auto-merged auto-merged` prefix
- **Symptom (S3):**
  1. After Node B rebased and pushed its store commits, Node A ran a clean fast-forward `aida db sync --pull` (with zero local uncommitted or unpushed changes). Despite being a pure fast-forward pull, `aida db sync --pull` printed a scary red `Conflicts detected:` banner for specs modified by Node B.
  2. When `AutoMerged` notes were printed, the CLI prefixed `"auto-merged "` onto a note that already started with `"auto-merged "`, printing `auto-merged auto-merged ...`.
- **Root Cause:**
  1. [git_backend_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/git_backend_cmd.rs#L1430-L1440) (`db_sync_local_window`) unconditionally ran `detect_store_conflicts(&local_reqs, &remote.requirements)` even when `pull_rebase_auto_merge` returned `StorePullOutcome::Clean` (a fast-forward pull), comparing pre-pull `local_reqs` against post-pull `remote.requirements` where `detect_conflict`'s fallback heuristic (`remote.modified_at > local.modified_at`) flagged normal incoming updates as conflicts.
  2. Both [git_backend_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/git_backend_cmd.rs#L1415) and [store_push_pull.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/store_push_pull.rs#L2587) printed `println!("  {} {}", "auto-merged".cyan().bold(), note)`.
- **Fix:**
  1. Gated `detect_store_conflicts` in `db_sync_local_window` on `matches!(outcome, StorePullOutcome::AutoMerged { .. })`.
  2. Stripped `"auto-merged "` prefix (`note.strip_prefix("auto-merged ").unwrap_or(note)`) before printing.
- **Patch:** [0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch](file:///home/joe/ai/aida-multinode/patches/0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch)

---

### `BUG-MN-004` (P1): `aida edit` on git backend omits `FieldChange` history entries for scalar and tag edits
- **Symptom (S4):** Editing `title`, `description`, `priority`, `type`, `owner`, `feature`, or `tags` via `aida edit <ID> ...` updated the YAML fields on disk and bumped `modified_at`, but left `req.history` completely empty (`history: []`), whereas `--status` recorded a `HistoryEntry` with `FieldChange { field_name: "status", ... }`.
- **Root Cause:** In [requirements_cli.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/requirements_cli.rs#L8584-L9220) (the `StoreCtx::Git` arm of `aida edit`), only the `if let Some(s) = status` block pushed a `FieldChange` onto `field_changes`.
- **Fix:** Added `FieldChange` capture (`old_value` -> `new_value`) for `title`, `description`, `priority`, `type`, `owner`, `feature`, `tags`, `add_tag`, and `remove_tag` in [requirements_cli.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/requirements_cli.rs#L8584-L9220) before `req.record_change(get_default_author(), field_changes)` is called.
- **Patch:** [0004-AI-antigravity-fix-edit-record-FieldChange-history-e.patch](file:///home/joe/ai/aida-multinode/patches/0004-AI-antigravity-fix-edit-record-FieldChange-history-e.patch)

---

### `BUG-MN-005` (P2): `aida db sync` leaves `.aida/cache-db/cache.db` stale after moving `.aida-store` HEAD
- **Symptom (S5/S6):** After running `aida db sync --pull` (or `--push` with rebase), `.aida-store` `HEAD` advanced to the new commit, but `.aida/cache-db/cache.db` still held the pre-sync `source_head_sha` (causing subsequent read-only snapshot commands like `aida show` to serve a `stale: true` projection or fall back to bounded object scans).
- **Root Cause:** In [db_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/db_cmd.rs#L806-L812), `git_backend_cmd.rs` opened `backend: CachedGitBackend` *before* `db_sync_local_window` ran, and `DbCommand::Sync` never refreshed `backend` after moving `.aida-store` `HEAD`.
- **Fix:** Added `if backend.cache().path().exists() { let _ = backend.ensure_cache_fresh_with_schema_retry(); }` at the end of `DbCommand::Sync` in [db_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/db_cmd.rs#L806-L812), performing an inline incremental cache update (or full rebuild after rebase) before `aida db sync` exits.
- **Patch:** [0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch](file:///home/joe/ai/aida-multinode/patches/0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch)

---

### `BUG-MN-006` (P2): `default_author()` in `aida-core::provenance` ignores `AIDA_USER`
- **Symptom (S2/S4):** While `oplog.yaml` and `node acquire` honored `AIDA_USER`, comments (`aida comment add`) and edit history entries (`aida edit`) fell back to OS `$USER` because `aida_core::provenance::default_author()` only checked `AIDA_AUTHOR` and `USER`/`USERNAME`.
- **Root Cause:** [provenance.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/provenance.rs#L397-L403) checked `std::env::var("AIDA_AUTHOR").or_else(|_| std::env::var("USER"))`.
- **Fix:** Added `.or_else(|_| std::env::var("AIDA_USER"))` immediately after `AIDA_AUTHOR` in `default_author()`.
- **Patch:** [0001-AI-antigravity-fix-provenance-honor-AIDA_USER-in-def.patch](file:///home/joe/ai/aida-multinode/patches/0001-AI-antigravity-fix-provenance-honor-AIDA_USER-in-def.patch)

---

### `BUG-MN-007` (P2): Mailbox recipient discovery on work-node clones, mailbox publication on `db sync --push`, and `aida init --with-mcp` flag gating
- **Symptom (S7 & Setup):**
  1. On a work-node clone (`aida node acquire` without a team-mode `registry/team.toml`), `aida mailbox send --to node-b ...` failed with `Error: unknown recipient 'node-b'` even though `node-b` had an active node lease and authored commits/specs in `.aida-store`.
  2. `aida db sync --push` did not digest local `.aida/mailbox/` messages into `<store>/mailbox/`, unlike `aida push`.
  3. `aida init` without `--with-mcp` still wrote global MCP server registrations into `~/.codex/config.toml` and `~/.gemini/antigravity/mcp_config.json`.
- **Root Cause & Fix:**
  1. Extended `PeopleCatalogView::load` in [mailbox_identity.rs](file:///home/joe/ai/aida-multinode-fixes/aida-core/src/mailbox_identity.rs#L204-L240) to discover known authors from `oplog.yaml` (`author:`) and `objects/*.yaml` (`created_by` and `comments[].author`) when `team.toml` does not list them.
  2. Wired `mailbox_publish_snapshot` / `publish_mailbox_snapshot` into `DbCommand::Sync` (`*push`) in [db_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/db_cmd.rs#L695-L731).
  3. Gated global Codex and Antigravity MCP registration in [init_cmd.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/init_cmd.rs#L1942-L1980) on `if with_mcp`.
- **Patches:**
  - [0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch](file:///home/joe/ai/aida-multinode/patches/0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch)
  - [0005-AI-antigravity-fix-init-gate-global-Codex-and-Antigr.patch](file:///home/joe/ai/aida-multinode/patches/0005-AI-antigravity-fix-init-gate-global-Codex-and-Antigr.patch)

---

### `BUG-MN-008` (P2): `gitlab_project_create.rs` masks `glab api` JSON OAuth/API error descriptions on project creation failure
- **Symptom (S12):** When `aida remote mirror gitlab --url ...` invoked `glab api --hostname <host> -X POST projects ...` and `glab` failed due to an expired OAuth token, `glab api` printed the JSON error (`{"error":"invalid_token","error_description":"Token is expired..."}`) to **stdout** and `glab: (HTTP 401)` to **stderr**. Because `create_gitlab_project` only inspected the last line of `stderr` when non-empty (or fell back to exit status), the user saw only `glab api exited with status: 1` or `glab: (HTTP 401)` without the `invalid_token` reason or re-auth command.
- **Fix:** Updated `create_gitlab_project` in [gitlab_project_create.rs](file:///home/joe/ai/aida-multinode-fixes/aida-cli-lib/src/gitlab_project_create.rs#L118-L148) to parse JSON error fields (`error_description`, `message`, `error`) from `stdout`/`stderr` and include an actionable hint (`glab auth login --hostname <host>`).
- **Patch:** [0006-AI-antigravity-fix-gitlab-surface-glab-api-JSON-erro.patch](file:///home/joe/ai/aida-multinode/patches/0006-AI-antigravity-fix-gitlab-surface-glab-api-JSON-erro.patch)

---

## 4. How to Apply the Patch Series to Upstream `aida`

### Option A: Apply the `git format-patch` series from `aida-multinode`
```bash
cd /home/joe/ai/aida
git checkout -b fix/multinode-sync-bugs
git am /home/joe/ai/aida-multinode/patches/*.patch
```

| Patch File | Commit Subject |
|---|---|
| [0001-AI-antigravity-fix-provenance-honor-AIDA_USER-in-def.patch](file:///home/joe/ai/aida-multinode/patches/0001-AI-antigravity-fix-provenance-honor-AIDA_USER-in-def.patch) | `[AI:antigravity] fix(provenance): honor AIDA_USER in default_author() fallback chain (BUG-MN-006)` |
| [0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch](file:///home/joe/ai/aida-multinode/patches/0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch) | `[AI:antigravity] fix(sync): force non-interactive rebase editor and 3-way scalar merge in merge_spec_upstream (BUG-MN-001 BUG-MN-002)` |
| [0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch](file:///home/joe/ai/aida-multinode/patches/0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch) | `[AI:antigravity] fix(sync): refresh cache on db sync, publish mailbox on push, and fix FF pull conflict detection (BUG-MN-003 BUG-MN-005 BUG-MN-007)` |
| [0004-AI-antigravity-fix-edit-record-FieldChange-history-e.patch](file:///home/joe/ai/aida-multinode/patches/0004-AI-antigravity-fix-edit-record-FieldChange-history-e.patch) | `[AI:antigravity] fix(edit): record FieldChange history entries for scalar and tag edits (BUG-MN-004)` |
| [0005-AI-antigravity-fix-init-gate-global-Codex-and-Antigr.patch](file:///home/joe/ai/aida-multinode/patches/0005-AI-antigravity-fix-init-gate-global-Codex-and-Antigr.patch) | `[AI:antigravity] fix(init): gate global Codex and Antigravity MCP registration on --with-mcp (BUG-MN-007)` |
| [0006-AI-antigravity-fix-gitlab-surface-glab-api-JSON-erro.patch](file:///home/joe/ai/aida-multinode/patches/0006-AI-antigravity-fix-gitlab-surface-glab-api-JSON-erro.patch) | `[AI:antigravity] fix(gitlab): surface glab api JSON error_description on project creation failure (BUG-MN-008)` |

### Option B: Merge or cherry-pick directly from the local worktree branch
The branch `fix/multinode-sync-bugs` is already present in the shared git object store of `/home/joe/ai/aida` (via the linked worktree at `/home/joe/ai/aida-multinode-fixes`):
```bash
git -C /home/joe/ai/aida log --oneline main..fix/multinode-sync-bugs
```

---

## 5. Architectural Direction: `aida-store-sync` Crate Extraction

To keep `aida-core` modular and make multi-node synchronization logic independently testable in millisecond unit tests (without compiling the full CLI workspace), we prototyped a standalone Rust crate at [crates/aida-store-sync](file:///home/joe/ai/aida-multinode/crates/aida-store-sync/src/lib.rs) in `aida-multinode` with **11 unit tests** covering:
1. **3-way scalar & collection merge (`merge_spec_3way`)** with explicit `overridden_scalars` conflict reporting (`BUG-MN-002`).
2. **Pull conflict detection (`detect_pull_conflicts`)** gated on `PullKind::{FastForward, RebasedAutoMerge}` (`BUG-MN-003`).
3. **Audit history `FieldChange` diffs (`diff_spec_fields`)** across all scalar and tag fields (`BUG-MN-004`).
4. **Non-interactive git rebase invocation builder (`rebase_continue_command`)** immune to `GIT_EDITOR=false` (`BUG-MN-001`).
5. **Provenance resolution (`resolve_author_from_env`)** honoring `AIDA_AUTHOR -> AIDA_USER -> git user.name -> USER` (`BUG-MN-006`).
6. **Multi-hub topology evaluation (`evaluate_multi_hub_sync`, `select_push_targets`)** modeling `origin` + mirror drift, `push-hubs`, and union reconciliation (`S8`–`S11`).
