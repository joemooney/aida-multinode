# Session Handoff — `aida-multinode`

**Date:** 2026-11-17  
**Repository:** `/home/joe/ai/aida-multinode`  
**Remotes (keep both in sync):**
- `origin`: `https://github.com/joemooney/aida-multinode.git`
- `gitlab`: `ssh://git@gitlab.joemooney.com:2222/ai/aida-multinode.git` (configured as a store mirror in `.aida/config.toml`)

---

## 1. Repository Layout & Key Artifacts

- **[REPORT.md](file:///home/joe/ai/aida-multinode/REPORT.md):** Initial multi-node sync audit report (`Setup` + `S1`–`S7`) and root-cause analysis for `BUG-MN-001`..`BUG-MN-007`.
- **[docs/UPSTREAM-REPORT.md](file:///home/joe/ai/aida-multinode/docs/UPSTREAM-REPORT.md):** Shareable upstream engineering report covering `BUG-MN-001`..`BUG-MN-009`, before/after verification (`19/19 PASS` under `--strict`), and patch instructions.
- **[docs/TEST-SCENARIOS.md](file:///home/joe/ai/aida-multinode/docs/TEST-SCENARIOS.md):** Detailed specification of automated test scenarios (`Setup` + `S1`–`S19`).
- **[harness/run_multinode_suite.sh](file:///home/joe/ai/aida-multinode/harness/run_multinode_suite.sh):** Self-contained, repeatable multi-node + multi-hub bash test harness. Now includes:
  - `Setup` + `S1`–`S7`: Core multi-node sync, conflict resolution, offline ID allocation, cache freshness, and mailbox.
  - `S8`–`S11`: Multi-hub GitLab mirror fan-out, drift detection (`remote status`), `remote mirror-sync`, `remote push-hubs gitlab`, and `remote reconcile --execute`.
  - `S13`: **Codex-only agent mode** (`--agent codex`, `CODEX_CI=1`, stripped `ANTIGRAVITY_*`/`CLAUDE_*` env vars).
  - `S14`–`S16`: **Global / agreed spec ID block allocation** across single-forge CAS (`ClaimAuthority::HubCas`), multi-forge rooted pool delegation (`enroll-root` + `node hub add gitlab`), and air-gapped node-local fallback.
  - `S17`–`S18`: **Read-only clone + per-node feedback branch** (`aida intent` on `refs/heads/aida-feedback-<node-id>` and `refs/heads/aida-feedback-receipts-<proc-id>`, plus `STORY-2652` Ruling `R4` foreign-write check).
  - `S19`: **Hourly inter-node heartbeat & allow-listed action channel**.
- **[crates/aida-store-sync/src/lib.rs](file:///home/joe/ai/aida-multinode/crates/aida-store-sync/src/lib.rs):** Zero-dependency standalone Rust crate prototyping extracted store-sync, 3-way scalar merge, multi-hub reconciliation, global ID block allocation (`S14`–`S16`), read-only feedback branch validation (`S17`–`S18`), and hourly heartbeat/action evaluation (`S19`). Run with `cargo test` (~1.2s, 11 unit tests passing).
- **[patches/](file:///home/joe/ai/aida-multinode/patches):** Exported `git format-patch` series from the linked upstream fix worktree.

---

## 2. Linked Upstream `aida` Worktree & Product Seat Mapping

- **Worktree Path:** `/home/joe/ai/aida-multinode-fixes` (branch `fix/multinode-sync-bugs`, rebased onto `origin/main` `fde1fb8145e`).
- **Patched Binary:** `/home/joe/ai/aida-multinode-fixes/target/debug/aida`
  *(Important: when compiling in `/home/joe/ai/aida-multinode-fixes`, export `CARGO_TARGET_DIR=/home/joe/ai/aida-multinode-fixes/target` so Cargo reuses the warm build cache rather than the shell wrapper's worktree target dir.)*
- **Upstream AIDA Spec Mapping (from Product Seat):**
  - **Mechanical fixes (ready for `main`):**
    - `BUG-2609` ← `BUG-MN-001` (`GIT_EDITOR=false` on `rebase --continue`), `BUG-MN-003` (false conflict on FF pull + duplicate `auto-merged` prefix), `BUG-MN-005` (`cache.db` refresh after `db sync`).
    - `BUG-2610` ← `BUG-MN-004` (`FieldChange` history entries on `aida edit`).
    - `BUG-2611` ← `BUG-MN-007` part 3 (`aida init --with-mcp` gating for Codex/Antigravity).
    - `BUG-2612` ← `BUG-MN-008` (`gitlab_project_create.rs` JSON `error_description` + `glab auth login` hint).
    - `STORY-2652` Ruling `R4` ← `BUG-MN-009` (`intent_intake.rs::sync_branch` own-branch foreign-write refusal on `refs/heads/aida-feedback-<writer>`).
  - **Held for advisor ruling (keep on separate branches out of `main`):**
    - `BUG-2613` ← `BUG-MN-002` (`merge_spec_upstream` 3-way scalar merge & override reporting).
    - `BUG-2614` ← `BUG-MN-006` (`AIDA_USER` in `default_author()`).
    - `BUG-2615` ← `BUG-MN-007` parts 1 & 2 (mailbox recipient discovery from `oplog`/`objects` + mailbox publish on `db sync --push`).
  - Trace comments (`// trace:BUG-2609 | ai:antigravity` .. `// trace:BUG-2615 | ai:antigravity`, `// trace:STORY-2652 | ai:antigravity`) and regression unit tests are committed in `/home/joe/ai/aida-multinode-fixes` and exported in `patches/`.

---

## 3. Immediate Next Steps for the `aida-multinode` Agent

1. **Rebuild `/home/joe/ai/aida-multinode-fixes/target/debug/aida` and run the expanded `S1`–`S19` suite:**
   ```bash
   CARGO_TARGET_DIR=/home/joe/ai/aida-multinode-fixes/target \
     cargo build --manifest-path /home/joe/ai/aida-multinode-fixes/Cargo.toml --bin aida

   ./harness/run_multinode_suite.sh \
     --aida-bin /home/joe/ai/aida-multinode-fixes/target/debug/aida
   ```
2. **Investigate & document any new findings from `S13`–`S19`:**
   - Check `S14`–`S16` (whether `aida add` automatically dispenses `agreed_id` from a claimed block on a work node or if there is a bug in `effective_policy_for` / `delegated_block` in `aida-core/src/allocation_authority.rs`).
   - Check `S17`–`S18` (specifically `BUG-MN-009` / `STORY-2652` Ruling `R4`: `sync_branch` in `aida-cli-lib/src/intent_intake.rs:1142-1158` currently fast-forwards a producer's own `refs/heads/aida-feedback-<writer>` branch when a foreign writer pushes to it instead of refusing).
   - Expand `S19` (inter-node hourly heartbeat & allow-listed action channel) and update `docs/TEST-SCENARIOS.md` with `S13`–`S19`.
3. **Keep both forges in sync after changes:**
   ```bash
   aida db sync --push
   git push origin main
   git push gitlab main
   aida remote status
   ```
