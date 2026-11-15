# AIDA Multi-Node & Multi-Hub Test Scenario Catalog

// trace:EPIC-MN-001 trace:STORY-MN-001 trace:STORY-MN-003 | ai:antigravity

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
