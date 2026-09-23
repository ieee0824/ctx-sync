//! Findings from manifest files: `Cargo.toml`, `package.json`, `go.mod`,
//! `pyproject.toml`.
//!
//! Facts read from a manifest are `Confirmed`. What a dependency suggests
//! (e.g. tokio → async runtime) is only `Inferred`. Broken manifests are
//! ignored.

use std::path::Path;

use super::Finding;

const MAX_DEPENDENCIES: usize = 20;

pub fn collect_manifests(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(cargo(root));
    findings.extend(package_json(root));
    findings.extend(go_mod(root));
    findings.extend(pyproject(root));
    findings
}

/// What a dependency suggests: `(topic, text)`.
pub fn infer_from_dependency(name: &str) -> Option<(&'static str, String)> {
    let name = name.to_ascii_lowercase();
    let inference = match name.as_str() {
        "tokio" => (
            "Async runtime",
            "Tokio appears to be used as the async runtime.".to_string(),
        ),
        "async-std" => (
            "Async runtime",
            "async-std appears to be used as the async runtime.".to_string(),
        ),
        "axum" | "actix-web" | "warp" | "rocket" | "django" | "flask" | "fastapi" => (
            "Web framework",
            format!("{name} appears to be used as the web framework."),
        ),
        "clap" => (
            "Interface",
            "This appears to be a CLI application (clap).".to_string(),
        ),
        "sqlx" | "diesel" | "sea-orm" => (
            "Database",
            format!("{name} appears to be used for database access."),
        ),
        "reqwest" => (
            "HTTP client",
            "reqwest appears to be used as the HTTP client.".to_string(),
        ),
        "react" | "vue" | "svelte" => (
            "UI framework",
            format!("{name} appears to be used for the UI."),
        ),
        "next" => ("Web framework", "Next.js appears to be used.".to_string()),
        "express" | "fastify" => (
            "Web framework",
            format!("{name} appears to be used as the HTTP server."),
        ),
        "jest" | "vitest" => ("Testing", format!("{name} appears to be used for tests.")),
        _ => return None,
    };
    Some(inference)
}

fn read(root: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(root.join(file)).ok()
}

/// `Dependencies: a, b, ...` (first 20) plus one inference per dependency.
fn dependency_findings(names: &[String], source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !names.is_empty() {
        let shown: Vec<&str> = names
            .iter()
            .take(MAX_DEPENDENCIES)
            .map(String::as_str)
            .collect();
        findings.push(Finding::confirmed(
            "Dependencies",
            &shown.join(", "),
            Some(source),
        ));
    }
    let mut inferred: Vec<(&str, String)> = Vec::new();
    for name in names {
        if let Some(inference) = infer_from_dependency(name)
            && !inferred.contains(&inference)
        {
            inferred.push(inference);
        }
    }
    findings.extend(
        inferred
            .into_iter()
            .map(|(topic, text)| Finding::inferred(topic, &text, Some(source))),
    );
    findings
}

fn table_keys(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_table)
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

fn cargo(root: &Path) -> Vec<Finding> {
    const SOURCE: &str = "Cargo.toml";
    let Some(value) = read(root, SOURCE).and_then(|t| t.parse::<toml::Table>().ok()) else {
        return Vec::new();
    };
    let mut findings = vec![Finding::confirmed("Language", "Rust", Some(SOURCE))];
    if let Some(name) = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
    {
        findings.push(Finding::confirmed("Package", name, Some(SOURCE)));
    }
    let workspace = value.get("workspace");
    if let Some(members) = workspace
        .and_then(|w| w.get("members"))
        .and_then(toml::Value::as_array)
    {
        let members: Vec<&str> = members.iter().filter_map(toml::Value::as_str).collect();
        if !members.is_empty() {
            findings.push(Finding::confirmed(
                "Workspace members",
                &members.join(", "),
                Some(SOURCE),
            ));
        }
    }
    let mut kinds = Vec::new();
    if root.join("src/main.rs").is_file() || value.contains_key("bin") {
        kinds.push("binary");
    }
    if root.join("src/lib.rs").is_file() {
        kinds.push("library");
    }
    if !kinds.is_empty() {
        findings.push(Finding::confirmed(
            "Crate type",
            &kinds.join(", "),
            Some(SOURCE),
        ));
    }
    let mut deps = table_keys(value.get("dependencies"));
    if deps.is_empty() {
        deps = table_keys(workspace.and_then(|w| w.get("dependencies")));
    }
    findings.extend(dependency_findings(&deps, SOURCE));
    findings
}

fn package_json(root: &Path) -> Vec<Finding> {
    const SOURCE: &str = "package.json";
    let Some(value) =
        read(root, SOURCE).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return Vec::new();
    };
    let keys = |field: &str| -> Vec<String> {
        value
            .get(field)
            .and_then(serde_json::Value::as_object)
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default()
    };
    let mut deps = keys("dependencies");
    deps.extend(keys("devDependencies"));

    let typescript = deps.iter().any(|d| d == "typescript") || root.join("tsconfig.json").is_file();
    let language = if typescript {
        "TypeScript"
    } else {
        "JavaScript"
    };
    let mut findings = vec![Finding::confirmed("Language", language, Some(SOURCE))];
    if let Some(name) = value.get("name").and_then(serde_json::Value::as_str) {
        findings.push(Finding::confirmed("Package", name, Some(SOURCE)));
    }
    let scripts = keys("scripts");
    if !scripts.is_empty() {
        findings.push(Finding::confirmed(
            "Scripts",
            &scripts.join(", "),
            Some(SOURCE),
        ));
    }
    findings.extend(dependency_findings(&deps, SOURCE));
    findings
}

fn go_mod(root: &Path) -> Vec<Finding> {
    const SOURCE: &str = "go.mod";
    let Some(text) = read(root, SOURCE) else {
        return Vec::new();
    };
    let mut findings = vec![Finding::confirmed("Language", "Go", Some(SOURCE))];
    for line in text.lines().map(str::trim) {
        if let Some(module) = line.strip_prefix("module ") {
            findings.push(Finding::confirmed("Module", module.trim(), Some(SOURCE)));
        } else if let Some(version) = line.strip_prefix("go ") {
            findings.push(Finding::confirmed(
                "Go version",
                version.trim(),
                Some(SOURCE),
            ));
        }
    }
    findings
}

fn pyproject(root: &Path) -> Vec<Finding> {
    const SOURCE: &str = "pyproject.toml";
    let Some(value) = read(root, SOURCE).and_then(|t| t.parse::<toml::Table>().ok()) else {
        return Vec::new();
    };
    let project = value.get("project");
    let mut findings = vec![Finding::confirmed("Language", "Python", Some(SOURCE))];
    if let Some(name) = project
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
    {
        findings.push(Finding::confirmed("Package", name, Some(SOURCE)));
    }
    let deps: Vec<String> = project
        .and_then(|p| p.get("dependencies"))
        .and_then(toml::Value::as_array)
        .map(|deps| {
            deps.iter()
                .filter_map(toml::Value::as_str)
                .filter_map(|spec| {
                    // "fastapi>=0.100" -> "fastapi"
                    let name = spec
                        .split(|c: char| {
                            !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
                        })
                        .next()?;
                    (!name.is_empty()).then(|| name.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    findings.extend(dependency_findings(&deps, SOURCE));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::Certainty;

    fn write(root: &Path, file: &str, text: &str) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn texts(findings: &[Finding], certainty: Certainty) -> Vec<String> {
        findings
            .iter()
            .filter(|f| f.certainty == certainty)
            .map(|f| format!("{}: {}", f.topic, f.text))
            .collect()
    }

    #[test]
    fn rust_binary_crate() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\ntokio = \"1\"\nclap = \"4\"\nserde = \"1\"\n",
        );
        write(dir.path(), "src/main.rs", "fn main() {}\n");
        let findings = collect_manifests(dir.path());
        assert_eq!(
            texts(&findings, Certainty::Confirmed),
            [
                "Language: Rust",
                "Package: demo",
                "Crate type: binary",
                "Dependencies: clap, serde, tokio",
            ]
        );
        assert_eq!(
            texts(&findings, Certainty::Inferred),
            [
                "Interface: This appears to be a CLI application (clap).",
                "Async runtime: Tokio appears to be used as the async runtime.",
            ]
        );
        assert!(
            findings
                .iter()
                .all(|f| f.source.as_deref() == Some("Cargo.toml"))
        );
    }

    #[test]
    fn rust_workspace_uses_workspace_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n\n[workspace.dependencies]\nreqwest = \"0.12\"\n",
        );
        let findings = collect_manifests(dir.path());
        let confirmed = texts(&findings, Certainty::Confirmed);
        assert!(confirmed.contains(&"Workspace members: crates/a, crates/b".to_string()));
        assert!(confirmed.contains(&"Dependencies: reqwest".to_string()));
        assert_eq!(
            texts(&findings, Certainty::Inferred),
            ["HTTP client: reqwest appears to be used as the HTTP client."]
        );
    }

    #[test]
    fn typescript_package() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"name": "web", "scripts": {"build": "tsc", "test": "jest"},
                "dependencies": {"react": "^18"},
                "devDependencies": {"typescript": "^5", "jest": "^29"}}"#,
        );
        let findings = collect_manifests(dir.path());
        let confirmed = texts(&findings, Certainty::Confirmed);
        assert_eq!(confirmed[0], "Language: TypeScript");
        assert!(confirmed.contains(&"Package: web".to_string()));
        assert!(confirmed.contains(&"Scripts: build, test".to_string()));
        assert_eq!(
            texts(&findings, Certainty::Inferred),
            [
                "UI framework: react appears to be used for the UI.",
                "Testing: jest appears to be used for tests.",
            ]
        );
    }

    #[test]
    fn javascript_without_typescript() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"dependencies": {"express": "^4"}}"#,
        );
        let findings = collect_manifests(dir.path());
        assert_eq!(
            texts(&findings, Certainty::Confirmed)[0],
            "Language: JavaScript"
        );
        assert_eq!(
            texts(&findings, Certainty::Inferred),
            ["Web framework: express appears to be used as the HTTP server."]
        );
    }

    #[test]
    fn go_and_python() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "go.mod", "module example.com/demo\n\ngo 1.22\n");
        write(
            dir.path(),
            "pyproject.toml",
            "[project]\nname = \"api\"\ndependencies = [\"fastapi>=0.100\", \"pydantic\"]\n",
        );
        let confirmed = texts(&collect_manifests(dir.path()), Certainty::Confirmed);
        assert_eq!(
            confirmed,
            [
                "Language: Go",
                "Module: example.com/demo",
                "Go version: 1.22",
                "Language: Python",
                "Package: api",
                "Dependencies: fastapi, pydantic",
            ]
        );
    }

    #[test]
    fn empty_and_broken_manifests_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_manifests(dir.path()).is_empty());
        write(dir.path(), "Cargo.toml", "[package\nname = ");
        write(dir.path(), "package.json", "{");
        assert!(collect_manifests(dir.path()).is_empty());
    }
}
