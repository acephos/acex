use std::fs;
use std::path::Path;
use std::process::Command;

use acex_discover::DiscoveryReport;
use herdr_client::SocketTarget;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const CHECKPOINT_STATUS_SCHEMA_VERSION: u64 = 1;
pub const HERDR_PROTOCOL: u64 = 16;
pub const HERDR_VERSION: &str = "0.7.2-preview";

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub dirty: bool,
}

#[derive(Debug, Clone)]
struct TrackerProbe {
    valid: bool,
    error: Option<String>,
    last_updated: Option<String>,
    checkpoint_schema_version: Option<u64>,
    active_phase: Option<String>,
    latest_comment_id: Option<String>,
    next_ready: Vec<String>,
}

#[derive(Debug, Clone)]
struct LedgerProbe {
    entries: usize,
    latest_hash: Option<String>,
    valid: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CheckpointCapsule {
    schema_version: u64,
    last_updated: String,
    active_phase: String,
    latest_comment_id: String,
    next_ready: Vec<NextReadyItem>,
}

#[derive(Debug, Deserialize)]
struct NextReadyItem {
    id: String,
}

pub fn collect_git_info(root: &Path) -> GitInfo {
    let branch = git_output(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    let commit = git_output(root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let dirty = git_output(root, &["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    GitInfo {
        branch,
        commit,
        dirty,
    }
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn build_checkpoint_status(
    root: &Path,
    target: &SocketTarget,
    discovery: &DiscoveryReport,
    git: GitInfo,
) -> Value {
    let tracker = read_tracker(root);
    let ledger = read_ledger(root);
    let socket_target = target
        .path_hint()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    json!({
        "schema_version": CHECKPOINT_STATUS_SCHEMA_VERSION,
        "project_root": root.to_string_lossy(),
        "git": {
            "branch": git.branch,
            "commit": git.commit,
            "dirty": git.dirty,
        },
        "tracker": {
            "path": "docs/tracker.html",
            "valid": tracker.valid,
            "error": tracker.error,
            "last_updated": tracker.last_updated,
            "checkpoint_schema_version": tracker.checkpoint_schema_version,
            "active_phase": tracker.active_phase,
            "next_ready": tracker.next_ready,
            "latest_comment_id": tracker.latest_comment_id,
        },
        "ledger": {
            "path": "docs/checkpoint-ledger.jsonl",
            "entries": ledger.entries,
            "latest_hash": ledger.latest_hash,
            "valid": ledger.valid,
            "error": ledger.error,
        },
        "herdr": {
            "side_effects": "none",
            "conn": "Unknown",
            "protocol": HERDR_PROTOCOL,
            "version": HERDR_VERSION,
            "socket_target": socket_target,
        },
        "discovery": {
            "packages": &discovery.packages,
            "skills": &discovery.skills,
            "diagnostics": &discovery.diagnostics,
        },
        "config": {
            "start_presets": []
        }
    })
}

fn read_tracker(root: &Path) -> TrackerProbe {
    let path = root.join("docs").join("tracker.html");
    let html = match fs::read_to_string(&path) {
        Ok(html) => html,
        Err(err) => return tracker_error(format!("read {}: {err}", path.display())),
    };
    match parse_checkpoint_capsule(&html) {
        Ok(capsule) => TrackerProbe {
            valid: true,
            error: None,
            last_updated: Some(capsule.last_updated),
            checkpoint_schema_version: Some(capsule.schema_version),
            active_phase: Some(capsule.active_phase),
            latest_comment_id: Some(capsule.latest_comment_id),
            next_ready: capsule.next_ready.into_iter().map(|item| item.id).collect(),
        },
        Err(err) => tracker_error(err),
    }
}

fn tracker_error(error: String) -> TrackerProbe {
    TrackerProbe {
        valid: false,
        error: Some(error),
        last_updated: None,
        checkpoint_schema_version: None,
        active_phase: None,
        latest_comment_id: None,
        next_ready: Vec::new(),
    }
}

fn parse_checkpoint_capsule(html: &str) -> Result<CheckpointCapsule, String> {
    let raw = extract_checkpoint_script(html)?;
    serde_json::from_str(raw).map_err(|err| format!("invalid acex checkpoint capsule JSON: {err}"))
}

fn extract_checkpoint_script(html: &str) -> Result<&str, String> {
    let id_pos = html
        .find("id=\"acex-checkpoint\"")
        .ok_or_else(|| "missing <script id=\"acex-checkpoint\"> capsule".to_string())?;
    let script_start = html[..id_pos]
        .rfind("<script")
        .ok_or_else(|| "checkpoint capsule id is not inside a script tag".to_string())?;
    let open_end_rel = html[script_start..]
        .find('>')
        .ok_or_else(|| "checkpoint capsule script tag is not closed".to_string())?;
    let content_start = script_start + open_end_rel + 1;
    let close_rel = html[content_start..]
        .find("</script>")
        .ok_or_else(|| "checkpoint capsule script is missing </script>".to_string())?;
    Ok(html[content_start..content_start + close_rel].trim())
}

fn read_ledger(root: &Path) -> LedgerProbe {
    let path = root.join("docs").join("checkpoint-ledger.jsonl");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            return LedgerProbe {
                entries: 0,
                latest_hash: None,
                valid: false,
                error: Some(format!("read {}: {err}", path.display())),
            }
        }
    };
    validate_ledger_text(&text)
}

fn validate_ledger_text(text: &str) -> LedgerProbe {
    let mut previous_hash = "GENESIS".to_string();
    let mut latest_hash = None;
    let mut entries = 0;

    if text.is_empty() {
        return LedgerProbe {
            entries,
            latest_hash,
            valid: false,
            error: Some("ledger is empty".to_string()),
        };
    }

    for (idx, line) in text.split_terminator('\n').enumerate() {
        let line_no = idx + 1;
        entries = line_no;
        if line.trim().is_empty() {
            return ledger_error(entries, latest_hash, format!("blank line at {line_no}"));
        }
        let mut entry: Value = match serde_json::from_str(line) {
            Ok(entry) => entry,
            Err(err) => {
                return ledger_error(entries, latest_hash, format!("line {line_no} JSON: {err}"));
            }
        };
        let Some(object) = entry.as_object_mut() else {
            return ledger_error(
                entries,
                latest_hash,
                format!("line {line_no} is not an object"),
            );
        };
        let prev_hash_matches = object
            .get("prev_hash")
            .and_then(Value::as_str)
            .map(|prev| prev == previous_hash)
            .unwrap_or(false);
        if !prev_hash_matches {
            return ledger_error(
                entries,
                latest_hash,
                format!("line {line_no} prev_hash does not match previous hash"),
            );
        }
        let actual_hash = object
            .get("hash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        object.remove("hash");
        let canonical = serde_json::to_string(&entry).unwrap_or_default();
        let expected_hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        if actual_hash != expected_hash {
            return ledger_error(
                entries,
                latest_hash,
                format!("line {line_no} hash mismatch; expected {expected_hash}"),
            );
        }
        previous_hash = expected_hash.clone();
        latest_hash = Some(expected_hash);
    }

    LedgerProbe {
        entries,
        latest_hash,
        valid: true,
        error: None,
    }
}

fn ledger_error(entries: usize, latest_hash: Option<String>, error: String) -> LedgerProbe {
    LedgerProbe {
        entries,
        latest_hash,
        valid: false,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acex_discover::DiscoveryReport;
    use tempfile::tempdir;

    #[test]
    fn parses_checkpoint_capsule() {
        let html = r#"<html><body><script type="application/json" id="acex-checkpoint">
        {"schema_version":1,"last_updated":"2026-07-14","active_phase":"G1 polish","latest_comment_id":"2026-07-14-x","next_ready":[{"id":"F31"},{"id":"F32"}]}
        </script></body></html>"#;
        let capsule = parse_checkpoint_capsule(html).unwrap();
        assert_eq!(capsule.schema_version, 1);
        assert_eq!(capsule.active_phase, "G1 polish");
        assert_eq!(capsule.next_ready.len(), 2);
    }

    #[test]
    fn checkpoint_status_golden_contract() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(
            root.join("docs").join("tracker.html"),
            r#"<script type="application/json" id="acex-checkpoint">{"schema_version":1,"last_updated":"2026-07-14","active_phase":"G1 polish","latest_comment_id":"2026-07-14-hardening","next_ready":[{"id":"F31"},{"id":"F32"},{"id":"F33"}]}</script>"#,
        )
        .unwrap();
        let first = ledger_line(json!({
            "schema_version": 1,
            "ts": "2026-07-14T00:00:00Z",
            "kind": "process",
            "actor": "test",
            "commit": {"kind":"pending","reason":"entry created in same commit"},
            "refs": ["docs/tracker.html"],
            "state": "test state",
            "evidence": ["test evidence"],
            "next": "test next",
            "prev_hash": "GENESIS"
        }));
        fs::write(root.join("docs").join("checkpoint-ledger.jsonl"), first).unwrap();

        let status = build_checkpoint_status(
            root,
            &SocketTarget::Default,
            &DiscoveryReport::default(),
            GitInfo {
                branch: "master".to_string(),
                commit: "0123456789012345678901234567890123456789".to_string(),
                dirty: false,
            },
        );

        assert_eq!(status["schema_version"], json!(1));
        assert_eq!(status["git"]["branch"], json!("master"));
        assert_eq!(status["tracker"]["valid"], json!(true));
        assert_eq!(
            status["tracker"]["next_ready"],
            json!(["F31", "F32", "F33"])
        );
        assert_eq!(status["ledger"]["entries"], json!(1));
        assert_eq!(status["ledger"]["valid"], json!(true));
        assert_eq!(status["herdr"]["side_effects"], json!("none"));
        assert_eq!(status["discovery"]["diagnostics"], json!([]));
    }

    fn ledger_line(mut entry: Value) -> String {
        let object = entry.as_object_mut().unwrap();
        object.remove("hash");
        let canonical = serde_json::to_string(&entry).unwrap();
        let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        entry
            .as_object_mut()
            .unwrap()
            .insert("hash".to_string(), json!(hash));
        format!("{}\n", serde_json::to_string(&entry).unwrap())
    }
}
