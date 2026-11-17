//! Standalone prototype and reference implementation for AIDA multi-node store
//! synchronization (`aida-store-sync`).
//!
//! This crate factors out the multi-node store synchronization primitives from
//! `aida-core` and `aida-cli-lib`, demonstrating concrete fixes and regression
//! tests for every issue uncovered in the multi-node sync test suite:
//!
//! - `BUG-MN-001`: Hermetic `git rebase --continue` execution that overrides
//!   `GIT_EDITOR=false` in non-interactive agent harness environments.
//! - `BUG-MN-002`: True 3-way per-field scalar merge in `merge_spec_upstream`
//!   (preserving non-colliding local scalar edits while keeping `upstream` as
//!   winner on same-field collisions) + `ScalarOverrideReport` so `db sync --push`
//!   never silently drops scalar edits.
//! - `BUG-MN-003`: Ancestor/base-aware conflict detection (`detect_conflict_with_base`)
//!   that eliminates false-positive conflict alerts on clean fast-forward pulls,
//!   plus deduplicated `auto-merged` note formatting.
//! - `BUG-MN-004`: Field-level `FieldChange` history builder (`record_spec_edit`)
//!   for git-canonical `aida edit` so scalar and tag edits never fall back to
//!   empty `changes: []` write events.
//! - `BUG-MN-006`: Consistent author resolution (`resolve_default_author`) that
//!   checks `AIDA_AUTHOR` -> `AIDA_USER` -> `USER` -> `USERNAME` -> `"Unknown"`.
//!
//! // trace:EPIC-MN-001 trace:STORY-MN-002 | ai:antigravity

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

// ============================================================================
// 1. Sync-Relevant Domain Model
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field_name: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    pub author: String,
    pub timestamp: u64,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub created_at: u64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRecord {
    pub spec_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub owner: String,
    pub feature: String,
    pub modified_at: u64,
    pub tags: BTreeSet<String>,
    pub history: Vec<HistoryEntry>,
    pub comments: Vec<Comment>,
}

impl SpecRecord {
    pub fn new(spec_id: &str, title: &str, description: &str) -> Self {
        Self {
            spec_id: spec_id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: "Draft".to_string(),
            priority: "Medium".to_string(),
            owner: String::new(),
            feature: String::new(),
            modified_at: 1,
            tags: BTreeSet::new(),
            history: Vec::new(),
            comments: Vec::new(),
        }
    }
}

// ============================================================================
// 2. Author Provenance & Field-Change Recorder (BUG-MN-004, BUG-MN-006)
// ============================================================================

/// Resolves the author identity from an environment lookup function.
///
/// Precedence (`BUG-MN-006`):
/// 1. `AIDA_AUTHOR` (explicit agent/harness stamp, e.g. `ai:codex:joe`)
/// 2. `AIDA_USER` (multi-node operator identity from `docs/advanced/multi-node.md`)
/// 3. `USER` (POSIX login user)
/// 4. `USERNAME` (Windows fallback)
/// 5. `"Unknown"`
// trace:BUG-MN-006 | ai:antigravity
pub fn resolve_default_author_with<F>(mut get_env: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    for key in ["AIDA_AUTHOR", "AIDA_USER", "USER", "USERNAME"] {
        if let Some(val) = get_env(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "Unknown".to_string()
}

/// Resolves the default author from `std::env::var`.
// trace:BUG-MN-006 | ai:antigravity
pub fn resolve_default_author() -> String {
    resolve_default_author_with(|k| std::env::var(k).ok())
}

#[derive(Debug, Default, Clone)]
pub struct SpecEditPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub owner: Option<String>,
    pub feature: Option<String>,
    pub add_tags: BTreeSet<String>,
    pub remove_tags: BTreeSet<String>,
}

/// Applies a CLI edit patch to `spec` and records explicit `FieldChange` items
/// for every modified scalar or tag field (`BUG-MN-004`), preventing git-canonical
/// `aida edit` from falling back to an empty `changes: []` write event.
// trace:BUG-MN-004 | ai:antigravity
pub fn apply_spec_edit_recorded(
    spec: &mut SpecRecord,
    patch: &SpecEditPatch,
    entry_id: &str,
    author: &str,
    timestamp: u64,
) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    if let Some(new_title) = &patch.title {
        if new_title != &spec.title {
            changes.push(FieldChange {
                field_name: "title".to_string(),
                old_value: spec.title.clone(),
                new_value: new_title.clone(),
            });
            spec.title = new_title.clone();
        }
    }
    if let Some(new_desc) = &patch.description {
        if new_desc != &spec.description {
            changes.push(FieldChange {
                field_name: "description".to_string(),
                old_value: spec.description.clone(),
                new_value: new_desc.clone(),
            });
            spec.description = new_desc.clone();
        }
    }
    if let Some(new_status) = &patch.status {
        if new_status != &spec.status {
            changes.push(FieldChange {
                field_name: "status".to_string(),
                old_value: spec.status.clone(),
                new_value: new_status.clone(),
            });
            spec.status = new_status.clone();
        }
    }
    if let Some(new_priority) = &patch.priority {
        if new_priority != &spec.priority {
            changes.push(FieldChange {
                field_name: "priority".to_string(),
                old_value: spec.priority.clone(),
                new_value: new_priority.clone(),
            });
            spec.priority = new_priority.clone();
        }
    }
    if let Some(new_owner) = &patch.owner {
        if new_owner != &spec.owner {
            changes.push(FieldChange {
                field_name: "owner".to_string(),
                old_value: spec.owner.clone(),
                new_value: new_owner.clone(),
            });
            spec.owner = new_owner.clone();
        }
    }
    if let Some(new_feature) = &patch.feature {
        if new_feature != &spec.feature {
            changes.push(FieldChange {
                field_name: "feature".to_string(),
                old_value: spec.feature.clone(),
                new_value: new_feature.clone(),
            });
            spec.feature = new_feature.clone();
        }
    }

    if !patch.add_tags.is_empty() || !patch.remove_tags.is_empty() {
        let mut next_tags = spec.tags.clone();
        for t in &patch.add_tags {
            next_tags.insert(t.clone());
        }
        for t in &patch.remove_tags {
            next_tags.remove(t);
        }
        if next_tags != spec.tags {
            let old_str = spec.tags.iter().cloned().collect::<Vec<_>>().join(", ");
            let new_str = next_tags.iter().cloned().collect::<Vec<_>>().join(", ");
            changes.push(FieldChange {
                field_name: "tags".to_string(),
                old_value: old_str,
                new_value: new_str,
            });
            spec.tags = next_tags;
        }
    }

    if !changes.is_empty() {
        spec.modified_at = timestamp;
        spec.history.push(HistoryEntry {
            id: entry_id.to_string(),
            author: author.to_string(),
            timestamp,
            changes: changes.clone(),
        });
    }

    changes
}

// ============================================================================
// 3. Three-Way Spec Merge & Conflict Detection (BUG-MN-002, BUG-MN-003)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarOverride {
    pub field_name: String,
    pub base_value: String,
    pub local_value: String,
    pub winning_upstream_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecMergeOutcome {
    pub merged: SpecRecord,
    /// Scalar fields where `local` modified `base`, `upstream` also modified `base`
    /// to a different value, and `upstream` won (`BUG-MN-002`).
    pub overridden_scalars: Vec<ScalarOverride>,
}

impl SpecMergeOutcome {
    /// Formats the single-line auto-merge summary WITHOUT a duplicate `"auto-merged "`
    /// prefix (`BUG-MN-003`), including any overridden scalar fields (`BUG-MN-002`).
    // trace:BUG-MN-002 trace:BUG-MN-003 | ai:antigravity
    pub fn summary_note(&self) -> String {
        let base_note = format!(
            "{}: unioned {} history entries, status upstream={}",
            self.merged.spec_id,
            self.merged.history.len(),
            self.merged.status
        );
        if self.overridden_scalars.is_empty() {
            base_note
        } else {
            let fields: Vec<&str> = self
                .overridden_scalars
                .iter()
                .map(|o| o.field_name.as_str())
                .collect();
            format!(
                "{base_note} (upstream overrode colliding local edits on: {})",
                fields.join(", ")
            )
        }
    }
}

/// 3-way merge of a single scalar field against `base`, recording when a colliding
/// `local` edit was overridden by `upstream` (`BUG-MN-002`).
// trace:BUG-MN-002 | ai:antigravity
fn merge_scalar_with_override(
    field_name: &str,
    base: &str,
    upstream: &str,
    local: &str,
    upstream_wins_ties: bool,
    overrides: &mut Vec<ScalarOverride>,
) -> String {
    if upstream == local {
        return upstream.to_string();
    }
    let upstream_changed = upstream != base;
    let local_changed = local != base;
    match (upstream_changed, local_changed) {
        // Only upstream changed it -> keep upstream
        (true, false) => upstream.to_string(),
        // Only local changed it -> preserve local's non-colliding edit! (Fixes BUG-MN-002)
        (false, true) => local.to_string(),
        // Both sides changed the same field to different values -> genuine collision
        (true, true) | (false, false) => {
            if upstream_wins_ties {
                overrides.push(ScalarOverride {
                    field_name: field_name.to_string(),
                    base_value: base.to_string(),
                    local_value: local.to_string(),
                    winning_upstream_value: upstream.to_string(),
                });
                upstream.to_string()
            } else {
                local.to_string()
            }
        }
    }
}

fn union_history(sources: &[&Vec<HistoryEntry>]) -> Vec<HistoryEntry> {
    let mut by_id: BTreeMap<String, HistoryEntry> = BTreeMap::new();
    for src in sources {
        for entry in src.iter() {
            by_id
                .entry(entry.id.clone())
                .or_insert_with(|| entry.clone());
        }
    }
    let mut out: Vec<_> = by_id.into_values().collect();
    out.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then(a.id.cmp(&b.id)));
    out
}

fn union_comments(sources: &[&Vec<Comment>]) -> Vec<Comment> {
    let mut by_id: BTreeMap<String, Comment> = BTreeMap::new();
    for src in sources {
        for c in src.iter() {
            by_id.entry(c.id.clone()).or_insert_with(|| c.clone());
        }
    }
    let mut out: Vec<_> = by_id.into_values().collect();
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    out
}

fn merge_tags_three_way(
    base: &BTreeSet<String>,
    ours: &BTreeSet<String>,
    theirs: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut result: BTreeSet<String> = ours.union(theirs).cloned().collect();
    for item in base {
        if !ours.contains(item) || !theirs.contains(item) {
            result.remove(item);
        }
    }
    result
}

/// Corrected `merge_spec_upstream` (`BUG-MN-002`):
/// - Non-colliding local scalar edits (where `upstream == base && local != base`)
///   are preserved instead of being clobbered by `upstream.clone()`.
/// - Same-field scalar collisions resolve to `upstream` AND are recorded in
///   `SpecMergeOutcome::overridden_scalars` so both `--push` and `--pull` can
///   warn the user.
// trace:BUG-MN-002 | ai:antigravity
pub fn merge_spec_upstream_with_report(
    base: &SpecRecord,
    upstream: &SpecRecord,
    local: &SpecRecord,
) -> SpecMergeOutcome {
    let mut overrides = Vec::new();
    let title = merge_scalar_with_override(
        "title",
        &base.title,
        &upstream.title,
        &local.title,
        true,
        &mut overrides,
    );
    let description = merge_scalar_with_override(
        "description",
        &base.description,
        &upstream.description,
        &local.description,
        true,
        &mut overrides,
    );
    let status = merge_scalar_with_override(
        "status",
        &base.status,
        &upstream.status,
        &local.status,
        true,
        &mut overrides,
    );
    let priority = merge_scalar_with_override(
        "priority",
        &base.priority,
        &upstream.priority,
        &local.priority,
        true,
        &mut overrides,
    );
    let owner = merge_scalar_with_override(
        "owner",
        &base.owner,
        &upstream.owner,
        &local.owner,
        true,
        &mut overrides,
    );
    let feature = merge_scalar_with_override(
        "feature",
        &base.feature,
        &upstream.feature,
        &local.feature,
        true,
        &mut overrides,
    );

    let merged = SpecRecord {
        spec_id: upstream.spec_id.clone(),
        title,
        description,
        status,
        priority,
        owner,
        feature,
        modified_at: upstream.modified_at.max(local.modified_at),
        tags: merge_tags_three_way(&base.tags, &upstream.tags, &local.tags),
        history: union_history(&[&base.history, &upstream.history, &local.history]),
        comments: union_comments(&[&base.comments, &upstream.comments, &local.comments]),
    };

    SpecMergeOutcome {
        merged,
        overridden_scalars: overrides,
    }
}

/// Base-aware conflict detector (`BUG-MN-003`).
///
/// Unlike `aida_core::conflict::detect_conflict(local, remote)` which lacked `base`
/// and reported a false conflict on every fast-forward pull where `remote.modified_at > local.modified_at`,
/// `detect_conflict_with_base` returns `Some` ONLY when both `local` and `remote`
/// diverged on at least one scalar field relative to `base`.
// trace:BUG-MN-003 | ai:antigravity
pub fn detect_conflict_with_base(
    base: &SpecRecord,
    local: &SpecRecord,
    remote: &SpecRecord,
) -> Option<Vec<ScalarOverride>> {
    // Fast-forward or no local changes relative to base -> never a conflict!
    if local == base || remote == base || local == remote {
        return None;
    }
    let outcome = merge_spec_upstream_with_report(base, remote, local);
    if outcome.overridden_scalars.is_empty() {
        None
    } else {
        Some(outcome.overridden_scalars)
    }
}

/// Deduplicates the `"auto-merged"` prefix when formatting pull/sync notes (`BUG-MN-003`).
// trace:BUG-MN-003 | ai:antigravity
pub fn format_auto_merged_line(note: &str) -> String {
    let body = note.strip_prefix("auto-merged ").unwrap_or(note);
    format!("auto-merged {body}")
}

// ============================================================================
// 4. Hermetic Git Rebase Auto-Merge Engine (BUG-MN-001)
// ============================================================================

/// Simple deterministic wire format for `SpecRecord` in Git rebase integration tests.
pub fn serialize_spec(spec: &SpecRecord) -> String {
    let mut lines = Vec::new();
    lines.push(format!("spec_id: {}", spec.spec_id));
    lines.push(format!("title: {}", spec.title));
    lines.push(format!("description: {}", spec.description));
    lines.push(format!("status: {}", spec.status));
    lines.push(format!("priority: {}", spec.priority));
    lines.push(format!("owner: {}", spec.owner));
    lines.push(format!("feature: {}", spec.feature));
    lines.push(format!("modified_at: {}", spec.modified_at));
    let tags: Vec<_> = spec.tags.iter().cloned().collect();
    lines.push(format!("tags: {}", tags.join(",")));
    for h in &spec.history {
        let changes_str: Vec<String> = h
            .changes
            .iter()
            .map(|c| format!("{}={}->{}", c.field_name, c.old_value, c.new_value))
            .collect();
        lines.push(format!(
            "history: {}|{}|{}|{}",
            h.id,
            h.author,
            h.timestamp,
            changes_str.join(";")
        ));
    }
    for c in &spec.comments {
        lines.push(format!(
            "comment: {}|{}|{}|{}",
            c.id, c.author, c.created_at, c.content
        ));
    }
    lines.join("\n") + "\n"
}

pub fn deserialize_spec(raw: &str) -> Result<SpecRecord, String> {
    let mut spec = SpecRecord::new("", "", "");
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(": ") else {
            let Some((k_empty, _)) = line.split_once(':') else {
                return Err(format!("invalid line: {line}"));
            };
            match k_empty.trim() {
                "owner" => spec.owner = String::new(),
                "feature" => spec.feature = String::new(),
                "tags" => spec.tags.clear(),
                _ => {}
            }
            continue;
        };
        match k {
            "spec_id" => spec.spec_id = v.to_string(),
            "title" => spec.title = v.to_string(),
            "description" => spec.description = v.to_string(),
            "status" => spec.status = v.to_string(),
            "priority" => spec.priority = v.to_string(),
            "owner" => spec.owner = v.to_string(),
            "feature" => spec.feature = v.to_string(),
            "modified_at" => {
                spec.modified_at = v.parse::<u64>().map_err(|e| e.to_string())?;
            }
            "tags" => {
                spec.tags = v
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "history" => {
                let parts: Vec<&str> = v.splitn(4, '|').collect();
                if parts.len() != 4 {
                    return Err(format!("invalid history line: {v}"));
                }
                let mut changes = Vec::new();
                if !parts[3].is_empty() {
                    for item in parts[3].split(';') {
                        if let Some((f, rest)) = item.split_once('=') {
                            if let Some((old_v, new_v)) = rest.split_once("->") {
                                changes.push(FieldChange {
                                    field_name: f.to_string(),
                                    old_value: old_v.to_string(),
                                    new_value: new_v.to_string(),
                                });
                            }
                        }
                    }
                }
                spec.history.push(HistoryEntry {
                    id: parts[0].to_string(),
                    author: parts[1].to_string(),
                    timestamp: parts[2].parse::<u64>().map_err(|e| e.to_string())?,
                    changes,
                });
            }
            "comment" => {
                let parts: Vec<&str> = v.splitn(4, '|').collect();
                if parts.len() != 4 {
                    return Err(format!("invalid comment line: {v}"));
                }
                spec.comments.push(Comment {
                    id: parts[0].to_string(),
                    author: parts[1].to_string(),
                    created_at: parts[2].parse::<u64>().map_err(|e| e.to_string())?,
                    content: parts[3].to_string(),
                });
            }
            _ => {}
        }
    }
    Ok(spec)
}

/// Spawns a Git command with `GIT_EDITOR=true` and `GIT_TERMINAL_PROMPT=0`
/// enforced in the child environment (`BUG-MN-001`), preventing `GIT_EDITOR=false`
/// in agent harnesses from breaking `git rebase --continue`.
// trace:BUG-MN-001 | ai:antigravity
pub fn run_git_hermetic(repo: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseSyncOutcome {
    pub notes: Vec<String>,
    pub overridden_scalars: Vec<(String, Vec<ScalarOverride>)>,
}

/// Rebases `repo` onto `upstream_ref`, automatically resolving conflicts on
/// `objects/*.yaml` via `merge_spec_upstream_with_report` and on `oplog.yaml`
/// via line-union, and continuing the rebase hermetically even when the parent
/// process has `GIT_EDITOR=false` (`BUG-MN-001`, `BUG-MN-002`, `BUG-MN-003`).
// trace:BUG-MN-001 trace:BUG-MN-002 trace:BUG-MN-003 | ai:antigravity
pub fn rebase_auto_merge_onto(
    repo: &Path,
    upstream_ref: &str,
) -> Result<RebaseSyncOutcome, String> {
    let out = run_git_hermetic(repo, &["rebase", upstream_ref])?;
    if out.status.success() {
        return Ok(RebaseSyncOutcome {
            notes: Vec::new(),
            overridden_scalars: Vec::new(),
        });
    }

    let mut notes = Vec::new();
    let mut all_overrides = Vec::new();

    for _step in 0..32 {
        let unmerged_out = run_git_hermetic(repo, &["diff", "--name-only", "--diff-filter=U"])?;
        let unmerged: Vec<String> = String::from_utf8_lossy(&unmerged_out.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        if unmerged.is_empty() {
            break;
        }

        for path in &unmerged {
            if path == "oplog.yaml" {
                let ours = run_git_hermetic(repo, &["show", ":2:oplog.yaml"])?;
                let theirs = run_git_hermetic(repo, &["show", ":3:oplog.yaml"])?;
                let mut lines: BTreeSet<String> = BTreeSet::new();
                for line in String::from_utf8_lossy(&ours.stdout)
                    .lines()
                    .chain(String::from_utf8_lossy(&theirs.stdout).lines())
                {
                    if !line.trim().is_empty() {
                        lines.insert(line.to_string());
                    }
                }
                let merged_oplog = lines.iter().cloned().collect::<Vec<_>>().join("\n") + "\n";
                std::fs::write(repo.join("oplog.yaml"), merged_oplog).map_err(|e| e.to_string())?;
                run_git_hermetic(repo, &["add", "--", "oplog.yaml"])?;
                notes.push(format!("oplog: {} operations after union", lines.len()));
            } else if path.starts_with("objects/") && path.ends_with(".yaml") {
                let base_raw = run_git_hermetic(repo, &["show", &format!(":1:{path}")])?;
                let ours_raw = run_git_hermetic(repo, &["show", &format!(":2:{path}")])?;
                let theirs_raw = run_git_hermetic(repo, &["show", &format!(":3:{path}")])?;

                let base = deserialize_spec(&String::from_utf8_lossy(&base_raw.stdout))?;
                let ours = deserialize_spec(&String::from_utf8_lossy(&ours_raw.stdout))?;
                let theirs = deserialize_spec(&String::from_utf8_lossy(&theirs_raw.stdout))?;

                let outcome = merge_spec_upstream_with_report(&base, &ours, &theirs);
                std::fs::write(repo.join(path), serialize_spec(&outcome.merged))
                    .map_err(|e| e.to_string())?;
                run_git_hermetic(repo, &["add", "--", path])?;

                notes.push(outcome.summary_note());
                if !outcome.overridden_scalars.is_empty() {
                    all_overrides
                        .push((outcome.merged.spec_id.clone(), outcome.overridden_scalars));
                }
            } else {
                let _ = run_git_hermetic(repo, &["rebase", "--abort"]);
                return Err(format!("unresolvable store conflict on {path}"));
            }
        }

        let cont = run_git_hermetic(repo, &["-c", "core.editor=true", "rebase", "--continue"])?;
        if cont.status.success() {
            return Ok(RebaseSyncOutcome {
                notes,
                overridden_scalars: all_overrides,
            });
        }
    }

    let _ = run_git_hermetic(repo, &["rebase", "--abort"]);
    Err("rebase loop exceeded step limit".to_string())
}

// ============================================================================
// 5. Multi-Hub GitLab Mirror, Reconcile & Push-to-Create (BUG-MN-008, STORY-MN-003)
// ============================================================================

/// Parses a GitLab `/api/v4/namespaces/<ns>` JSON body (`BUG-MN-008`).
///
/// In `aida-cli-lib/src/gitlab_project_create.rs:120` (`parse_namespace_id`),
/// an HTTP 401/403 JSON response like:
/// `{"error":"invalid_token","error_description":"Token is expired..."}`
/// returned `None`, causing `create_gitlab_project` to bail with the misleading
/// error `"could not read the id of GitLab namespace 'ai' on gitlab.joemooney.com"`.
///
/// This function surfaces the actual API `error_description` / `message` when
/// present so operators immediately know their `glab` token is expired.
// trace:BUG-MN-008 | ai:antigravity
pub fn parse_gitlab_namespace_id_or_error(
    body: &str,
    host: &str,
    namespace: &str,
) -> Result<u64, String> {
    // Check for explicit error_description or message fields first
    if let Some(desc) = extract_json_string_field(body, "error_description")
        .or_else(|| extract_json_string_field(body, "message"))
        .or_else(|| extract_json_string_field(body, "error"))
    {
        return Err(format!(
            "GitLab API error on {host} for namespace `{namespace}`: {desc}"
        ));
    }

    if let Some(id) = extract_json_u64_field(body, "id") {
        return Ok(id);
    }

    Err(format!(
        "could not read the id of GitLab namespace `{namespace}` on {host}"
    ))
}

fn extract_json_string_field(body: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let idx = body.find(&pattern)?;
    let rest = &body[idx + pattern.len()..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let content = &after_colon[1..];
    let end_quote = content.find('"')?;
    Some(content[..end_quote].to_string())
}

fn extract_json_u64_field(body: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{key}\"");
    let idx = body.find(&pattern)?;
    let rest = &body[idx + pattern.len()..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    let digits: String = after_colon
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u64>().ok()
}

/// Reconciles diverged `aida-store` tips across multiple hubs (`origin`, `gitlab`)
/// by creating a structural union-merge commit in `repo` with all hub tips as parents
/// and fast-forward pushing the union to every hub (`STORY-MN-003`).
// trace:STORY-MN-003 | ai:antigravity
pub fn reconcile_diverged_hubs(repo: &Path, hubs: &[&str], branch: &str) -> Result<String, String> {
    if hubs.is_empty() {
        return Err("no hubs provided".to_string());
    }
    for hub in hubs {
        let fetch = run_git_hermetic(repo, &["fetch", hub, branch])?;
        if !fetch.status.success() {
            return Err(format!(
                "failed to fetch {hub}/{branch}: {}",
                String::from_utf8_lossy(&fetch.stderr)
            ));
        }
    }

    // Checkout local branch tracking the first hub
    let first_ref = format!("{}/{branch}", hubs[0]);
    run_git_hermetic(repo, &["checkout", "-B", branch, &first_ref])?;

    // Merge each additional hub structurally
    for hub in &hubs[1..] {
        let remote_ref = format!("{hub}/{branch}");
        let merge_out = run_git_hermetic(repo, &["merge", "--no-edit", &remote_ref])?;
        if !merge_out.status.success() {
            // Resolve conflicts structurally on oplog.yaml and objects/*.yaml
            let unmerged_out = run_git_hermetic(repo, &["diff", "--name-only", "--diff-filter=U"])?;
            for path in String::from_utf8_lossy(&unmerged_out.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if path == "oplog.yaml" {
                    let ours = run_git_hermetic(repo, &["show", ":2:oplog.yaml"])?;
                    let theirs = run_git_hermetic(repo, &["show", ":3:oplog.yaml"])?;
                    let mut lines: BTreeSet<String> = BTreeSet::new();
                    for line in String::from_utf8_lossy(&ours.stdout)
                        .lines()
                        .chain(String::from_utf8_lossy(&theirs.stdout).lines())
                    {
                        if !line.trim().is_empty() {
                            lines.insert(line.to_string());
                        }
                    }
                    let merged_oplog = lines.iter().cloned().collect::<Vec<_>>().join("\n") + "\n";
                    std::fs::write(repo.join("oplog.yaml"), merged_oplog)
                        .map_err(|e| e.to_string())?;
                    run_git_hermetic(repo, &["add", "--", "oplog.yaml"])?;
                } else if path.starts_with("objects/") && path.ends_with(".yaml") {
                    let base_raw = run_git_hermetic(repo, &["show", &format!(":1:{path}")])?;
                    let ours_raw = run_git_hermetic(repo, &["show", &format!(":2:{path}")])?;
                    let theirs_raw = run_git_hermetic(repo, &["show", &format!(":3:{path}")])?;
                    let base = deserialize_spec(&String::from_utf8_lossy(&base_raw.stdout))?;
                    let ours = deserialize_spec(&String::from_utf8_lossy(&ours_raw.stdout))?;
                    let theirs = deserialize_spec(&String::from_utf8_lossy(&theirs_raw.stdout))?;
                    let outcome = merge_spec_upstream_with_report(&base, &ours, &theirs);
                    std::fs::write(repo.join(path), serialize_spec(&outcome.merged))
                        .map_err(|e| e.to_string())?;
                    run_git_hermetic(repo, &["add", "--", path])?;
                } else {
                    let _ = run_git_hermetic(repo, &["merge", "--abort"]);
                    return Err(format!("unresolvable multi-hub conflict on {path}"));
                }
            }
            let commit = run_git_hermetic(repo, &["commit", "--no-edit"])?;
            if !commit.status.success() {
                return Err(format!(
                    "failed to commit multi-hub union merge: {}",
                    String::from_utf8_lossy(&commit.stderr)
                ));
            }
        }
    }

    let head_out = run_git_hermetic(repo, &["rev-parse", "HEAD"])?;
    let union_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

    // Push union commit to every hub (each is a fast-forward by construction)
    for hub in hubs {
        let push = run_git_hermetic(
            repo,
            &["push", hub, &format!("{union_sha}:refs/heads/{branch}")],
        )?;
        if !push.status.success() {
            return Err(format!(
                "failed to push reconciled union to {hub}: {}",
                String::from_utf8_lossy(&push.stderr)
            ));
        }
    }

    Ok(union_sha)
}

// ============================================================================
// 6. Global / Agreed Spec ID Block Allocation & Multi-Forge Pool Delegation (S14-S16)
// ============================================================================

/// A delegated hub pool (`aida node hub add <hub> --size <N>`) carved by the
/// enrolled allocation root (`registry/root.yaml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubIdPool {
    pub hub_name: String,
    pub type_prefix: String,
    pub pool_start: u32,
    pub pool_end: u32,
}

/// An agreed-ID block claimed by a node (`registry/blocks.yaml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgreedBlock {
    pub node_id: String,
    pub hub_name: String,
    pub type_prefix: String,
    pub range_start: u32,
    pub range_end: u32,
    pub next: u32,
}

impl AgreedBlock {
    pub fn remaining(&self) -> u32 {
        if self.next > self.range_end {
            0
        } else {
            self.range_end - self.next + 1
        }
    }

    pub fn dispense(&mut self) -> Option<String> {
        if self.next > self.range_end {
            None
        } else {
            let id = format!("{}-{:03}", self.type_prefix, self.next);
            self.next += 1;
            Some(id)
        }
    }
}

/// Result of allocating a spec ID on a node (`S14`–`S16`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispensedSpecId {
    /// Canonical node-local spec ID (`TASK-<node>-001`).
    pub spec_id: String,
    /// Global agreed ID (`TASK-001` or `TASK-1001`) if dispensed from an active
    /// `AgreedBlock`; `None` when air-gapped or when no block is available.
    pub agreed_id: Option<String>,
}

/// Claims a new non-overlapping `AgreedBlock` of `size` IDs inside `pool` (or
/// from `1..=u32::MAX` in single-hub mode when `pool` is `None`).
// trace:STORY-MN-003 | ai:antigravity
pub fn claim_block_in_pool(
    existing_blocks: &[AgreedBlock],
    pool: Option<&HubIdPool>,
    node_id: &str,
    hub_name: &str,
    type_prefix: &str,
    size: u32,
) -> Result<AgreedBlock, String> {
    if size == 0 {
        return Err("block size must be > 0".to_string());
    }
    let prefix = type_prefix.to_ascii_uppercase();
    let (floor, ceiling) = match pool {
        Some(p) => {
            if p.hub_name != hub_name || p.type_prefix.to_ascii_uppercase() != prefix {
                return Err(format!(
                    "pool mismatch: expected ({hub_name}, {prefix}), got ({}, {})",
                    p.hub_name, p.type_prefix
                ));
            }
            (p.pool_start, p.pool_end)
        }
        None => (1, u32::MAX - size),
    };

    let mut next_start = floor;
    for b in existing_blocks
        .iter()
        .filter(|b| b.type_prefix.eq_ignore_ascii_case(&prefix))
    {
        if b.range_end >= floor && b.range_start <= ceiling && b.range_end >= next_start {
            next_start = b.range_end.saturating_add(1);
        }
    }
    let range_end = next_start.saturating_add(size - 1);
    if range_end > ceiling {
        return Err(format!(
            "hub pool `{hub_name}` for `{prefix}` exhausted ({next_start}..={range_end} exceeds pool ceiling {ceiling})"
        ));
    }

    Ok(AgreedBlock {
        node_id: node_id.to_string(),
        hub_name: hub_name.to_string(),
        type_prefix: prefix,
        range_start: next_start,
        range_end,
        next: next_start,
    })
}

/// Dispenses a spec ID for `(node_id, type_prefix)`: draws a global `agreed_id`
/// from the first non-exhausted block owned by `node_id` when online/allocated,
/// or falls back to a node-local ID (`agreed_id: None`) when air-gapped.
// trace:STORY-MN-003 | ai:antigravity
pub fn dispense_spec_id(
    blocks: &mut [AgreedBlock],
    node_id: &str,
    type_prefix: &str,
    local_seq: u32,
) -> DispensedSpecId {
    let prefix = type_prefix.to_ascii_uppercase();
    let spec_id = format!("{prefix}-{node_id}-{local_seq:03}");
    let agreed_id = blocks
        .iter_mut()
        .find(|b| {
            b.node_id == node_id && b.type_prefix.eq_ignore_ascii_case(&prefix) && b.remaining() > 0
        })
        .and_then(AgreedBlock::dispense);
    DispensedSpecId { spec_id, agreed_id }
}

// ============================================================================
// 7. Read-Only Clone Per-Node Feedback Branch & Foreign-Write Guard (S17-S18)
// ============================================================================

/// Checksummed observation envelope published by a read-only clone onto its
/// exclusive `refs/heads/aida-feedback-<node-id>` orphan branch (`STORY-2652`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackEnvelope {
    pub producer_node_id: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub checksum: String,
}

/// Processing receipt published by an authorized home processor onto
/// `refs/heads/aida-feedback-receipts-<processor-node-id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackReceipt {
    pub processor_node_id: String,
    pub checksum: String,
    pub action: String,
    pub destination_spec: Option<String>,
}

/// Enforces `STORY-2652` Advisor Ruling `R4` (own-branch foreign-write rule)
/// and read-only clone branch restrictions:
/// - A read-only producer clone may ONLY push to `refs/heads/aida-feedback-<producer_node_id>`.
/// - Pushes to `refs/heads/main` or `refs/heads/aida-store` from the read-only
///   producer are refused.
/// - On the producer's own branch, if `remote_tip` is not equal to `last_published_tip`
///   (e.g., a foreign writer advanced the producer's branch), sync refuses instead
///   of fast-forwarding (`BUG-MN-009` / `STORY-2652` R4).
// trace:BUG-MN-009 trace:STORY-MN-003 trace:STORY-MN-004 | ai:antigravity
pub fn validate_readonly_feedback_push_and_sync(
    producer_node_id: &str,
    target_ref: &str,
    last_published_tip: Option<&str>,
    remote_tip: Option<&str>,
) -> Result<(), String> {
    let expected_ref = format!("refs/heads/aida-feedback-{producer_node_id}");
    if target_ref != expected_ref {
        return Err(format!(
            "read-only clone `{producer_node_id}` is forbidden from pushing to `{target_ref}`; only `{expected_ref}` is writable"
        ));
    }
    if let Some(remote) = remote_tip {
        if last_published_tip != Some(remote) {
            return Err(format!(
                "foreign write detected on producer branch `{expected_ref}`: remote tip `{remote}` != last published tip `{:?}` (STORY-2652 R4 refusal)",
                last_published_tip
            ));
        }
    }
    Ok(())
}

// ============================================================================
// 8. Inter-Node Hourly Heartbeat & Allow-Listed Action Channel (S19)
// ============================================================================

/// Closed allow-list of remote actions a node may execute during an hourly
/// heartbeat tick (`STORY-MN-004`). Arbitrary shell strings are never admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeAction {
    /// `aida pull --code-only`
    PullCode,
    /// `aida db sync --pull`
    PullStore,
    /// `aida db sync --push`
    PushStore,
    /// `aida intent pull`
    PullFeedback,
    /// `aida intent publish`
    PublishFeedback,
    /// `aida remote status`
    ReportStatus,
}

impl NodeAction {
    /// Returns the exact allow-listed `aida` CLI arguments for this action.
    pub fn cli_args(self) -> &'static [&'static str] {
        match self {
            Self::PullCode => &["pull", "--code-only"],
            Self::PullStore => &["db", "sync", "--pull"],
            Self::PushStore => &["db", "sync", "--push"],
            Self::PullFeedback => &["intent", "pull"],
            Self::PublishFeedback => &["intent", "publish"],
            Self::ReportStatus => &["remote", "status"],
        }
    }
}

/// A directed action message queued for `target_node_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDirective {
    pub directive_id: String,
    pub target_node_id: String,
    pub issued_by: String,
    pub action: NodeAction,
    pub issued_at_epoch_s: u64,
}

/// Durable receipt proving `directive_id` was already executed by `node_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveReceipt {
    pub directive_id: String,
    pub node_id: String,
    pub executed_at_epoch_s: u64,
    pub status: String,
}

/// Periodic liveness & state beacon emitted by a node on its hourly tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeHeartbeat {
    pub node_id: String,
    pub timestamp_epoch_s: u64,
    pub code_head_sha: String,
    pub store_head_sha: String,
    pub executed_directives: Vec<DirectiveReceipt>,
}

/// Evaluates an hourly heartbeat tick for `node_id`:
/// - Filters `inbox` for directives targeting `node_id` (or `"*"` broadcast)
///   that have not yet been recorded in `prior_receipts`.
/// - For read-only clones (`is_readonly_clone == true`), refuses store-mutating
///   actions (`PushStore`) while allowing `PullCode`, `PullStore`, `PullFeedback`,
///   `PublishFeedback`, and `ReportStatus`.
/// - Returns the ordered list of `(NodeDirective, &'static [&'static str])` CLI
///   invocations to run and the updated `NodeHeartbeat` with idempotent receipts.
// trace:STORY-MN-004 | ai:antigravity
pub fn evaluate_hourly_node_tick(
    node_id: &str,
    is_readonly_clone: bool,
    now_epoch_s: u64,
    code_head_sha: &str,
    store_head_sha: &str,
    inbox: &[NodeDirective],
    prior_receipts: &[DirectiveReceipt],
) -> (Vec<(NodeDirective, &'static [&'static str])>, NodeHeartbeat) {
    let done_ids: BTreeSet<&str> = prior_receipts
        .iter()
        .filter(|r| r.node_id == node_id)
        .map(|r| r.directive_id.as_str())
        .collect();

    let mut planned = Vec::new();
    let mut receipts = prior_receipts.to_vec();

    for d in inbox {
        if d.target_node_id != node_id && d.target_node_id != "*" {
            continue;
        }
        if done_ids.contains(d.directive_id.as_str()) {
            continue;
        }
        if is_readonly_clone && d.action == NodeAction::PushStore {
            receipts.push(DirectiveReceipt {
                directive_id: d.directive_id.clone(),
                node_id: node_id.to_string(),
                executed_at_epoch_s: now_epoch_s,
                status: "refused:read-only-clone".to_string(),
            });
            continue;
        }
        planned.push((d.clone(), d.action.cli_args()));
        receipts.push(DirectiveReceipt {
            directive_id: d.directive_id.clone(),
            node_id: node_id.to_string(),
            executed_at_epoch_s: now_epoch_s,
            status: "executed".to_string(),
        });
    }

    let heartbeat = NodeHeartbeat {
        node_id: node_id.to_string(),
        timestamp_epoch_s: now_epoch_s,
        code_head_sha: code_head_sha.to_string(),
        store_head_sha: store_head_sha.to_string(),
        executed_directives: receipts,
    };
    (planned, heartbeat)
}

// ============================================================================
// 9. Unit & Multi-Clone Integration Tests (BUG-MN-001 through BUG-MN-008, STORY-MN-003)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // trace:BUG-MN-006 | ai:antigravity
    #[test]
    fn bug_mn_006_default_author_respects_aida_user_before_system_user() {
        let author = resolve_default_author_with(|key| match key {
            "AIDA_AUTHOR" => None,
            "AIDA_USER" => Some("node-b".to_string()),
            "USER" => Some("joe".to_string()),
            _ => None,
        });
        assert_eq!(author, "node-b");

        // Explicit AIDA_AUTHOR still takes top priority when set
        let explicit = resolve_default_author_with(|key| match key {
            "AIDA_AUTHOR" => Some("ai:codex:node-b".to_string()),
            "AIDA_USER" => Some("node-b".to_string()),
            "USER" => Some("joe".to_string()),
            _ => None,
        });
        assert_eq!(explicit, "ai:codex:node-b");
    }

    // trace:BUG-MN-004 | ai:antigravity
    #[test]
    fn bug_mn_004_edit_records_field_changes_in_history() {
        let mut spec = SpecRecord::new("FR-NODEA-001", "Original Title", "Original Desc");
        let patch = SpecEditPatch {
            title: Some("Edited Title".to_string()),
            priority: Some("High".to_string()),
            add_tags: ["security".to_string()].into_iter().collect(),
            ..Default::default()
        };

        let changes = apply_spec_edit_recorded(&mut spec, &patch, "hist-1", "node-a", 100);
        assert_eq!(changes.len(), 3);
        assert_eq!(spec.history.len(), 1);
        assert_eq!(spec.history[0].author, "node-a");
        assert_eq!(spec.history[0].changes.len(), 3);
        assert_eq!(spec.history[0].changes[0].field_name, "title");
        assert_eq!(spec.history[0].changes[0].old_value, "Original Title");
        assert_eq!(spec.history[0].changes[0].new_value, "Edited Title");
    }

    // trace:BUG-MN-002 | ai:antigravity
    #[test]
    fn bug_mn_002_merge_spec_upstream_preserves_non_colliding_local_scalar_edits() {
        let base = SpecRecord::new("FR-NODEA-001", "Base Title", "Base Desc");

        // Node A (upstream) edits ONLY title
        let mut upstream = base.clone();
        apply_spec_edit_recorded(
            &mut upstream,
            &SpecEditPatch {
                title: Some("Upstream Title".to_string()),
                ..Default::default()
            },
            "h-up",
            "node-a",
            10,
        );

        // Node B (local) edits ONLY description and priority
        let mut local = base.clone();
        apply_spec_edit_recorded(
            &mut local,
            &SpecEditPatch {
                description: Some("Local Description".to_string()),
                priority: Some("High".to_string()),
                ..Default::default()
            },
            "h-loc",
            "node-b",
            11,
        );

        let outcome = merge_spec_upstream_with_report(&base, &upstream, &local);
        assert_eq!(outcome.merged.title, "Upstream Title");
        assert_eq!(
            outcome.merged.description, "Local Description",
            "Non-colliding local description edit must survive merge_spec_upstream!"
        );
        assert_eq!(
            outcome.merged.priority, "High",
            "Non-colliding local priority edit must survive merge_spec_upstream!"
        );
        assert!(
            outcome.overridden_scalars.is_empty(),
            "Disjoint scalar edits must not report scalar overrides"
        );
        assert_eq!(outcome.merged.history.len(), 2);
    }

    // trace:BUG-MN-002 | ai:antigravity
    #[test]
    fn bug_mn_002_merge_spec_upstream_reports_colliding_scalar_overrides() {
        let base = SpecRecord::new("FR-NODEA-001", "Base Title", "Base Desc");

        let mut upstream = base.clone();
        apply_spec_edit_recorded(
            &mut upstream,
            &SpecEditPatch {
                title: Some("Title from Node A".to_string()),
                priority: Some("High".to_string()),
                ..Default::default()
            },
            "h-a",
            "node-a",
            10,
        );

        let mut local = base.clone();
        apply_spec_edit_recorded(
            &mut local,
            &SpecEditPatch {
                title: Some("Title from Node B".to_string()),
                priority: Some("Low".to_string()),
                ..Default::default()
            },
            "h-b",
            "node-b",
            11,
        );

        let outcome = merge_spec_upstream_with_report(&base, &upstream, &local);
        assert_eq!(outcome.merged.title, "Title from Node A");
        assert_eq!(outcome.merged.priority, "High");
        assert_eq!(outcome.overridden_scalars.len(), 2);
        assert_eq!(outcome.overridden_scalars[0].field_name, "title");
        assert_eq!(
            outcome.overridden_scalars[0].local_value,
            "Title from Node B"
        );
        assert_eq!(
            outcome.overridden_scalars[0].winning_upstream_value,
            "Title from Node A"
        );
        assert!(outcome
            .summary_note()
            .contains("upstream overrode colliding local edits on: title, priority"));
    }

    // trace:BUG-MN-003 | ai:antigravity
    #[test]
    fn bug_mn_003_fast_forward_pull_does_not_report_false_conflict() {
        let base = SpecRecord::new("TASK-NODEA-003", "Write S3 test", "Initial");
        // Local has not touched the spec since base
        let local = base.clone();
        // Remote edited the spec
        let mut remote = base.clone();
        apply_spec_edit_recorded(
            &mut remote,
            &SpecEditPatch {
                title: Some("Write S3 test (updated on B)".to_string()),
                status: Some("Completed".to_string()),
                ..Default::default()
            },
            "h-b",
            "node-b",
            20,
        );

        assert_eq!(
            detect_conflict_with_base(&base, &local, &remote),
            None,
            "Clean fast-forward pull must not trigger a false conflict!"
        );

        // Also verify duplicate prefix stripping
        let note = "auto-merged BUG-002: unioned 2 history entries, status upstream=Draft";
        assert_eq!(
            format_auto_merged_line(note),
            "auto-merged BUG-002: unioned 2 history entries, status upstream=Draft"
        );
    }

    // trace:BUG-MN-001 trace:BUG-MN-002 | ai:antigravity
    #[test]
    fn bug_mn_001_and_002_git_rebase_auto_merge_succeeds_under_git_editor_false() {
        let tmp = std::env::temp_dir().join(format!(
            "aida-store-sync-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let hub = tmp.join("hub.git");
        let node_a = tmp.join("node-a");
        let node_b = tmp.join("node-b");
        std::fs::create_dir_all(&tmp).unwrap();

        // 1. Initialize bare hub and node-a
        run_git_hermetic(
            &tmp,
            &["init", "--bare", "-b", "aida-store", hub.to_str().unwrap()],
        )
        .unwrap();
        run_git_hermetic(
            &tmp,
            &["clone", hub.to_str().unwrap(), node_a.to_str().unwrap()],
        )
        .unwrap();

        for repo in [&node_a] {
            run_git_hermetic(repo, &["checkout", "-b", "aida-store"]).unwrap();
            run_git_hermetic(repo, &["config", "user.name", "Node A"]).unwrap();
            run_git_hermetic(repo, &["config", "user.email", "a@test.invalid"]).unwrap();
        }

        std::fs::create_dir_all(node_a.join("objects")).unwrap();
        let base_spec = SpecRecord::new("FR-001", "Seed Title", "Seed Desc");
        std::fs::write(
            node_a.join("objects/FR-001.yaml"),
            serialize_spec(&base_spec),
        )
        .unwrap();
        std::fs::write(node_a.join("oplog.yaml"), "- op: seed\n").unwrap();
        run_git_hermetic(&node_a, &["add", "."]).unwrap();
        run_git_hermetic(&node_a, &["commit", "-m", "seed store"]).unwrap();
        run_git_hermetic(&node_a, &["push", "-u", "origin", "aida-store"]).unwrap();

        // 2. Clone node-b from hub
        run_git_hermetic(
            &tmp,
            &[
                "clone",
                "-b",
                "aida-store",
                hub.to_str().unwrap(),
                node_b.to_str().unwrap(),
            ],
        )
        .unwrap();
        run_git_hermetic(&node_b, &["config", "user.name", "Node B"]).unwrap();
        run_git_hermetic(&node_b, &["config", "user.email", "b@test.invalid"]).unwrap();

        // 3. Divergent edits on FR-001 and oplog.yaml:
        //    - Node A changes title="Title A" (colliding)
        //    - Node B changes title="Title B" (colliding) AND description="Desc B" (non-colliding!)
        let mut spec_a = base_spec.clone();
        apply_spec_edit_recorded(
            &mut spec_a,
            &SpecEditPatch {
                title: Some("Title A".to_string()),
                ..Default::default()
            },
            "ha-1",
            "node-a",
            10,
        );
        std::fs::write(node_a.join("objects/FR-001.yaml"), serialize_spec(&spec_a)).unwrap();
        std::fs::write(node_a.join("oplog.yaml"), "- op: seed\n- op: node-a-edit\n").unwrap();
        run_git_hermetic(&node_a, &["add", "."]).unwrap();
        run_git_hermetic(&node_a, &["commit", "-m", "node-a edit"]).unwrap();
        run_git_hermetic(&node_a, &["push", "origin", "aida-store"]).unwrap();

        let mut spec_b = base_spec.clone();
        apply_spec_edit_recorded(
            &mut spec_b,
            &SpecEditPatch {
                title: Some("Title B".to_string()),
                description: Some("Desc B".to_string()),
                ..Default::default()
            },
            "hb-1",
            "node-b",
            11,
        );
        std::fs::write(node_b.join("objects/FR-001.yaml"), serialize_spec(&spec_b)).unwrap();
        std::fs::write(node_b.join("oplog.yaml"), "- op: seed\n- op: node-b-edit\n").unwrap();
        run_git_hermetic(&node_b, &["add", "."]).unwrap();
        run_git_hermetic(&node_b, &["commit", "-m", "node-b edit"]).unwrap();

        // 4. Fetch on Node B and run rebase_auto_merge_onto with GIT_EDITOR=false in env!
        run_git_hermetic(&node_b, &["fetch", "origin", "aida-store"]).unwrap();
        std::env::set_var("GIT_EDITOR", "false");
        let rebase_res = rebase_auto_merge_onto(&node_b, "origin/aida-store");
        assert!(
            rebase_res.is_ok(),
            "rebase_auto_merge_onto failed under GIT_EDITOR=false: {rebase_res:?}"
        );
        let outcome = rebase_res.unwrap();

        // 5. Verify:
        //    - Colliding title resolved to upstream ("Title A") AND reported in overridden_scalars
        //    - Non-colliding description from Node B ("Desc B") survived!
        //    - Both history entries ("ha-1" and "hb-1") survived with full FieldChange details!
        let merged_disk =
            deserialize_spec(&std::fs::read_to_string(node_b.join("objects/FR-001.yaml")).unwrap())
                .unwrap();
        assert_eq!(merged_disk.title, "Title A");
        assert_eq!(merged_disk.description, "Desc B");
        assert_eq!(merged_disk.history.len(), 2);
        assert_eq!(outcome.overridden_scalars.len(), 1);
        assert_eq!(outcome.overridden_scalars[0].0, "FR-001");
        assert_eq!(outcome.overridden_scalars[0].1.len(), 1);
        assert_eq!(outcome.overridden_scalars[0].1[0].field_name, "title");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // trace:BUG-MN-008 | ai:antigravity
    #[test]
    fn bug_mn_008_gitlab_namespace_parser_surfaces_expired_token_error() {
        let expired_body = r#"{
  "error": "invalid_token",
  "error_description": "Token is expired. You can either do re-authorization or token refresh."
}"#;
        let err = parse_gitlab_namespace_id_or_error(expired_body, "gitlab.joemooney.com", "ai")
            .unwrap_err();
        assert!(
            err.contains("Token is expired"),
            "Expected expired token message to be surfaced, got: {err}"
        );

        let valid_body = r#"{"id": 42, "name": "ai", "path": "ai"}"#;
        let id =
            parse_gitlab_namespace_id_or_error(valid_body, "gitlab.joemooney.com", "ai").unwrap();
        assert_eq!(id, 42);
    }

    // trace:STORY-MN-003 | ai:antigravity
    #[test]
    fn story_mn_003_multi_hub_reconcile_merges_diverged_origin_and_gitlab() {
        let tmp = std::env::temp_dir().join(format!(
            "aida-multihub-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let origin_hub = tmp.join("origin.git");
        let gitlab_hub = tmp.join("gitlab.git");
        let bridge_clone = tmp.join("bridge");
        let gitlab_only_clone = tmp.join("work-clone");
        std::fs::create_dir_all(&tmp).unwrap();

        run_git_hermetic(
            &tmp,
            &[
                "init",
                "--bare",
                "-b",
                "aida-store",
                origin_hub.to_str().unwrap(),
            ],
        )
        .unwrap();
        run_git_hermetic(
            &tmp,
            &[
                "init",
                "--bare",
                "-b",
                "aida-store",
                gitlab_hub.to_str().unwrap(),
            ],
        )
        .unwrap();

        // Seed bridge clone and push initial store to both origin and gitlab
        run_git_hermetic(
            &tmp,
            &[
                "clone",
                origin_hub.to_str().unwrap(),
                bridge_clone.to_str().unwrap(),
            ],
        )
        .unwrap();
        run_git_hermetic(&bridge_clone, &["checkout", "-b", "aida-store"]).unwrap();
        run_git_hermetic(&bridge_clone, &["config", "user.name", "Bridge"]).unwrap();
        run_git_hermetic(
            &bridge_clone,
            &["config", "user.email", "bridge@test.invalid"],
        )
        .unwrap();
        run_git_hermetic(
            &bridge_clone,
            &["remote", "add", "gitlab", gitlab_hub.to_str().unwrap()],
        )
        .unwrap();

        std::fs::create_dir_all(bridge_clone.join("objects")).unwrap();
        let seed_spec = SpecRecord::new("FR-001", "Shared Seed", "Initial");
        std::fs::write(
            bridge_clone.join("objects/FR-001.yaml"),
            serialize_spec(&seed_spec),
        )
        .unwrap();
        std::fs::write(bridge_clone.join("oplog.yaml"), "- op: seed\n").unwrap();
        run_git_hermetic(&bridge_clone, &["add", "."]).unwrap();
        run_git_hermetic(&bridge_clone, &["commit", "-m", "seed"]).unwrap();
        run_git_hermetic(&bridge_clone, &["push", "-u", "origin", "aida-store"]).unwrap();
        run_git_hermetic(&bridge_clone, &["push", "-u", "gitlab", "aida-store"]).unwrap();

        // Clone work-clone from gitlab_hub only
        run_git_hermetic(
            &tmp,
            &[
                "clone",
                "-b",
                "aida-store",
                gitlab_hub.to_str().unwrap(),
                gitlab_only_clone.to_str().unwrap(),
            ],
        )
        .unwrap();
        run_git_hermetic(&gitlab_only_clone, &["config", "user.name", "Work"]).unwrap();
        run_git_hermetic(
            &gitlab_only_clone,
            &["config", "user.email", "work@test.invalid"],
        )
        .unwrap();

        // 1. Bridge clone edits FR-001 title and pushes ONLY to origin
        let mut origin_spec = seed_spec.clone();
        apply_spec_edit_recorded(
            &mut origin_spec,
            &SpecEditPatch {
                title: Some("Title on Origin".to_string()),
                ..Default::default()
            },
            "h-origin",
            "node-origin",
            10,
        );
        std::fs::write(
            bridge_clone.join("objects/FR-001.yaml"),
            serialize_spec(&origin_spec),
        )
        .unwrap();
        std::fs::write(
            bridge_clone.join("oplog.yaml"),
            "- op: seed\n- op: origin-edit\n",
        )
        .unwrap();
        run_git_hermetic(&bridge_clone, &["add", "."]).unwrap();
        run_git_hermetic(&bridge_clone, &["commit", "-m", "origin edit"]).unwrap();
        run_git_hermetic(&bridge_clone, &["push", "origin", "aida-store"]).unwrap();

        // 2. Work clone edits FR-001 description + adds TASK-GL-001 and pushes ONLY to gitlab
        let mut gl_spec = seed_spec.clone();
        apply_spec_edit_recorded(
            &mut gl_spec,
            &SpecEditPatch {
                description: Some("Desc on GitLab".to_string()),
                ..Default::default()
            },
            "h-gitlab",
            "node-gitlab",
            11,
        );
        std::fs::write(
            gitlab_only_clone.join("objects/FR-001.yaml"),
            serialize_spec(&gl_spec),
        )
        .unwrap();
        let new_gl_task = SpecRecord::new("TASK-GL-001", "GitLab Task", "Filed on GitLab");
        std::fs::write(
            gitlab_only_clone.join("objects/TASK-GL-001.yaml"),
            serialize_spec(&new_gl_task),
        )
        .unwrap();
        std::fs::write(
            gitlab_only_clone.join("oplog.yaml"),
            "- op: seed\n- op: gitlab-edit\n",
        )
        .unwrap();
        run_git_hermetic(&gitlab_only_clone, &["add", "."]).unwrap();
        run_git_hermetic(&gitlab_only_clone, &["commit", "-m", "gitlab edit"]).unwrap();
        run_git_hermetic(&gitlab_only_clone, &["push", "origin", "aida-store"]).unwrap();

        // 3. Reconcile diverged origin and gitlab hubs from bridge_clone
        std::env::set_var("GIT_EDITOR", "false");
        let union_sha =
            reconcile_diverged_hubs(&bridge_clone, &["origin", "gitlab"], "aida-store").unwrap();

        // 4. Verify both bare hubs now point to the exact same union commit,
        //    and both edits + TASK-GL-001 are preserved!
        let origin_tip = String::from_utf8_lossy(
            &run_git_hermetic(&origin_hub, &["rev-parse", "refs/heads/aida-store"])
                .unwrap()
                .stdout,
        )
        .trim()
        .to_string();
        let gitlab_tip = String::from_utf8_lossy(
            &run_git_hermetic(&gitlab_hub, &["rev-parse", "refs/heads/aida-store"])
                .unwrap()
                .stdout,
        )
        .trim()
        .to_string();
        assert_eq!(origin_tip, union_sha);
        assert_eq!(gitlab_tip, union_sha);

        let merged_fr = deserialize_spec(
            &std::fs::read_to_string(bridge_clone.join("objects/FR-001.yaml")).unwrap(),
        )
        .unwrap();
        assert_eq!(merged_fr.title, "Title on Origin");
        assert_eq!(merged_fr.description, "Desc on GitLab");
        assert_eq!(merged_fr.history.len(), 2);
        assert!(bridge_clone.join("objects/TASK-GL-001.yaml").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn s14_s16_global_id_block_allocation_single_forge_multiforge_and_airgapped() {
        let mut blocks = Vec::new();
        // 1. Single-forge: Node A and Node B claim contiguous TASK blocks on origin
        let b_a = claim_block_in_pool(&blocks, None, "node-a", "origin", "TASK", 5).unwrap();
        assert_eq!((b_a.range_start, b_a.range_end), (1, 5));
        blocks.push(b_a);

        let b_b = claim_block_in_pool(&blocks, None, "node-b", "origin", "TASK", 5).unwrap();
        assert_eq!((b_b.range_start, b_b.range_end), (6, 10));
        blocks.push(b_b);

        // 2. Multi-forge delegated pool: gitlab granted 1001..=2000 for TASK
        let gitlab_pool = HubIdPool {
            hub_name: "gitlab".to_string(),
            type_prefix: "TASK".to_string(),
            pool_start: 1001,
            pool_end: 2000,
        };
        let b_c = claim_block_in_pool(&blocks, Some(&gitlab_pool), "node-c", "gitlab", "TASK", 10)
            .unwrap();
        assert_eq!((b_c.range_start, b_c.range_end), (1001, 1010));
        blocks.push(b_c);

        // Dispense on Node A -> TASK-001, Node B -> TASK-006, Node C -> TASK-1001
        let id_a = dispense_spec_id(&mut blocks, "node-a", "TASK", 1);
        let id_b = dispense_spec_id(&mut blocks, "node-b", "TASK", 1);
        let id_c = dispense_spec_id(&mut blocks, "node-c", "TASK", 1);
        assert_eq!(id_a.agreed_id.as_deref(), Some("TASK-001"));
        assert_eq!(id_b.agreed_id.as_deref(), Some("TASK-006"));
        assert_eq!(id_c.agreed_id.as_deref(), Some("TASK-1001"));

        // 3. Air-gapped Node D (no block) -> falls back to node-local with agreed_id = None
        let id_d = dispense_spec_id(&mut blocks, "node-d", "TASK", 1);
        assert_eq!(id_d.spec_id, "TASK-node-d-001");
        assert!(id_d.agreed_id.is_none());
    }

    #[test]
    fn s17_s18_readonly_clone_feedback_branch_and_foreign_write_guard() {
        let node_ro = "a65b4bf5902d469995f816efd3a27661";
        // Pushing to main or aida-store from read-only clone is refused
        assert!(
            validate_readonly_feedback_push_and_sync(node_ro, "refs/heads/main", None, None)
                .is_err()
        );
        assert!(validate_readonly_feedback_push_and_sync(
            node_ro,
            "refs/heads/aida-store",
            None,
            None
        )
        .is_err());

        // Pushing to own feedback branch succeeds when remote matches last published tip
        let own_ref = format!("refs/heads/aida-feedback-{node_ro}");
        assert!(validate_readonly_feedback_push_and_sync(
            node_ro,
            &own_ref,
            Some("abc111"),
            Some("abc111")
        )
        .is_ok());

        // Foreign write advancing remote tip on producer's own branch is refused (STORY-2652 R4)
        let err = validate_readonly_feedback_push_and_sync(
            node_ro,
            &own_ref,
            Some("abc111"),
            Some("def222"),
        )
        .unwrap_err();
        assert!(err.contains("foreign write detected"));
    }

    #[test]
    fn s19_hourly_heartbeat_executes_allowlisted_actions_idempotently() {
        let inbox = vec![
            NodeDirective {
                directive_id: "dir-1".to_string(),
                target_node_id: "node-ro".to_string(),
                issued_by: "node-home".to_string(),
                action: NodeAction::PullCode,
                issued_at_epoch_s: 1000,
            },
            NodeDirective {
                directive_id: "dir-2".to_string(),
                target_node_id: "node-ro".to_string(),
                issued_by: "node-home".to_string(),
                action: NodeAction::PushStore, // Forbidden on read-only clone!
                issued_at_epoch_s: 1001,
            },
            NodeDirective {
                directive_id: "dir-3".to_string(),
                target_node_id: "*".to_string(),
                issued_by: "node-home".to_string(),
                action: NodeAction::PullFeedback,
                issued_at_epoch_s: 1002,
            },
        ];

        let (planned, hb1) = evaluate_hourly_node_tick(
            "node-ro",
            true,
            3600,
            "code-sha-1",
            "store-sha-1",
            &inbox,
            &[],
        );
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].1, &["pull", "--code-only"]);
        assert_eq!(planned[1].1, &["intent", "pull"]);
        assert_eq!(hb1.executed_directives.len(), 3);
        assert_eq!(hb1.executed_directives[1].status, "refused:read-only-clone");

        // Second hourly tick with same inbox executes nothing (idempotent receipts)
        let (planned_again, _hb2) = evaluate_hourly_node_tick(
            "node-ro",
            true,
            7200,
            "code-sha-1",
            "store-sha-1",
            &inbox,
            &hb1.executed_directives,
        );
        assert!(planned_again.is_empty());
    }
}
