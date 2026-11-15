# aida-multinode

Dedicated testbed, scenario catalog, behavioral specification, root-cause repository, and standalone Rust crate prototype (`aida-store-sync`) for [AIDA](https://github.com/joemooney/aida) multi-node and multi-hub (GitHub + GitLab) synchronization.

---

## Repository Contents

| Path | Description |
| :--- | :--- |
| **[`docs/TEST-SCENARIOS.md`](docs/TEST-SCENARIOS.md)** | **Canonical catalog of all 13 test scenarios (`Setup`, `S1`–`S12`)** covering single-hub peer sync, concurrent disjoint/same-spec merges, offline ID allocation, SQLite cache coherency, mailbox messaging, and multi-hub GitLab mirroring (`mirror`, `status`, `mirror-sync`, `push-hubs`, `reconcile`). |
| [`harness/run_multinode_suite.sh`](harness/run_multinode_suite.sh) | Automated, repeatable end-to-end CLI test runner (`Setup` + `S1`–`S11`). Creates isolated bare hubs (`hub.git`, `gitlab.git`), `seed`, and peer clones (`node-a`, `node-b`, `node-c`) with separate `$HOME` and `AIDA_USER` identities, executing all scenarios and printing a pass/fail/warn + latency table. |
| [`crates/aida-store-sync/`](crates/aida-store-sync/) | Standalone, zero-dependency Rust crate (`aida-store-sync`) factoring out 3-way per-field spec merging (`merge_spec_upstream_with_report`), base-aware conflict detection (`detect_conflict_with_base`), hermetic Git rebase auto-merging (`rebase_auto_merge_onto`), multi-hub structural union-merging (`reconcile_diverged_hubs`), GitLab namespace API error parsing (`parse_gitlab_namespace_id_or_error`), and consistent author provenance (`resolve_default_author`). |
| [`docs/REPORT-2026-11-15.md`](docs/REPORT-2026-11-15.md) | Full baseline multi-node test report, scenario-by-scenario transcripts and timings, code-level root causes in `~/ai/aida`, proposed fixes, and prioritized implementation plan. |
| [`docs/MULTINODE-CONTRACT.md`](docs/MULTINODE-CONTRACT.md) | Canonical behavioral contract and invariants for AIDA multi-node & multi-hub synchronization. |
| [`docs/ARCHITECTURE-EXTRACTION.md`](docs/ARCHITECTURE-EXTRACTION.md) | Architecture blueprint mapping `crates/aida-store-sync` back to `aida-core` and `aida-cli-lib`. |
| [`.aida-store/`](.aida-store/) | AIDA git-canonical store tracking `EPIC-MN-001`, `BUG-MN-001` through `BUG-MN-008`, and `STORY-MN-001` through `STORY-MN-003` (under friendly node alias `MN`). |

---

## Test Scenario Summary (`Setup` + `S1`–`S12`)

See [`docs/TEST-SCENARIOS.md`](docs/TEST-SCENARIOS.md) for the full step-by-step commands and invariants for each scenario:

| ID | Scenario | Category | Linked Specs |
| :--- | :--- | :--- | :--- |
| **Setup** | Seed initialization & fresh-clone store auto-attach | Bootstrap | `BUG-MN-007` |
| **S1** | Single-node `aida add` + push $\rightarrow$ pull on peer | Basic Sync | `STORY-MN-001` |
| **S2** | Cross-node `aida comment add` round-trip & `AIDA_USER` author stamp | Provenance | `BUG-MN-006` |
| **S3** | Concurrent disjoint spec edits & `oplog.yaml` auto-merge under `GIT_EDITOR=false` | Disjoint Merge | `BUG-MN-001`, `BUG-MN-003` |
| **S4a** | Concurrent same-spec scalar edits resolved on `aida db sync --push` | Conflict Resolution | `BUG-MN-002`, `BUG-MN-004` |
| **S4b** | Concurrent same-spec scalar edits resolved on `aida db sync --pull` | Conflict Resolution | `BUG-MN-002`, `BUG-MN-003`, `BUG-MN-004` |
| **S5** | Offline node-local ID allocation (5 specs on A, 5 on B) & online convergence | ID Allocation | `STORY-MN-001` |
| **S6** | SQLite `cache.db` freshness immediately after `aida db sync` & `aida pull` | Cache Coherency | `BUG-MN-005` |
| **S7** | Cross-node mailbox messaging (`--to <user>` vs `--broadcast`) | Mailbox Sync | `BUG-MN-007` |
| **S8** | Multi-hub GitLab mirror setup (`aida remote mirror gitlab`) & store fan-out | Multi-Hub (GitLab) | `STORY-MN-003`, `BUG-MN-008` |
| **S9** | Multi-hub drift detection (`aida remote status`) & catch-up (`aida remote mirror-sync`) | Multi-Hub (GitLab) | `STORY-MN-003` |
| **S10** | Restricted work-clone routing (`aida remote push-hubs gitlab`) | Multi-Hub (GitLab) | `STORY-MN-003` |
| **S11** | Diverged multi-hub CRDT union-merge (`aida remote reconcile --execute`) | Multi-Hub (GitLab) | `STORY-MN-003` |
| **S12** | GitLab SSH push-to-create vs `ls-remote` probe & expired `glab` token error reporting | Multi-Hub (GitLab) | `BUG-MN-008` |

---

## Quick Start

### 1. View Tracked Multi-Node Requirements
```bash
aida list
aida show EPIC-MN-001
```

### 2. Run the `aida-store-sync` Unit & Multi-Clone/Multi-Hub Git Tests (8 tests)
```bash
cargo test
```

### 3. Run the End-to-End Multi-Node & Multi-Hub CLI Test Suite (`Setup` + `S1`–`S11`)
```bash
# Diagnostic baseline mode (completes full suite and flags known issues):
./harness/run_multinode_suite.sh

# Strict mode (exits non-zero if any of BUG-MN-001..BUG-MN-008 reproduce):
./harness/run_multinode_suite.sh --strict

# Test against a custom aida binary:
./harness/run_multinode_suite.sh --aida-bin /path/to/aida
```
