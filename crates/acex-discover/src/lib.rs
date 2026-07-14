//! Discover drop-in acex packages and skills from the filesystem.
//!
//! # Philosophy
//! Pi-like **progressive disclosure**: scan returns name + description only.
//! Full package body is loaded via [`load_package`] when needed.
//!
//! # Locations (project root)
//! - `.acex/packages/<name>/acex-package.toml`
//! - `packages/<name>/acex-package.toml`
//! - `skills/<name>/SKILL.md` (Agent Skills frontmatter)
//!
//! No dynamic code loading — manifests only (idiomatic Rust).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from discovery / parse (expected failures — not panics).
#[derive(Debug, Error)]
pub enum DiscoverError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid package at {path}: {message}")]
    Invalid { path: PathBuf, message: String },
}

pub type Result<T> = std::result::Result<T, DiscoverError>;

/// Declared action metadata (maps to compile-time Intent ids when known).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageActionMeta {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Optional mapping to a known Intent variant name (e.g. `FocusSelected`).
    #[serde(default)]
    pub intent: Option<String>,
}

/// Full package manifest (`acex-package.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageManifest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub actions: Vec<PackageActionMeta>,
    /// Relative skill dirs/files contributed by this package.
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Lightweight package summary (progressive disclosure).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: PackageSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageSource {
    /// `.acex/packages/`
    AcexDot,
    /// `packages/`
    PackagesDir,
}

/// Skill summary from SKILL.md frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Full discovery result for a project root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiscoveryReport {
    pub packages: Vec<PackageSummary>,
    pub skills: Vec<SkillSummary>,
}

/// Scan `project_root` for packages and skills.
pub fn scan(project_root: impl AsRef<Path>) -> Result<DiscoveryReport> {
    let root = project_root.as_ref();
    let mut report = DiscoveryReport::default();

    scan_package_tree(
        root.join(".acex").join("packages"),
        PackageSource::AcexDot,
        &mut report.packages,
    )?;
    scan_package_tree(
        root.join("packages"),
        PackageSource::PackagesDir,
        &mut report.packages,
    )?;

    scan_skills_dir(root.join("skills"), &mut report.skills)?;
    // Pi also uses .agents/skills — optional project convention
    scan_skills_dir(root.join(".agents").join("skills"), &mut report.skills)?;

    report.packages.sort_by(|a, b| a.name.cmp(&b.name));
    report.skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(report)
}

fn scan_package_tree(
    dir: PathBuf,
    source: PackageSource,
    out: &mut Vec<PackageSummary>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("acex-package.toml");
        if !manifest_path.is_file() {
            continue;
        }
        match load_package_summary(&manifest_path, source) {
            Ok(s) => out.push(s),
            Err(e) => {
                // Skip invalid packages but keep scanning (Pi warns; we surface via empty skip).
                // For pure API: invalid package is an error only when loading that path.
                let _ = e;
            }
        }
    }
    Ok(())
}

/// Load summary from a manifest path (validates required fields).
pub fn load_package_summary(manifest_path: &Path, source: PackageSource) -> Result<PackageSummary> {
    let full = load_package(manifest_path)?;
    Ok(PackageSummary {
        name: full.name,
        description: full.description,
        path: manifest_path
            .parent()
            .unwrap_or(manifest_path)
            .to_path_buf(),
        source,
    })
}

/// Load full package manifest (progressive detail).
pub fn load_package(manifest_path: impl AsRef<Path>) -> Result<PackageManifest> {
    let path = manifest_path.as_ref();
    let text = fs::read_to_string(path)?;
    let m: PackageManifest = toml::from_str(&text)?;
    if m.name.trim().is_empty() {
        return Err(DiscoverError::Invalid {
            path: path.to_path_buf(),
            message: "name is required".into(),
        });
    }
    if m.description.trim().is_empty() {
        return Err(DiscoverError::Invalid {
            path: path.to_path_buf(),
            message: "description is required".into(),
        });
    }
    Ok(m)
}

fn scan_skills_dir(dir: PathBuf, out: &mut Vec<SkillSummary>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        if let Ok(s) = parse_skill_frontmatter(&skill_md) {
            out.push(s);
        }
    }
    Ok(())
}

/// Parse Agent Skills-style YAML frontmatter from SKILL.md.
pub fn parse_skill_frontmatter(path: impl AsRef<Path>) -> Result<SkillSummary> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let (name, description) =
        extract_frontmatter_name_desc(&text).ok_or_else(|| DiscoverError::Invalid {
            path: path.to_path_buf(),
            message: "missing name/description frontmatter".into(),
        })?;
    Ok(SkillSummary {
        name,
        description,
        path: path.to_path_buf(),
    })
}

fn extract_frontmatter_name_desc(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];
    let mut name = None;
    let mut description = None;
    let mut collecting_desc = false;
    let mut desc_buf = String::new();
    for line in yaml.lines() {
        let raw = line;
        let line = line.trim();
        if collecting_desc {
            // Folded/literal block continuation (indented) or stop on next key.
            if raw.starts_with(' ') || raw.starts_with('\t') {
                if !desc_buf.is_empty() {
                    desc_buf.push(' ');
                }
                desc_buf.push_str(line);
                continue;
            }
            collecting_desc = false;
            description = Some(desc_buf.trim().to_string());
            desc_buf.clear();
        }
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(unquote(v.trim()));
        } else if let Some(v) = line.strip_prefix("description:") {
            let v = v.trim();
            if v == ">" || v == "|" || v.is_empty() {
                collecting_desc = true;
                desc_buf.clear();
            } else {
                description = Some(unquote(v));
            }
        }
    }
    if collecting_desc && !desc_buf.is_empty() {
        description = Some(desc_buf.trim().to_string());
    }
    match (name, description) {
        (Some(n), Some(d)) if !n.is_empty() && !d.is_empty() && d != ">" && d != "|" => {
            Some((n, d))
        }
        _ => None,
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Stable JSON for machine consumers (`--status`).
pub fn report_to_json_value(report: &DiscoveryReport) -> serde_json::Value {
    serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn scan_finds_package_and_skill_fixtures() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // .acex/packages/demo
        let pkg = root.join(".acex").join("packages").join("demo");
        fs::create_dir_all(&pkg).unwrap();
        let mut f = fs::File::create(pkg.join("acex-package.toml")).unwrap();
        write!(
            f,
            r#"
name = "demo-pack"
description = "Demo package for discovery tests"
version = "0.1.0"

[[actions]]
id = "focus"
label = "Focus (from pack)"
intent = "FocusSelected"
"#
        )
        .unwrap();

        // skills/acex-dev
        let skill = root.join("skills").join("acex-dev");
        fs::create_dir_all(&skill).unwrap();
        let mut sf = fs::File::create(skill.join("SKILL.md")).unwrap();
        write!(
            sf,
            "---\nname: acex-dev\ndescription: Develop the acex control plane.\n---\n\n# body\n"
        )
        .unwrap();

        let report = scan(root).expect("scan");
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].name, "demo-pack");
        assert!(report.packages[0].description.contains("Demo package"));
        assert_eq!(report.packages[0].source, PackageSource::AcexDot);

        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].name, "acex-dev");
        assert!(report.skills[0].description.contains("control plane"));

        // Progressive detail
        let full = load_package(pkg.join("acex-package.toml")).unwrap();
        assert_eq!(full.actions.len(), 1);
        assert_eq!(full.actions[0].intent.as_deref(), Some("FocusSelected"));
    }

    #[test]
    fn scan_empty_root_is_empty_not_error() {
        let dir = tempdir().unwrap();
        let report = scan(dir.path()).unwrap();
        assert!(report.packages.is_empty());
        assert!(report.skills.is_empty());
    }

    #[test]
    fn packages_dir_source() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("packages").join("other");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("acex-package.toml"),
            "name = \"other\"\ndescription = \"From packages/\"\n",
        )
        .unwrap();
        let report = scan(dir.path()).unwrap();
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].source, PackageSource::PackagesDir);
    }

    #[test]
    fn invalid_manifest_missing_description_fails_load() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("acex-package.toml");
        fs::write(&p, "name = \"x\"\n").unwrap();
        let err = load_package(&p).unwrap_err();
        // Missing required field → Toml; empty description after parse → Invalid.
        assert!(
            matches!(err, DiscoverError::Toml(_) | DiscoverError::Invalid { .. }),
            "got {err:?}"
        );
        let p2 = dir.path().join("empty-desc.toml");
        fs::write(&p2, "name = \"x\"\ndescription = \"\"\n").unwrap();
        let err2 = load_package(&p2).unwrap_err();
        assert!(
            matches!(err2, DiscoverError::Invalid { .. }),
            "got {err2:?}"
        );
    }
}
