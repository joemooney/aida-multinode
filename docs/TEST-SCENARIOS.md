# AIDA Multi-Node & Multi-Hub Test Scenario Catalog

// trace:EPIC-MN-001 trace:STORY-MN-001 trace:STORY-MN-003 trace:STORY-MN-004 trace:BUG-MN-009 | ai:antigravity

This document is the canonical catalog of all multi-node and multi-hub (GitHub + GitLab) synchronization test scenarios covered in `aida-multinode`.

Every scenario below is automated in:
1. **End-to-End CLI Suite:** [`harness/run_multinode_suite.sh`](../harness/run_multinode_suite.sh)
2. **Crate Unit & Integration Suite:** [`crates/aida-store-sync/src/lib.rs`](../crates/aida-store-sync/src/lib.rs) (`cargo test`)

---

## Summary Matrix

| ID | Scenario Name | Category | Topology | Linked Specs | CLI Harness | `cargo test` |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Setup** | Seed initialization & fresh-clone store auto-attach | Bootstrap | `hub.git` + `seed` + `node-a` + `node-b` | `BUG-MN-007` | `Setup` | `bug_mn_001_and_002_*` |
| **S1** | Single-node `aida add` + push $\rightarrow$ pull on peer | Basic Sync | `node-a` $\rightarrow$ `hub.git` $\rightarrow$ `node-b` | `STORY-MN-001` | `S1` | — |
| **S2** | Cross-node `aida comment add` round-trip & `AIDA_USER` author stamp | Provenance & Sync | `node-b` $\rightarrow$ `hub.git` $\rightarrow$ `node-a` | `BUG-MN-006` | `S2` | `bug_mn_006_*` |
| **S3** | Concurrent disjoint spec edits & `oplog.yaml` auto-merge under `GIT_EDITOR=false` | Disjoint Merge | `node-a` $\leftrightarrow$ `hub.git` $\leftrightarrow$ `node-b` | `BUG-MN-001`, `BUG-MN-003` | `S3` | `bug_mn_001_and_002_*`, `bug_mn_003_*` |
| **S4a** | Concurrent same-spec scalar edits resolved on `aida db sync --push` | Conflict Resolution | `node-a` & `node-b` on same spec | `BUG-MN-002`, `BUG-MN-004` | `S4a` | `bug_mn_002_*`, `bug_mn_004_*` |
| **S4b** | Concurrent same-spec scalar edits resolved on `aida db sync --pull` | Conflict Resolution | `node-a` & `node-b` on same spec | `BUG-MN-002`, `BUG-MN-003`, `BUG-MN-004` | `S4a`/`S4b` | `bug_mn_002_*`, `bug_mn_004_*` |
| **S5** | Offline node-local ID allocation (5 specs on A, 5 on B) & online convergence | ID Allocation | `node-a` & `node-b` offline $\rightarrow$ online | `STORY-MN-001` | `S5` | — |
| **S6** | SQLite `cache.db` freshness immediately after `aida db sync` & `aida pull` | Cache Coherency | `node-a` $\rightarrow$ `node-b` (`cache.db`) | `BUG-MN-005` | `S6` | — |
| **S7** | Cross-node mailbox messaging (`--to <user>` vs `--broadcast`) | Mailbox Sync | `node-a` $\rightarrow$ `hub.git` $\rightarrow$ `node-b` | `BUG-MN-007` | `S7` | — |
| **S8** | Multi-hub GitLab mirror setup (`aida remote mirror gitlab`) & store fan-out | Multi-Hub (GitLab) | `node-a` $\rightarrow$ `origin` (`hub.git`) + `gitlab` (`gitlab.git`) | `STORY-MN-003`, `BUG-MN-008` | `S8` | — |
| **S9** | Multi-hub drift detection (`aida remote status`) & catch-up (`aida remote mirror-sync`) | Multi-Hub (GitLab) | `node-b` $\rightarrow$ `origin`; `node-a` syncs `gitlab` | `STORY-MN-003` | `S9` | — |
| **S10** | Restricted work-clone routing (`aida remote push-hubs gitlab`) | Multi-Hub (GitLab) | `node-c` $\rightarrow$ `gitlab.git` only | `STORY-MN-003` | `S10` | — |
| **S11** | Diverged multi-hub CRDT union-merge (`aida remote reconcile --execute`) | Multi-Hub (GitLab) | `origin` (`hub.git`) $\neq$ `gitlab` (`gitlab.git`) | `STORY-MN-003` | `S11` | `story_mn_003_*` |
| **S12** | GitLab SSH push-to-create vs `ls-remote` probe & expired `glab` token error reporting | Multi-Hub (GitLab) | `gitlab.joemooney.com` / API parser | `BUG-MN-008` | `S8` | `bug_mn_008_*` |
| **S13** | Codex-only agent mode (`aida init --agent codex`, `CODEX_CI=1`, stripped `ANTIGRAVITY_*`/`CLAUDE_*` env) | Agent Isolation | `codex-hub.git` + `codex-seed` + `codex-a` + `codex-b` | `STORY-MN-004` | `S13` | — |
| **S14** | Single-forge global/agreed ID block allocation (`[store.hubs]` + `aida db block claim`) | Global ID Allocation | `codex-a` & `codex-b` $\rightarrow$ `codex-hub.git` | `STORY-MN-003` | `S14` | `s14_s16_*` |
| **S15** | Multi-forge rooted ID pool delegation (`aida node enroll-root` + `aida node hub add gitlab`) | Global ID Allocation | `codex-a` (root) $\rightarrow$ `codex-gitlab.git` $\leftarrow$ `codex-c` | `STORY-MN-003` | `S15` | `s14_s16_*` |
| **S16** | Air-gapped clone node-local ID fallback & online convergence | Air-Gapped ID Allocation | `codex-airgap` (offline $\rightarrow$ online) | `STORY-MN-003` | `S16` | `s14_s16_*` |
| **S17** | Read-only clone (`main` & `aida-store` push-blocked) publishing to `refs/heads/aida-feedback-<node-id>` | Read-Only Feedback | `node-ro` $\rightarrow$ `hub.git` $\leftrightarrow$ `node-a` | `STORY-2652` | `S17` | `s17_s18_*` |
| **S18** | Producer own-branch foreign-write refusal (`STORY-2652` Ruling `R4` / `BUG-MN-009`) | Read-Only Feedback | `foreign` $\rightarrow$ `refs/heads/aida-feedback-<ro>` $\leftarrow$ `node-ro` | `BUG-MN-009`, `STORY-2652` | `S18` | `s17_s18_*` |
| **S19** | Hourly inter-node heartbeat & allow-listed action channel (`PullCode`, `PullStore`, `PushStore` guard) | Heartbeat & Actions | `node-a` $\rightarrow$ `hub.git` $\rightarrow$ `node-ro` | `STORY-MN-004` | `S19` | `s19_*` |

---

## Detailed Scenario Specifications

### Setup: Seed Initialization & Secondary Clone Store Auto-Attachment
- **Goal:** Verify the new-user onboarding workflow where one developer initializes AIDA (`seed`), pushes `main` and `aida-store` to a bare hub (`hub.git`), and two teammates (`node-a`, `node-b`) clone the hub and attach the existing store without running `aida init` a second time.
- **Steps:**
  1. Create bare repository `$T/hub.git` (`git init --bare -b main`).
  2. Create `$T/seed` with `README.md` and `src/lib.rs`, commit on `main`, and run `aida init`.
  3. Push `main` and `.aida-store` (`aida-store` branch) to `$T/hub.git`.
  4. Clone `$T/hub.git` into `$T/node-a` (`HOME=$T/home-a`, `AIDA_USER=node-a`) and `$T/node-b` (`HOME=$T/home-b`, `AIDA_USER=node-b`).
  5. Run `aida node acquire` and `aida node alias set NODEA` / `NODEB` in each clone.
  6. Add 3 starter specs on `node-a` (`STORY`, `BUG`, `TASK`), push from `node-a`, and pull on `node-b`.
- **Expected Invariants:**
  - Running `aida node acquire` or `aida db status` in a fresh clone automatically attaches `.aida-store` as a git worktree tracking `origin/aida-store`.
  - Each clone receives a distinct 32-hex UUID node binding in `.git/aida/work-binding.toml`.
  - Bare `aida init` (without `--with-mcp`) must **not** mutate global Codex/Antigravity MCP settings (`BUG-MN-007`).

---

### Scenario S1: Single-Node Add + Push $\rightarrow$ Pull on Peer
- **Goal:** Verify basic spec creation, store publication, and peer retrieval.
- **Steps:**
  1. On `node-a`: `aida add --type functional --title "S1 Feature from A" --description "S1 body"` and `aida db sync --push`.
  2. On `node-b`: `aida db sync --pull` and `aida show FR-NODEA-001`.
- **Expected Invariants:**
  - `FR-NODEA-001` is immediately readable via `aida show` and `aida list` on `node-b` with identical title, description, status, and filing provenance.

---

### Scenario S2: Cross-Node Comment Round-Trip & `AIDA_USER` Author Provenance
- **Goal:** Verify that comments added on `node-b` sync cleanly to `node-a` and record `AIDA_USER` (`node-b`) as the comment author.
- **Steps:**
  1. On `node-b` (with `AIDA_USER=node-b`, `AIDA_AUTHOR` unset): `aida comment add STORY-NODEA-001 "S2 comment from node-b"` and `aida db sync --push`.
  2. On `node-a`: `aida db sync --pull` and inspect `STORY-NODEA-001`.
- **Expected Invariants:**
  - Comment content and timestamp appear on `node-a`.
  - Comment `author` field equals `"node-b"` from `AIDA_USER`, not the host OS `$USER` (`BUG-MN-006`).

---

### Scenario S3: Concurrent Disjoint Spec Edits & Hermetic `oplog.yaml` Rebase
- **Goal:** Verify that when two nodes concurrently edit **different** specs (and both append to `oplog.yaml`), syncing auto-merges cleanly even when the caller environment sets `GIT_EDITOR=false`.
- **Steps:**
  1. While offline (or before pushing), `node-a` edits/adds a spec (`S3 Task A`) and `node-b` edits/adds a different spec (`S3 Task B`).
  2. `node-a` runs `aida db sync --push`.
  3. `node-b` runs `GIT_EDITOR=false aida db sync --push` (triggering `pull_rebase_store_retry` $\rightarrow$ `rebase_auto_merge_onto`).
  4. `node-a` runs `aida db sync --pull`.
- **Expected Invariants:**
  - `git rebase --continue` inside `rebase_auto_merge_onto` succeeds under `GIT_EDITOR=false` (`BUG-MN-001`).
  - `oplog.yaml` unions both nodes' operations without manual intervention.
  - `node-a`'s fast-forward `aida db sync --pull` does **not** print false-positive `Conflicts detected:` warnings (`BUG-MN-003`).

---

### Scenario S4 (S4a & S4b): Concurrent Same-Spec Scalar Edits (`--push` vs `--pull`)
- **Goal:** Verify 3-way per-field scalar merge, field-level history recording, and explicit conflict/override reporting when two nodes concurrently edit the **same** spec.
- **Steps:**
  1. **S4a (`--push` auto-rebase):** `node-a` and `node-b` concurrently edit `FR-NODEA-001` (`title`, `description`, `priority`). `node-a` pushes first; `node-b` runs `aida db sync --push`.
  2. **S4b (`--pull` auto-rebase):** `node-a` and `node-b` concurrently edit `BUG-NODEA-001`. `node-a` pushes first; `node-b` runs `aida db sync --pull`.
- **Expected Invariants:**
  - **Field-level history (`BUG-MN-004`):** Both `node-a` and `node-b`'s `HistoryEntry` items contain explicit `FieldChange { field_name, old_value, new_value }` records (not `changes: []`), and both history entries survive in the unioned `history` array.
  - **Non-colliding scalar preservation (`BUG-MN-002`):** If `node-a` changed `title` and `node-b` changed `description`, **both** scalar edits survive `merge_spec_upstream`.
  - **Colliding scalar override visibility (`BUG-MN-002`):** If both changed `title` to different values, `upstream` (`node-a`) wins the scalar field **and** both `aida db sync --push` (S4a) and `aida db sync --pull` (S4b) explicitly report which local scalar fields were overridden by upstream.
  - **Clean output (`BUG-MN-003`):** Auto-merge status lines print `auto-merged <ID>: ...` (not `auto-merged auto-merged ...`).

---

### Scenario S5: Offline Node-Local ID Allocation (5 Specs on A, 5 on B)
- **Goal:** Verify that multiple nodes can allocate spec IDs while completely disconnected from the hub without ID collisions.
- **Steps:**
  1. Make `$T/hub.git` unreachable.
  2. Run `aida add --type task` 5 times on `node-a` and 5 times on `node-b`.
  3. Restore `$T/hub.git`, run `aida db sync --push` on `node-a`, `aida db sync --push` on `node-b`, and `aida db sync --pull` on `node-a`.
- **Expected Invariants:**
  - Offline `aida add` commits locally and prints an offline notice without failing.
  - `node-a` dispenses `TASK-<node-a>-004..008` and `node-b` dispenses `TASK-<node-b>-002..006` (zero collisions).
  - After sync, both nodes have all 10 new tasks.

---

### Scenario S6: SQLite Cache Freshness (`cache.db`) Across Sync Commands
- **Goal:** Verify that `.aida/cache-db/cache.db` is synchronously refreshed whenever `.aida-store` `HEAD` advances during `aida db sync --pull`, `aida db sync --push`, or `aida pull`.
- **Steps:**
  1. Warm `cache.db` on `node-b` by running `aida list`.
  2. Add a spec on `node-a` and push (`aida db sync --push`).
  3. Run `aida db sync --pull` on `node-b`, then immediately run `aida cache status` and `aida list --format toon`.
  4. Run `AIDA_PULL_TIMING=1 aida pull` and inspect phase timings.
- **Expected Invariants:**
  - `aida cache status` immediately reports fresh (`stale: false`) after `aida db sync --pull` (`BUG-MN-005`).
  - The very first `aida list --format toon` call after `aida db sync --pull` includes the newly pulled spec without requiring a second read after a background refresh.

---

### Scenario S7: Cross-Node Mailbox Send & Receive
- **Goal:** Verify durable cross-clone messaging over `.aida-store/mailbox/`.
- **Steps:**
  1. On `node-a`: send a message to `node-b` (`aida mailbox send --to node-b --subject "S7" "Direct"`, or `--broadcast` / after `aida node team set-role`).
  2. Sync store (`aida db sync --push` on `node-a`, `aida db sync --pull` on `node-b`).
  3. On `node-b`: `aida mailbox inbox` and `aida mailbox read <MSG-ID>`.
- **Expected Invariants:**
  - Direct recipient addressing either discovers work-node teammates or provides an actionable registration command in the refusal hint (`BUG-MN-007`).
  - Sent messages propagate across `.aida-store` and appear unread in `node-b`'s inbox.

---

### Scenario S8: Multi-Hub GitLab Mirror Setup & Store Fan-Out
- **Goal:** Verify that registering a second hub (`gitlab`) via `aida remote mirror gitlab --url <url>` fans out store pushes to both `origin` and `gitlab`.
- **Steps:**
  1. Create a second bare repository `$T/gitlab.git`.
  2. On `node-a`, run `aida remote mirror gitlab --url "$T/gitlab.git"`, commit `.aida/config.toml` (`[store.sync] mirror_remotes = ["gitlab"]`), and push `main`.
  3. Add a spec on `node-a` and run `aida db sync --push`.
- **Expected Invariants:**
  - `refs/heads/aida-store` on `$T/hub.git` (`origin`) and `$T/gitlab.git` (`gitlab`) point to the exact same commit SHA.

---

### Scenario S9: Multi-Hub Drift Detection (`aida remote status`) & `aida remote mirror-sync` Catch-Up
- **Goal:** Verify that when `origin` advances ahead of `gitlab` (e.g. a push from a clone that only knows `origin`), `aida remote status` flags the drift and `aida remote mirror-sync` levels `gitlab`.
- **Steps:**
  1. On `node-b` (which only has `origin`), add a spec and run `aida db sync --push`.
  2. On `node-a` (which has both `origin` and `gitlab`), run `aida remote status` (expect non-zero exit code).
  3. On `node-a`, run `aida db sync --pull` and `aida remote mirror-sync`, then re-run `aida remote status`.
- **Expected Invariants:**
  - `aida remote status` exits non-zero when `origin/aida-store != gitlab/aida-store`.
  - `aida remote mirror-sync` fast-forwards `gitlab/aida-store` to `origin/aida-store`, after which `aida remote status` exits `0`.

---

### Scenario S10: Restricted Work-Clone Routing (`aida remote push-hubs gitlab`)
- **Goal:** Verify that a work clone configured with `aida remote push-hubs gitlab` publishes store updates exclusively to `gitlab` and never pushes to `origin`.
- **Steps:**
  1. Clone `$T/gitlab.git` into `$T/node-c` (`HOME=$T/home-c`, `AIDA_USER=node-c`), wire both `origin` (`$T/hub.git`) and `gitlab` (`$T/gitlab.git`), and run `aida remote push-hubs gitlab`.
  2. On `node-c`, add a spec (`S10 GitLab-Only Task from C`) and run `aida db sync --push`.
- **Expected Invariants:**
  - `$T/gitlab.git` `aida-store` advances with `node-c`'s commit, while `$T/hub.git` (`origin`) `aida-store` remains untouched.

---

### Scenario S11: Diverged Multi-Hub CRDT Union-Merge (`aida remote reconcile --execute`)
- **Goal:** Verify that when `origin/aida-store` and `gitlab/aida-store` have diverged with independent commits on both hubs, `aida remote reconcile --execute` structurally union-merges both tips and fast-forwards both hubs to the union commit.
- **Steps:**
  1. Following S10 (`gitlab` has `node-c`'s new spec), add a different spec on `node-b` (`S10 Origin-Only Task from B`) and push to `origin`. Now `origin/aida-store` and `gitlab/aida-store` have diverged.
  2. On `node-a` (the bridge clone reaching both hubs), run `aida remote reconcile` (dry-run; expect exit code `2`).
  3. On `node-a`, run `aida remote reconcile --execute`.
- **Expected Invariants:**
  - Dry-run `aida remote reconcile` exits `2` and prints the per-hub divergence plan.
  - `aida remote reconcile --execute` creates a merge commit with both hub tips as parents, preserves specs and `oplog.yaml` entries from both hubs, and fast-forwards both `origin` and `gitlab` to the new union SHA.
  - `aida remote status` subsequently exits `0`.

---

### Scenario S12: GitLab SSH Push-to-Create vs `ls-remote` Probe & Expired `glab` Token Error Reporting
- **Goal:** Ensure `aida remote mirror` works seamlessly with GitLab SSH push-to-create and surfaces clear API errors when `glab` tokens expire (`BUG-MN-008`).
- **Expected Invariants:**
  - When `glab api` returns `{"error":"invalid_token","error_description":"Token is expired..."}`, the parser surfaces `"Token is expired..."` instead of `"could not read the id of GitLab namespace"` (`bug_mn_008_gitlab_namespace_parser_surfaces_expired_token_error`).

---

### Scenario S13: Codex-Only Agent Mode (`--agent codex`, `CODEX_CI=1`)
- **Goal:** Verify that AIDA initializes, files specs, records provenance, and syncs across nodes in a pure Codex environment (`env -i` with `CODEX_CI=1` and zero `ANTIGRAVITY_*` or `CLAUDE_*` environment variables).
- **Steps:**
  1. Run `aida init --agent codex --no-hooks --no-post-hooks` inside `$T/codex-seed` under `env -i PATH="$PATH" HOME="$T/home-codex-seed" USER=codex-seed AIDA_USER=codex-seed CODEX_CI=1`.
  2. Push `main` and `aida-store` to `$T/codex-hub.git`, clone `$T/codex-a` and `$T/codex-b`, acquire node bindings, set alias `CXA`, file `TASK-CXA-001` on `codex-a`, comment from `codex-b`, and sync back to `codex-a`.
- **Expected Invariants:**
  - `AGENTS.md`, `.codex/`, and `.agents/` are scaffolded while `CLAUDE.md` and `.claude/` are omitted.
  - Filing provenance records `· codex` (`Filed at: <sha> (main) · aida ... · codex`).
  - Cross-node `db sync --push` / `--pull` and `show --comments` round-trip cleanly under `CODEX_CI=1`.

---

### Scenario S14: Single-Forge Global / Agreed Spec ID Block Allocation
- **Goal:** Verify that two non-root work nodes (`codex-a`, `codex-b`) sharing a single declared hub (`[store.hubs] primary = ...`) can claim non-overlapping global ID blocks via CAS push (`ClaimAuthority::HubCas`) and dispense global agreed IDs (`TASK-1`, `TASK-6`) at `aida add` time without a merge gate.
- **Steps:**
  1. Configure `[ids] counter_scope = "per-type"`, `format = "agreed-then-node-aware"`, and `[store.hubs] primary = { urls = ["$T/codex-hub.git"] }` in `.aida/config.toml`; commit and push to `origin/main`.
  2. On `codex-a`: run `aida node upstream origin`, `aida db block claim --type TASK --size 5`, `aida add --type task`, and `aida db sync --push`.
  3. On `codex-b`: pull `main` and `aida-store`, run `aida node upstream origin`, `aida db block claim --type TASK --size 5`, `aida add --type task`, and `aida db sync --push`.
- **Expected Invariants:**
  - `codex-a` claims `TASK 1..=5` (`pool_id: hub:primary:TASK`) and dispenses `agreed_id: TASK-1` alongside its node-local `spec_id`.
  - `codex-b` claims `TASK 6..=10` (`pool_id: hub:primary:TASK`) and dispenses `agreed_id: TASK-6`.

---

### Scenario S15: Multi-Forge Rooted ID Pool Delegation (`enroll-root` + `node hub add gitlab`)
- **Goal:** Verify that when a second forge (`gitlab`) is added, enrolling an allocation root (`aida node enroll-root`) and delegating a child ID pool (`aida node hub add gitlab --url ... --size 100`) enables clones of `gitlab` (`codex-c`) to claim blocks inside the delegated pool (`ClaimAuthority::Delegated`) and dispense global agreed IDs (`TASK-11`) that cannot collide with `origin`.
- **Steps:**
  1. On `codex-a`, clear upstream, run `aida node enroll-root`, and run `aida node hub add gitlab --url "$T/codex-gitlab.git" --size 100`.
  2. Commit `.aida/config.toml` and push `main` and `aida-store` to both `origin` and `gitlab`.
  3. Clone `$T/codex-gitlab.git` into `$T/codex-c`, run `aida node acquire`, `aida node upstream origin`, `aida db block claim --type TASK --size 5`, `aida add --type task`, and `aida db sync --push`.
- **Expected Invariants:**
  - `registry/root.yaml` records `codex-a` as the root and `registry/pools.yaml` records `hub:gitlab:TASK` (`11..=110`) under parent `hub:primary:TASK`.
  - `codex-c` carves `TASK 11..=15` from `hub:gitlab:TASK` and dispenses `agreed_id: TASK-11`.

---

### Scenario S16: Air-Gapped Clone Node-Local ID Fallback & Online Convergence
- **Goal:** Verify that when a clone (`codex-airgap`) is completely disconnected from all hubs and holds no global ID block, `aida add` gracefully falls back to node-local IDs (`TASK-<node-uuid>-001`, `agreed_id: null`) and later syncs cleanly once connectivity returns.
- **Steps:**
  1. Clone `$T/codex-hub.git` into `$T/codex-airgap` and run `aida node acquire` (without claiming any block).
  2. Make `$T/codex-hub.git` unreachable (`mv`), run `aida add --type task --title "S16 Airgapped Task"`, restore `$T/codex-hub.git`, and run `aida db sync --push`.
- **Expected Invariants:**
  - `aida add` succeeds while air-gapped, issuing `TASK-<32-hex-node-id>-001` with `agreed_id: null`.
  - Subsequent `aida db sync --push` publishes the spec to the hub with zero collisions.

---

### Scenario S17: Read-Only Clone + Per-Node Feedback Branch (`aida intent` / `STORY-2652`)
- **Goal:** Verify that a read-only clone (`node-ro`), forbidden by a server `pre-receive` hook from pushing to `refs/heads/main` or `refs/heads/aida-store`, can still submit structured observations, questions, and requests via its exclusive `refs/heads/aida-feedback-<node-id>` branch and receive processing receipts on `refs/heads/aida-feedback-receipts-<processor-id>`.
- **Steps:**
  1. Install a `pre-receive` hook on `$T/hub.git` that denies all pushes when `AIDA_READONLY_CLONE=1` unless `$refname == refs/heads/aida-feedback-$RO_WRITER`.
  2. Confirm that `git push origin main` and `git -C .aida-store push origin aida-store` from `node-ro` fail with non-zero exit codes.
  3. On `node-ro`: enroll as `producer` (`aida intent enroll --role producer --writer-id $RO_WRITER --receipt-writer $HOME_PROC --reviewed`), create an observation note (`aida intent note`), and publish (`aida intent publish --file ... --reviewed`).
  4. On `node-a`: enroll as `processor` (`--role processor --writer-id $HOME_PROC --producer-id $RO_WRITER --reviewed`), run `aida intent pull`, and process the observation into a new spec (`aida intent process <checksum> --action create`).
  5. On `node-ro`: run `aida intent pull` and `aida intent list --all --json`.
- **Expected Invariants:**
  - `main` and `aida-store` pushes from `node-ro` are rejected by the server hook.
  - `node-ro` publishes exclusively to `refs/heads/aida-feedback-$RO_WRITER`.
  - `node-a` creates the spec in `aida-store`, publishes the receipt to `refs/heads/aida-feedback-receipts-$HOME_PROC`, and `node-ro` sees `"state":"processed"` on `aida intent list --all --json`.

---

### Scenario S18: Producer Own-Branch Foreign-Write Refusal (`STORY-2652` Ruling `R4` / `BUG-MN-009`)
- **Goal:** Verify `STORY-2652` Advisor Ruling `R4`: if a foreign writer pushes a commit onto a producer's own `refs/heads/aida-feedback-<writer>` branch (advancing the remote tip ahead of what the producer clone itself published), the producer's `aida intent pull` and `aida intent publish` must **refuse** rather than silently fast-forwarding the foreign commit into its accepted local branch.
- **Steps:**
  1. Clone `refs/heads/aida-feedback-$RO_WRITER` into `$T/foreign-intake`, add a valid observation envelope with `AIDA Intake <intake@invalid>` commit metadata, and push directly to `refs/heads/aida-feedback-$RO_WRITER` on `$T/hub.git`.
  2. On `node-ro` (the legitimate owner of `$RO_WRITER`), run `aida intent pull`.
- **Expected Invariants:**
  - `aida intent pull` exits non-zero with `"Foreign write detected on enrolled writer branch ..."` and leaves `node-ro`'s accepted local branch unchanged (`BUG-MN-009`).

---

### Scenario S19: Hourly Inter-Node Heartbeat & Allow-Listed Action Channel
- **Goal:** Verify that nodes can exchange periodic liveness/state heartbeats (`code_head_sha`, `store_head_sha`, `executed_directives`) and execute directed or broadcast maintenance actions from a closed allow-list (`PullCode`, `PullStore`, `PushStore`, `PullFeedback`, `PublishFeedback`, `ReportStatus`) idempotently, while read-only clones automatically refuse store-mutating directives (`PushStore`).
- **Steps:**
  1. `node-a` commits and pushes a code update on `main` to `$T/hub.git`.
  2. `node-ro` executes its allow-listed hourly tick actions (`aida pull --code-only` and `aida db sync --pull`), advancing its `HEAD` to match `origin/main`.
  3. Run `s19_hourly_heartbeat_executes_allowlisted_actions_idempotently` in `crates/aida-store-sync` to verify allow-list dispatch, `refused:read-only-clone` on `PushStore`, and second-tick idempotency via `DirectiveReceipt`.
- **Expected Invariants:**
  - Only closed `NodeAction` enum variants map to CLI arguments; arbitrary shell strings cannot be represented.
  - Read-only clones execute `PullCode` / `PullStore` / `PullFeedback` and record `refused:read-only-clone` for `PushStore`.
  - Re-evaluating the same inbox on subsequent hourly ticks is a strict no-op.
