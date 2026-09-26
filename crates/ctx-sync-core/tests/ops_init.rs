use std::path::{Path, PathBuf};

use ctx_sync_core::Error;
use ctx_sync_core::clock::parse_now;
use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_core::model::{ContextSnapshot, Meta};
use ctx_sync_core::ops::{InitOptions, InitRemote, Runtime, init};
use ctx_sync_core::state::{Index, LocalProject, Protocol, StateRoot};
use ctx_sync_testutil::{FIXED_NOW, TestRemote};

struct Fixture {
    remote: TestRemote,
    _dir: tempfile::TempDir,
    project: PathBuf,
    rt: Runtime,
}

fn fixture() -> Fixture {
    let remote = TestRemote::new();
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let rt = Runtime {
        state_root: StateRoot::new(dir.path().join("state")),
        remote_url_override: Some(remote.url()),
        git_env: remote.git_env(),
    };
    Fixture {
        remote,
        _dir: dir,
        project,
        rt,
    }
}

fn options(project: &Path, name: &str, force: bool) -> InitOptions {
    InitOptions {
        project_root: project.to_path_buf(),
        project_name: name.into(),
        remote: InitRemote::ExistingGist {
            gist_id: "testgist01".into(),
        },
        protocol: Protocol::Https,
        force,
        context_files: ctx_sync_core::config::ContextFiles::default(),
        now: parse_now(Some(FIXED_NOW)).unwrap(),
    }
}

fn assert_invalid(result: ctx_sync_core::Result<ctx_sync_core::ops::InitOutcome>, needle: &str) {
    match result {
        Err(e @ Error::InvalidConfig(_)) => assert!(e.to_string().contains(needle), "{e}"),
        Err(e) => panic!("unexpected error {e:?}"),
        Ok(_) => panic!("expected an error"),
    }
}

#[test]
fn initializes_an_existing_gist() {
    let f = fixture();
    let outcome = init(&f.rt, options(&f.project, "demo", false)).unwrap();
    assert_eq!(outcome.project_name, "demo");
    assert_eq!(outcome.gist_id, "testgist01");
    assert_eq!(outcome.config_path, f.project.join(CONFIG_FILE));

    // The initial context is pushed.
    let meta = Meta::parse(&f.remote.read_file("00-meta.json").unwrap()).unwrap();
    assert_eq!(meta.project_id, outcome.project_id);
    assert!(
        f.remote
            .read_file("10-project.md")
            .unwrap()
            .contains("demo")
    );
    assert!(f.remote.read_file("20-architecture.md").is_some());
    assert_eq!(f.remote.log()[0], "ctx-sync: init demo");

    // Config and local state are written.
    let config = ProjectConfig::load(&f.project.join(CONFIG_FILE)).unwrap();
    assert_eq!(config.remote.id, "testgist01");
    assert_eq!(
        Index::load(&f.rt.state_root).unwrap().get("testgist01"),
        Some(outcome.project_id)
    );
    let state = f.rt.state_root.project(&outcome.project_id);
    assert!(LocalProject::load(&state.local_json()).unwrap().is_some());
    let snapshot = ContextSnapshot::load(&state.context_repo()).unwrap();
    assert!(snapshot.warnings.is_empty(), "{:?}", snapshot.warnings);
}

#[test]
fn existing_config_needs_force() {
    let f = fixture();
    init(&f.rt, options(&f.project, "demo", false)).unwrap();
    assert_invalid(init(&f.rt, options(&f.project, "demo", false)), "--force");
}

#[test]
fn gist_with_a_context_is_rejected_even_with_force() {
    let f = fixture();
    init(&f.rt, options(&f.project, "demo", false)).unwrap();
    let projects_before = std::fs::read_dir(f.rt.state_root.root().join("projects"))
        .unwrap()
        .count();
    assert_invalid(
        init(&f.rt, options(&f.project, "demo", true)),
        "ctx-sync attach testgist01",
    );
    // The clone made for the failed attempt is removed.
    let projects_after = std::fs::read_dir(f.rt.state_root.root().join("projects"))
        .unwrap()
        .count();
    assert_eq!(projects_after, projects_before);
}

#[test]
fn blank_project_name_is_rejected() {
    let f = fixture();
    assert_invalid(
        init(&f.rt, options(&f.project, "   ", false)),
        "project name",
    );
    assert!(!f.project.join(CONFIG_FILE).exists());
}
