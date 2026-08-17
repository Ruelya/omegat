//! G1: `translate` / `stats` / `pseudo` / legacy `--mode` on a real project.

use omegat_core::prefs::Preferences;
use omegat_core::session::ProjectSession;

#[test]
fn translate_stats_pseudo_and_legacy_mode() {
    let root = std::env::temp_dir().join(format!("omegat-cli-g1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let prefs = Preferences::default_in(root.join("cfg"));
    let mut session = ProjectSession::create(
        &omegat_ipc::CreateProjectParams {
            root: root.to_string_lossy().into(),
            source_lang: "en".into(),
            target_lang: "fr".into(),
            sentence_seg: false,
        },
        prefs,
    )
    .unwrap();
    std::fs::write(session.props.source_dir.join("a.txt"), "Hello world").unwrap();
    session.reload().unwrap();
    session.entries[0].translation = "Bonjour".into();
    session.save().unwrap();
    drop(session);

    let bin = env!("CARGO_BIN_EXE_omegat");
    let translate = std::process::Command::new(bin)
        .args(["translate", &root.to_string_lossy(), "--quiet"])
        .output()
        .unwrap();
    assert!(
        translate.status.success(),
        "translate: {}",
        String::from_utf8_lossy(&translate.stderr)
    );
    let target = std::fs::read_to_string(root.join("target/a.txt")).unwrap();
    assert_eq!(target.trim(), "Bonjour");

    let stats = std::process::Command::new(bin)
        .args(["stats", &root.to_string_lossy(), "json"])
        .output()
        .unwrap();
    assert!(
        stats.status.success(),
        "stats: {}",
        String::from_utf8_lossy(&stats.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&stats.stdout).unwrap();
    assert_eq!(v["segments"], 1);
    assert_eq!(v["translated"], 1);

    let dest = root.join("pseudo.tmx");
    let pseudo = std::process::Command::new(bin)
        .args([
            "pseudo",
            &root.to_string_lossy(),
            "--type",
            "equal",
            "--output-file",
            &dest.to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert!(
        pseudo.status.success(),
        "pseudo: {}",
        String::from_utf8_lossy(&pseudo.stderr)
    );
    assert!(dest.is_file());

    let mode = std::process::Command::new(bin)
        .args(["--mode", "console-stats", &root.to_string_lossy()])
        .output()
        .unwrap();
    assert!(
        mode.status.success(),
        "legacy --mode: {}",
        String::from_utf8_lossy(&mode.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
}
