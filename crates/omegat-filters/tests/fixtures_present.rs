//! R0: Java filter fixtures must exist and be referenced by the rewrite tree.

use std::path::PathBuf;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/filters")
}

#[test]
fn java_filter_fixture_tree_is_present() {
    let root = fixtures_root();
    assert!(root.is_dir(), "missing {}", root.display());
    let mut files = 0usize;
    let mut dirs = 0usize;
    fn walk(dir: &std::path::Path, depth: usize, files: &mut usize, dirs: &mut usize) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                if depth == 0 {
                    *dirs += 1;
                }
                walk(&path, depth + 1, files, dirs);
            } else if path.is_file() {
                *files += 1;
            }
        }
    }
    walk(&root, 0, &mut files, &mut dirs);
    assert!(
        files >= 140,
        "expected the Java filter corpus (≥140 files), found {files}"
    );
    assert!(
        dirs >= 20,
        "expected format subdirectories from Java tests, found {dirs}"
    );
}

#[test]
fn srx_and_align_fixtures_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    assert!(root.join("srx/defaultRules.srx").is_file());
    assert!(root.join("srx/segmentation.srx").is_file());
    assert!(root.join("align").is_dir());
}
