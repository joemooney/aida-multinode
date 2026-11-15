# Upstream AIDA Patch Series (`fix/multinode-sync-bugs`)

This directory contains a 6-commit `git format-patch` series built and verified in the isolated worktree `/home/joe/ai/aida-multinode-fixes` (branch `fix/multinode-sync-bugs` off `/home/joe/ai/aida` `main`) that resolves `BUG-MN-001` through `BUG-MN-008`.

## Patch Inventory

| Patch File | Spec ID(s) | Summary |
|---|---|---|
| `0001-AI-antigravity-fix-provenance-honor-AIDA_USER-in-def.patch` | `BUG-MN-006` | Honors `AIDA_USER` immediately after `AIDA_AUTHOR` in `aida_core::provenance::default_author()` so CLI and test environments using `AIDA_USER` attribute specs, comments, and history entries consistently. |
| `0002-AI-antigravity-fix-sync-force-non-interactive-rebase.patch` | `BUG-MN-001`, `BUG-MN-002` | Passes `-c core.editor=true` + `GIT_EDITOR=true` + `GIT_SEQUENCE_EDITOR=true` on automated `git rebase --continue` in `git_ops.rs`; upgrades `merge_spec_upstream` in `conflict.rs` to 3-way merge all scalar fields (`merge_scalar(&base.field, &upstream.field, &local.field, true)`) and adds `overridden_scalar_fields` so `resolve_spec_conflict` reports `(conflict: upstream overrode local <fields>)` when upstream wins colliding scalar edits during `aida db sync --push`. |
| `0003-AI-antigravity-fix-sync-refresh-cache-on-db-sync-pub.patch` | `BUG-MN-003`, `BUG-MN-005`, `BUG-MN-007` | Avoids false-positive `detect_store_conflicts` warnings on fast-forward `aida db sync --pull` (only running conflict detection when `StorePullOutcome::AutoMerged`), strips duplicate `auto-merged ` note prefixes, refreshes `cache.db` via `ensure_cache_fresh_with_schema_retry()` after `aida db sync`, publishes local `.aida/mailbox/` snapshots on `aida db sync --push`, and discovers known authors from `oplog.yaml` / `objects/` in `PeopleCatalogView::load` for work-node clones. |
| `0004-AI-antigravity-fix-edit-record-FieldChange-history-e.patch` | `BUG-MN-004` | Records `FieldChange` history entries on `aida edit` for `title`, `description`, `priority`, `type`, `owner`, `feature`, `tags`, `add_tag`, and `remove_tag` (matching `status`), enabling audit trails and history-based conflict detection. |
| `0005-AI-antigravity-fix-init-gate-global-Codex-and-Antigr.patch` | `BUG-MN-007` | Gates global `~/.codex/config.toml` and `~/.gemini/antigravity/mcp_config.json` MCP server registration in `aida init` behind `--with-mcp`. |
| `0006-AI-antigravity-fix-gitlab-surface-glab-api-JSON-erro.patch` | `BUG-MN-008` | Parses JSON error payloads (`error_description`, `message`, `error`) from `glab api -X POST projects` in `gitlab_project_create.rs` and suggests `glab auth login --hostname <host>` instead of reporting only `glab api exited with status: 1`. |

## Applying the Series to Upstream `aida`

```bash
cd /path/to/aida
git checkout -b fix/multinode-sync-bugs
git am /path/to/aida-multinode/patches/*.patch
```

## Verification (`harness/run_multinode_suite.sh --strict`)

```bash
./harness/run_multinode_suite.sh \
  --aida-bin /home/joe/ai/aida-multinode-fixes/target/debug/aida \
  --strict
```

| Scenario | Unpatched (`0.16.0` baseline) | Patched (`fix/multinode-sync-bugs`) | Notes |
|---|---|---|---|
| `Setup (hub+seed+A+B)` | `PASS` | `PASS` | Store auto-attached on A & B; 3 starter specs synced |
| `S1 (Add on A -> Pull on B)` | `PASS` | `PASS` | `FR-NODEA-001` visible on B |
| `S2 (Comment B -> A)` | `WARN` (`BUG-MN-006`) | `PASS` | Comment synced; `author=node-b` |
| `S3 (Disjoint edits + rebase)` | `FAIL` (`BUG-MN-001` / `BUG-MN-003`) | `PASS` | Clean oplog + disjoint spec auto-merge |
| `S4a (Same-spec edit --push)` | `FAIL` (`BUG-MN-002` / `BUG-MN-004`) | `PASS` | Push leg surfaced scalar override + preserved non-colliding fields |
| `S5 (5x2 offline IDs + sync)` | `PASS` | `PASS` | 10 offline specs merged with 0 ID collisions |
| `S6 (Cache freshness on sync)` | `WARN` (`BUG-MN-005`) | `PASS` | `cache.db` fresh immediately after `aida db sync --pull` |
| `S7 (Mailbox A -> B)` | `WARN` (`BUG-MN-007`) | `PASS` | Direct `--to node-b` succeeded and published via `db sync --push` |
| `S8 (GitLab mirror fan-out)` | `PASS` | `PASS` | `origin` & `gitlab` store tips match |
| `S9 (Drift + mirror-sync)` | `PASS` | `PASS` | `remote status` caught drift; `mirror-sync` leveled `gitlab` |
| `S10 (push-hubs gitlab on C)` | `PASS` | `PASS` | Node C pushed only to `gitlab`; hubs diverged as expected |
| `S11 (remote reconcile)` | `PASS` | `PASS` | Diverged `origin` & `gitlab` union-merged |
| **Total (`--strict`)** | **2 FAIL, 3 WARN** (exit `1`) | **12/12 PASS, 0 FAIL, 0 WARN** (exit `0`) | All unit tests in `aida-core` & `aida-cli-lib` pass |
