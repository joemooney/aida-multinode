# Architecture & Crate Extraction Blueprint: `aida-store-sync`

// trace:EPIC-MN-001 trace:STORY-MN-002 | ai:antigravity

## 1. Why Factor Multi-Node Store Sync into a Dedicated Crate?

In `/home/joe/ai/aida`, multi-node synchronization logic is currently split across several large modules in `aida-core` and `aida-cli-lib`:

| Current Location in `~/ai/aida` | Lines | Responsibilities & Coupling Issues |
| :--- | :--- | :--- |
| `aida-core/src/conflict.rs` | ~2,777 | Contains `detect_conflict`, `detect_store_conflicts`, `merge_spec_three_way`, `merge_spec_upstream`, `merge_scalar`, `union_history`, `union_comments`, and status-regression guards (`BUG-1625`). `merge_spec_upstream` bypasses `merge_scalar`, and `detect_conflict` lacks a `base` snapshot. |
| `aida-core/src/git_ops.rs` | ~7,500+ | Mixes low-level Git subprocess spawning (`host_git_command`), store rebase auto-merge (`rebase_auto_merge_onto`, `resolve_spec_conflict`, `pull_rebase`, `pull_rebase_store_retry`, `pull_rebase_auto_merge`), oplog unioning, node registration, and code-branch helpers. |
| `aida-core/src/provenance.rs` | ~891 | Handles `WriteContext`, `stamp_write_impl` (fallback `write_event`), and `default_author()`. Misses `AIDA_USER` in `default_author()`. |
| `aida-cli-lib/src/db_cmd.rs` & `git_backend_cmd.rs` | ~3,000+ | Implements `aida db sync --push` (`db_sync_push_leg`) and `aida db sync --pull` (`db_sync_local_window`) with asymmetric conflict reporting and no `CachedGitBackend` cache refresh. |
| `aida-cli-lib/src/requirements_cli.rs` | ~10,500 | Git-canonical `Command::Edit` mutates `Requirement` fields inline without building `FieldChange` history entries (`BUG-MN-004`). |

Because `aida` has high commit velocity across many subsystems (orchestrator, lanes, hooks, TUI, MCP, advisor gates), isolating the **distributed store synchronization engine** into a small, dependency-light crate (`crates/aida-store-sync`) provides:

1. **Fast, isolated verification:** Unit and multi-clone Git rebase tests run in `< 1s` without compiling the 100k+ LOC CLI.
2. **Single source of truth for merge & conflict semantics:** Both `pull` and `push` legs share one `SpecMergeOutcome` (containing the merged spec **plus** a structured `ScalarOverrideReport`), making silent scalar data loss impossible by construction.
3. **Hermetic Git execution:** Every Git subprocess spawned by the sync engine inherits `GIT_EDITOR=true` and `GIT_TERMINAL_PROMPT=0` in one place (`git_rebase.rs`).

---

## 2. Target Module Structure (`crates/aida-store-sync`)

```text
crates/aida-store-sync/
├── Cargo.toml
└── src/
    ├── lib.rs          # Public surface re-exports
    ├── model.rs        # Lightweight sync-relevant spec & history types (or trait adapter for aida_core::Requirement)
    ├── provenance.rs   # Author resolution (AIDA_AUTHOR -> AIDA_USER -> USER -> USERNAME) & field-change recorder
    ├── conflict.rs     # 3-way scalar/set/collection merge + ScalarOverrideReport + base-aware conflict detection
    └── git_rebase.rs   # Hermetic Git rebase --continue (GIT_EDITOR=true), stage :1:/:2:/:3: spec & oplog auto-merge
```

---

## 3. Mapping Prototype Fixes Back to `~/ai/aida`

When porting the fixes from `crates/aida-store-sync` into `/home/joe/ai/aida`:

1. **`BUG-MN-001` (`GIT_EDITOR=false` breaks store rebase):**
   - Port `git_rebase::run_git_hermetic` into `aida-core/src/git_ops.rs` (`host_git_command` at line 76 and `rebase_auto_merge_onto` at lines 2254, 2322).
2. **`BUG-MN-002` (`merge_spec_upstream` scalar preservation + override reporting):**
   - Port `conflict::merge_spec_upstream_with_report` into `aida-core/src/conflict.rs:428` and `aida-core/src/git_ops.rs:2692` (`resolve_spec_conflict`), and print `outcome.notes` in `aida-cli-lib/src/db_cmd.rs:500` (`db_sync_push_leg`).
3. **`BUG-MN-003` (False-positive conflict alerts on fast-forward pull & duplicate prefix):**
   - Port `conflict::detect_divergent_conflicts` (which requires `base` or an `AutoMerged` rebase outcome) into `aida-cli-lib/src/git_backend_cmd.rs:1420`, and strip `"auto-merged "` at `git_backend_cmd.rs:1415` and `store_push_pull.rs:2589`.
4. **`BUG-MN-004` (Missing `FieldChange` history on git-canonical `aida edit`):**
   - Port `provenance::diff_editable_fields` into `aida-cli-lib/src/requirements_cli.rs:8584–9079` (or `edit_rebase.rs`) so every scalar/tag mutation records a `FieldChange` before `stamp_write_impl`.
5. **`BUG-MN-005` (Stale `cache.db` after `aida db sync`):**
   - Add `CachedGitBackend::rebuild_cache_recovering()` to `DbCommand::Sync` in `aida-cli-lib/src/db_cmd.rs:765` whenever `head_sha(store_path)` changes.
6. **`BUG-MN-006` (`AIDA_USER` ignored by `default_author()`):**
   - Port `provenance::resolve_default_author` into `aida-core/src/provenance.rs:399`.

7. **`BUG-MN-008` (`aida remote mirror` SSH push-to-create vs `ls-remote` probe & expired `glab` token error masking):**
   - Port `parse_gitlab_namespace_id_or_error` into `aida-cli-lib/src/gitlab_project_create.rs:120` (`parse_namespace_id`) so expired `glab` tokens (`HTTP 401 invalid_token`) report `"Token is expired..."` instead of `"could not read the id of GitLab namespace"`.
   - In `aida-cli-lib/src/remote_create.rs` (`handle_remote_mirror`) and `gitlab_project_create.rs:426` (`ensure_gitlab_project_exists`), wire `.aida-store` remote, `[store.sync] mirror_remotes`, and the `pre-push` hook even when the `ls-remote` probe fails, or attempt SSH push-to-create before invoking `glab api`.
8. **`STORY-MN-003` (Multi-hub structural union-merge `reconcile_diverged_hubs`):**
   - `crates/aida-store-sync/src/lib.rs` includes `reconcile_diverged_hubs` and an end-to-end multi-hub test (`story_mn_003_multi_hub_reconcile_merges_diverged_origin_and_gitlab`) verifying structural union-merging across diverged `origin` and `gitlab` bare hubs.
