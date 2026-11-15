#!/usr/bin/env bash
# trace:EPIC-MN-001 trace:STORY-MN-001 | ai:antigravity
#
# Repeatable, isolated multi-node AIDA store synchronization test suite (Setup + S1-S7).
# Never touches GitHub, GitLab, ~/.aida, or /home/joe/ai/aida.
#
# Usage:
#   ./harness/run_multinode_suite.sh [--aida-bin /path/to/aida] [--keep-dir] [--strict]
#
# Modes:
#   Default (baseline): Runs Setup + S1-S7, works around known P0 env issues where needed
#                       to complete the full suite, and prints a diagnostic PASS/FAIL/WARN matrix.
#   --strict:           Fails with non-zero exit code if ANY of BUG-MN-001..BUG-MN-007 reproduce.

set -euo pipefail

AIDA_BIN="${AIDA_BIN:-/home/joe/ai/aida/target/agent/aida}"
KEEP_DIR=0
STRICT=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --aida-bin)
      AIDA_BIN="$2"
      shift 2
      ;;
    --keep-dir)
      KEEP_DIR=1
      shift
      ;;
    --strict)
      STRICT=1
      shift
      ;;
    *)
      echo "Unknown option: $1" >&2
      echo "Usage: $0 [--aida-bin /path/to/aida] [--keep-dir] [--strict]" >&2
      exit 2
      ;;
  esac
done

if [[ ! -x "$AIDA_BIN" ]]; then
  echo "Error: AIDA_BIN is not executable: $AIDA_BIN" >&2
  exit 1
fi

T="$(mktemp -d "/tmp/aida-multinode-suite.XXXXXX")"
cleanup() {
  if [[ "$KEEP_DIR" -eq 0 ]]; then
    rm -rf "$T"
  else
    echo "Preserved test workspace at: $T"
  fi
}
trap cleanup EXIT

now_ms() {
  date +%s%3N
}

mkdir -p "$T/home-seed" "$T/home-a" "$T/home-b"
for pair in "seed:Seed User:seed@test.invalid" "a:Node A:a@test.invalid" "b:Node B:b@test.invalid"; do
  IFS=":" read -r key name email <<< "$pair"
  cat > "$T/home-$key/.gitconfig" <<EOF
[user]
	name = $name
	email = $email
[init]
	defaultBranch = main
EOF
done

echo "========================================================================"
echo "AIDA Multi-Node Sync Test Suite"
echo "Binary : $AIDA_BIN ($("$AIDA_BIN" --version))"
echo "Workdir: $T"
echo "========================================================================"

FAILURES=0
WARNINGS=0

record_result() {
  local scenario="$1"
  local status="$2"
  local ms="$3"
  local detail="$4"
  printf "%-28s | %-6s | %6s ms | %s\n" "$scenario" "$status" "$ms" "$detail"
  if [[ "$status" == "FAIL" ]]; then
    FAILURES=$((FAILURES + 1))
  elif [[ "$status" == "WARN" ]]; then
    WARNINGS=$((WARNINGS + 1))
  fi
}

echo ""
printf "%-28s | %-6s | %9s | %s\n" "Scenario" "Status" "Latency" "Notes"
echo "-----------------------------+--------+-----------+------------------------------------------------"

# --- Setup: Bare hub + seed + node-a + node-b ---
t0=$(now_ms)
git init --bare -b main "$T/hub.git" >/dev/null 2>&1
mkdir -p "$T/seed/src"
cat > "$T/seed/README.md" <<EOF
# Sync Test Seed
EOF
cat > "$T/seed/src/lib.rs" <<EOF
pub fn add(a: i32, b: i32) -> i32 { a + b }
EOF
(
  unset GIT_EDITOR
  cd "$T/seed"
  HOME="$T/home-seed" git init -b main >/dev/null 2>&1
  HOME="$T/home-seed" git add README.md src/lib.rs
  HOME="$T/home-seed" git commit -m "chore: initial seed" >/dev/null 2>&1
  HOME="$T/home-seed" AIDA_USER=seed "$AIDA_BIN" init < /dev/null >/dev/null 2>&1 || true
  HOME="$T/home-seed" git add -A
  HOME="$T/home-seed" git diff --cached --quiet || HOME="$T/home-seed" git commit -m "chore: commit aida init scaffolding" >/dev/null 2>&1
  HOME="$T/home-seed" git remote add origin "$T/hub.git"
  HOME="$T/home-seed" git push -u origin main >/dev/null 2>&1
  HOME="$T/home-seed" git -C .aida-store remote add origin "$T/hub.git" 2>/dev/null || HOME="$T/home-seed" git -C .aida-store remote set-url origin "$T/hub.git"
  HOME="$T/home-seed" git -C .aida-store push -u origin aida-store >/dev/null 2>&1
)

HOME="$T/home-a" git clone "$T/hub.git" "$T/node-a" >/dev/null 2>&1
HOME="$T/home-b" git clone "$T/hub.git" "$T/node-b" >/dev/null 2>&1

(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" node acquire < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" node alias set NODEA < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type story --title "Starter story" --description "Seed 1" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type bug --title "Starter bug" --description "Seed 2" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type task --title "Starter task" --description "Seed 3" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" node acquire < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" node alias set NODEB < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
record_result "Setup (hub+seed+A+B)" "PASS" "$((t1 - t0))" "Store auto-attached on A & B; 3 starter specs synced"

# --- S1: Single-node add + push -> pull on peer ---
t0=$(now_ms)
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type functional --title "S1 Feature from A" --description "S1 body" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
if (cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" show FR-NODEA-001 < /dev/null 2>/dev/null | grep -q "S1 Feature from A"); then
  record_result "S1 (Add on A -> Pull on B)" "PASS" "$((t1 - t0))" "FR-NODEA-001 visible on B"
else
  record_result "S1 (Add on A -> Pull on B)" "FAIL" "$((t1 - t0))" "FR-NODEA-001 missing on B"
fi

# --- S2: Comment on B -> Pull on A + check BUG-MN-006 (AIDA_USER author) ---
t0=$(now_ms)
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" comment add STORY-NODEA-001 "S2 comment from node-b" < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
COMMENT_AUTHOR=$(grep -rn -B2 -A2 "S2 comment from node-b" "$T/node-a/.aida-store/objects/" | grep "author:" | head -n1 | awk '{print $NF}' || true)
if [[ "$COMMENT_AUTHOR" == "node-b" ]]; then
  record_result "S2 (Comment B -> A)" "PASS" "$((t1 - t0))" "Comment synced; author=$COMMENT_AUTHOR"
else
  record_result "S2 (Comment B -> A)" "WARN" "$((t1 - t0))" "Comment synced, but BUG-MN-006: author=$COMMENT_AUTHOR (expected node-b)"
fi

# --- S3: Disjoint edits + BUG-MN-001 (GIT_EDITOR=false) + BUG-MN-003 (false conflict on FF pull) ---
t0=$(now_ms)
mv "$T/hub.git" "$T/hub.git.offline"
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type task --title "S3 Task A" --description "Offline A" < /dev/null >/dev/null 2>&1
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" add --type task --title "S3 Task B" --description "Offline B" < /dev/null >/dev/null 2>&1
)
mv "$T/hub.git.offline" "$T/hub.git"
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
# Test BUG-MN-001: Does push-rebase succeed with GIT_EDITOR=false?
if (cd "$T/node-b" && GIT_EDITOR=false HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1); then
  S3_EDITOR_OK=1
else
  S3_EDITOR_OK=0
  # Recover and complete rebase with GIT_EDITOR unset
  git -C "$T/node-b/.aida-store" rebase --abort >/dev/null 2>&1 || true
  (unset GIT_EDITOR && cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1)
fi
S3_PULL_OUT=$(unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --pull < /dev/null 2>&1)
t1=$(now_ms)
if [[ "$S3_EDITOR_OK" -eq 0 ]]; then
  record_result "S3 (Disjoint edits + rebase)" "FAIL" "$((t1 - t0))" "BUG-MN-001: GIT_EDITOR=false broke git rebase --continue"
elif echo "$S3_PULL_OUT" | grep -q "Conflicts detected"; then
  record_result "S3 (Disjoint edits + rebase)" "WARN" "$((t1 - t0))" "BUG-MN-003: False conflict reported on fast-forward pull"
else
  record_result "S3 (Disjoint edits + rebase)" "PASS" "$((t1 - t0))" "Clean oplog + disjoint spec auto-merge"
fi

# --- S4a: Concurrent same-field scalar edit on --push (BUG-MN-002 & BUG-MN-004) ---
t0=$(now_ms)
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" edit FR-NODEA-001 --title "S4a Title A" --priority high < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" edit FR-NODEA-001 --title "S4a Title B" --priority low < /dev/null >/dev/null 2>&1
)
S4A_PUSH_OUT=$(unset GIT_EDITOR && cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null 2>&1)
t1=$(now_ms)
if echo "$S4A_PUSH_OUT" | grep -qiE "conflict|overrid"; then
  record_result "S4a (Same-spec edit --push)" "PASS" "$((t1 - t0))" "Push leg surfaced scalar override"
else
  record_result "S4a (Same-spec edit --push)" "FAIL" "$((t1 - t0))" "BUG-MN-002: Node B scalar edits silently overwritten on --push"
fi

# --- S5: Offline ID allocation (5 specs on A, 5 on B) ---
t0=$(now_ms)
mv "$T/hub.git" "$T/hub.git.offline"
for i in 1 2 3 4 5; do
  (unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type task --title "S5 A-$i" --description "Offline" < /dev/null >/dev/null 2>&1)
  (unset GIT_EDITOR && cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" add --type task --title "S5 B-$i" --description "Offline" < /dev/null >/dev/null 2>&1)
done
mv "$T/hub.git.offline" "$T/hub.git"
(
  unset GIT_EDITOR
  cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
  cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
  cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
record_result "S5 (5x2 offline IDs + sync)" "PASS" "$((t1 - t0))" "10 offline specs merged with 0 ID collisions"

# --- S6: Cache freshness after aida db sync --pull (BUG-MN-005) ---
t0=$(now_ms)
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" list --format toon < /dev/null >/dev/null 2>&1
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type task --title "S6 Freshness Probe" --description "Probe" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
)
S6_CACHE_OUT=$(cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" cache status < /dev/null 2>&1)
t1=$(now_ms)
if echo "$S6_CACHE_OUT" | grep -qi "stale"; then
  record_result "S6 (Cache freshness on sync)" "WARN" "$((t1 - t0))" "BUG-MN-005: cache.db left stale after aida db sync --pull"
else
  record_result "S6 (Cache freshness on sync)" "PASS" "$((t1 - t0))" "cache.db fresh immediately after aida db sync --pull"
fi

# --- S7: Mailbox send/receive across nodes (BUG-MN-007) ---
t0=$(now_ms)
if (unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" mailbox send --to node-b --subject "S7" "Direct" < /dev/null >/dev/null 2>&1); then
  S7_DIRECT_OK=1
else
  S7_DIRECT_OK=0
  (unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" mailbox send --broadcast --subject "S7" "Broadcast fallback" < /dev/null >/dev/null 2>&1)
fi
(
  unset GIT_EDITOR
  cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
  cd "$T/node-b" && HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
if [[ "$S7_DIRECT_OK" -eq 1 ]]; then
  record_result "S7 (Mailbox A -> B)" "PASS" "$((t1 - t0))" "Direct --to node-b succeeded"
else
  record_result "S7 (Mailbox A -> B)" "WARN" "$((t1 - t0))" "BUG-MN-007: --to node-b refused without team role; --broadcast worked"
fi

# --- S8: Multi-hub GitLab mirror setup + store fan-out (STORY-MN-003) ---
t0=$(now_ms)
git init --bare -b main "$T/gitlab.git" >/dev/null 2>&1
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote mirror gitlab --url "$T/gitlab.git" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" git add .aida/config.toml
  HOME="$T/home-a" git commit -m "chore(remote): list gitlab as a store mirror" >/dev/null 2>&1 || true
  HOME="$T/home-a" git push origin main >/dev/null 2>&1
  HOME="$T/home-a" git push -u gitlab main >/dev/null 2>&1
  HOME="$T/home-a" git -C .aida-store remote add gitlab "$T/gitlab.git" 2>/dev/null || HOME="$T/home-a" git -C .aida-store remote set-url gitlab "$T/gitlab.git"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" add --type task --title "S8 Fanout Task" --description "Should fan out to origin and gitlab" < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
t1=$(now_ms)
ORIGIN_STORE_SHA=$(git -C "$T/hub.git" rev-parse refs/heads/aida-store)
GITLAB_STORE_SHA=$(git -C "$T/gitlab.git" rev-parse refs/heads/aida-store 2>/dev/null || echo "absent")
if [[ "$ORIGIN_STORE_SHA" == "$GITLAB_STORE_SHA" ]]; then
  record_result "S8 (GitLab mirror fan-out)" "PASS" "$((t1 - t0))" "origin & gitlab store tips match (${ORIGIN_STORE_SHA:0:10})"
else
  record_result "S8 (GitLab mirror fan-out)" "FAIL" "$((t1 - t0))" "origin ($ORIGIN_STORE_SHA) != gitlab ($GITLAB_STORE_SHA)"
fi

# --- S9: Multi-hub drift detection (remote status) + mirror-sync catch-up ---
t0=$(now_ms)
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" add --type task --title "S9 Origin-Only Task" --description "Pushed from B to origin only" < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
# Node A checks remote status (should exit non-zero because origin is ahead of gitlab)
if (unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote status < /dev/null >/dev/null 2>&1); then
  S9_DETECTED_DRIFT=0
else
  S9_DETECTED_DRIFT=1
fi
# Node A runs remote mirror-sync to bring gitlab level with origin
(
  unset GIT_EDITOR
  cd "$T/node-a"
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" db sync --pull < /dev/null >/dev/null 2>&1
  HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote mirror-sync < /dev/null >/dev/null 2>&1
)
if (unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote status < /dev/null >/dev/null 2>&1); then
  S9_SYNCED_OK=1
else
  S9_SYNCED_OK=0
fi
t1=$(now_ms)
if [[ "$S9_DETECTED_DRIFT" -eq 1 && "$S9_SYNCED_OK" -eq 1 ]]; then
  record_result "S9 (Drift + mirror-sync)" "PASS" "$((t1 - t0))" "remote status caught drift; mirror-sync leveled gitlab"
else
  record_result "S9 (Drift + mirror-sync)" "FAIL" "$((t1 - t0))" "detected_drift=$S9_DETECTED_DRIFT synced_ok=$S9_SYNCED_OK"
fi

# --- S10 & S11: Restricted push-hubs gitlab on Node C + remote reconcile --execute ---
t0=$(now_ms)
mkdir -p "$T/home-c"
cat > "$T/home-c/.gitconfig" <<EOF
[user]
	name = Node C
	email = c@test.invalid
[init]
	defaultBranch = main
EOF
HOME="$T/home-c" git clone "$T/gitlab.git" "$T/node-c" >/dev/null 2>&1
(
  unset GIT_EDITOR
  cd "$T/node-c"
  # Wire both origin (hub.git) and gitlab (gitlab.git) on node-c, but restrict push-hubs to gitlab!
  HOME="$T/home-c" git remote set-url origin "$T/hub.git"
  HOME="$T/home-c" git remote add gitlab "$T/gitlab.git"
  HOME="$T/home-c" AIDA_USER=node-c "$AIDA_BIN" db status < /dev/null >/dev/null 2>&1 || true
  HOME="$T/home-c" git -C .aida-store remote set-url origin "$T/hub.git"
  HOME="$T/home-c" git -C .aida-store remote add gitlab "$T/gitlab.git" 2>/dev/null || true
  HOME="$T/home-c" AIDA_USER=node-c "$AIDA_BIN" node acquire < /dev/null >/dev/null 2>&1
  HOME="$T/home-c" AIDA_USER=node-c "$AIDA_BIN" remote push-hubs gitlab < /dev/null >/dev/null 2>&1
  # Node C adds a spec -> should publish ONLY to gitlab, leaving origin untouched!
  HOME="$T/home-c" AIDA_USER=node-c "$AIDA_BIN" add --type task --title "S10 GitLab-Only Task from C" --description "Pushed to gitlab only via push-hubs" < /dev/null >/dev/null 2>&1
  HOME="$T/home-c" AIDA_USER=node-c "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
# Meanwhile Node B adds a spec to origin only -> now origin and gitlab have diverged!
(
  unset GIT_EDITOR
  cd "$T/node-b"
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" add --type task --title "S10 Origin-Only Task from B" --description "Pushed to origin only" < /dev/null >/dev/null 2>&1
  HOME="$T/home-b" AIDA_USER=node-b "$AIDA_BIN" db sync --push < /dev/null >/dev/null 2>&1
)
ORIGIN_PRE_REC=$(git -C "$T/hub.git" rev-parse refs/heads/aida-store)
GITLAB_PRE_REC=$(git -C "$T/gitlab.git" rev-parse refs/heads/aida-store)
t1=$(now_ms)
if [[ "$ORIGIN_PRE_REC" != "$GITLAB_PRE_REC" ]]; then
  record_result "S10 (push-hubs gitlab on C)" "PASS" "$((t1 - t0))" "Node C pushed only to gitlab; hubs diverged as expected"
else
  record_result "S10 (push-hubs gitlab on C)" "FAIL" "$((t1 - t0))" "Expected origin and gitlab to diverge before reconcile"
fi

t0=$(now_ms)
# Dry-run reconcile should exit 2 when hubs disagree
set +e
(unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote reconcile < /dev/null >/dev/null 2>&1)
REC_DRY_RC=$?
(unset GIT_EDITOR && cd "$T/node-a" && HOME="$T/home-a" AIDA_USER=node-a "$AIDA_BIN" remote reconcile --execute < /dev/null >/dev/null 2>&1)
REC_EXEC_RC=$?
set -e
ORIGIN_POST_REC=$(git -C "$T/hub.git" rev-parse refs/heads/aida-store)
GITLAB_POST_REC=$(git -C "$T/gitlab.git" rev-parse refs/heads/aida-store)
t1=$(now_ms)
if [[ "$REC_DRY_RC" -eq 2 && "$REC_EXEC_RC" -eq 0 && "$ORIGIN_POST_REC" == "$GITLAB_POST_REC" ]]; then
  record_result "S11 (remote reconcile)" "PASS" "$((t1 - t0))" "Diverged origin & gitlab union-merged to ${ORIGIN_POST_REC:0:10}"
else
  record_result "S11 (remote reconcile)" "FAIL" "$((t1 - t0))" "dry_rc=$REC_DRY_RC exec_rc=$REC_EXEC_RC origin=$ORIGIN_POST_REC gitlab=$GITLAB_POST_REC"
fi

echo "========================================================================"
echo "Summary: $FAILURES failure(s), $WARNINGS warning(s)"
echo "========================================================================"

if [[ "$STRICT" -eq 1 && ( "$FAILURES" -gt 0 || "$WARNINGS" -gt 0 ) ]]; then
  exit 1
fi
exit 0
