# aida-multinode

Dedicated testbed, behavioral specification, root-cause repository, and standalone Rust crate prototype (`aida-store-sync`) for [AIDA](https://github.com/joemooney/aida) multi-node synchronization.

---

## Repository Contents

| Path | Description |
| :--- | :--- |
| [`crates/aida-store-sync/`](crates/aida-store-sync/) | Standalone, zero-dependency Rust crate (`aida-store-sync`) factoring out 3-way per-field spec merging (`merge_spec_upstream_with_report`), base-aware conflict detection (`detect_conflict_with_base`), hermetic Git rebase auto-merging (`rebase_auto_merge_onto`), and consistent author provenance (`resolve_default_author`), with unit and multi-clone Git rebase integration tests. |
| [`harness/run_multinode_suite.sh`](harness/run_multinode_suite.sh) | Automated, repeatable S1–S7 multi-node synchronization test harness. Creates an isolated bare hub (`hub.git`), seed repo (`seed`), and two peer clones (`node-a`, `node-b`) with separate `$HOME` and `AIDA_USER` identities, executing all 7 scenarios and printing a pass/fail/warn + latency matrix. |
| [`docs/REPORT-2026-11-15.md`](docs/REPORT-2026-11-15.md) | Full baseline multi-node test report (`aida 0.16.0`, `0dacba5a25e`), scenario-by-scenario transcripts and timings, code-level root causes in `~/ai/aida`, proposed fixes, and prioritized implementation plan. |
| [`docs/MULTINODE-CONTRACT.md`](docs/MULTINODE-CONTRACT.md) | Canonical behavioral contract and invariants for AIDA multi-node synchronization (clone auto-attachment, `AIDA_USER` provenance, 3-way scalar & set merges, fast-forward conflict suppression, and cache freshness). |
| [`docs/ARCHITECTURE-EXTRACTION.md`](docs/ARCHITECTURE-EXTRACTION.md) | Architecture blueprint mapping `crates/aida-store-sync` back to `aida-core` and `aida-cli-lib`. |
| [`.aida-store/`](.aida-store/) | AIDA git-canonical store tracking `EPIC-MN-001`, `BUG-MN-001` through `BUG-MN-007`, and `STORY-MN-001` through `STORY-MN-002` (under friendly node alias `MN`). |

---

## Quick Start

### 1. View Tracked Multi-Node Requirements
```bash
aida list
aida show EPIC-MN-001
```

### 2. Run the `aida-store-sync` Unit & Multi-Clone Git Rebase Tests
```bash
cargo test
```

### 3. Run the End-to-End Multi-Node CLI Test Suite (Setup + S1–S7)
```bash
# Diagnostic baseline mode (completes full suite and flags known issues):
./harness/run_multinode_suite.sh

# Strict mode (exits non-zero if any of BUG-MN-001..BUG-MN-007 reproduce):
./harness/run_multinode_suite.sh --strict

# Test against a custom aida binary:
./harness/run_multinode_suite.sh --aida-bin /path/to/aida
```
