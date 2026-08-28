use omegat_core::properties::{ProjectProperties, RepositoryDef, RepositoryMapping};
use omegat_team::{copy_mapped_from_worktree, CopyDir};
use serde_json::Value;
use std::path::Path;

fn golden(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens/remaining")
        .join(name);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        value["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens")
    );
    value
}

fn write_file(root: &Path, relative: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, relative).unwrap();
}

fn seed_primary(root: &Path) {
    for relative in [
        "omegat.project",
        ".git/gitstuff",
        "source/file1.txt",
        "source/file1.txt.bak",
        "source/subdir/file2.txt",
        "source/subdir/file2.txt.bak",
        "source/subdir/3.jpg",
        "source/subdir/4.png",
        "source/asubdir/subdir/3.jpg",
        "source/3.jpg",
        "source/4.png",
        "omegat/project_save.tmx",
        "glossary/sub/myglossary.txt",
    ] {
        write_file(root, relative);
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}

fn assert_copy_set(name: &str) {
    let spec = golden(name);
    let dir = tempfile::tempdir().unwrap();
    let project_root = dir.path().join("project");
    let primary = dir.path().join("primary");
    let secondary = dir.path().join("secondary");
    seed_primary(&primary);
    write_file(&secondary, "otherprojectfile.txt");

    let props = ProjectProperties::create(project_root, "en".into(), "fr".into(), false);
    props.ensure_dirs().unwrap();
    let excludes = strings(&spec["excludes"]);
    let primary_repo = RepositoryDef {
        repo_type: "file".into(),
        url: "primary".into(),
        branch: None,
        mappings: vec![RepositoryMapping {
            local: "/".into(),
            repository: "/".into(),
            includes: vec![],
            excludes: excludes.clone(),
        }],
    };
    let secondary_repo = RepositoryDef {
        repo_type: "file".into(),
        url: "secondary".into(),
        branch: None,
        mappings: vec![RepositoryMapping {
            local: "source/otherproject".into(),
            repository: "/".into(),
            includes: vec![],
            excludes,
        }],
    };

    let mut copied =
        copy_mapped_from_worktree(&props, &primary_repo, &primary, CopyDir::RepoToProject).unwrap();
    copied.extend(
        copy_mapped_from_worktree(&props, &secondary_repo, &secondary, CopyDir::RepoToProject)
            .unwrap(),
    );
    copied.sort();

    assert_eq!(copied, strings(&spec["copied"]));
}

#[test]
fn unanchored_excludes_match_java_copy_set() {
    assert_copy_set("RemoteRepositoryProviderTest-testCopyAllFromReposToProjectWithExcludes.json");
}

#[test]
fn slash_anchored_excludes_match_java_copy_set() {
    assert_copy_set("RemoteRepositoryProviderTest-testCopyAllFromReposToProjectWithSExcludes.json");
}
