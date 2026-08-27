//! G1: `translate` / `stats` / `pseudo` / legacy `--mode` on a real project.

use omegat_core::prefs::Preferences;
use omegat_core::session::ProjectSession;
use serde_json::Value;
use std::path::Path;

fn golden(relative: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens")
        .join(relative);
    let value: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        value["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens")
    );
    value
}

fn golden_argv(value: &Value) -> Vec<String> {
    value["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap().to_string())
        .collect()
}

fn run_argv(args: &[String]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_omegat"))
        .args(args)
        .output()
        .unwrap()
}

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

#[test]
fn java_restart_and_common_argv_reach_the_real_parser() {
    for relative in [
        "cli/MainTest#testConstructCommandParamsRoundTrip.json",
        "cli/MainTest#testConstructCommandParamsKeepsRuntimeOptions.json",
        "cli/CommandCommonTest#testParseCommonParamsAppliesSubCommandOptions.json",
        "cli/CommandCommonTest#testParseCommonParamsPositiveTeamKeepsDefault.json",
        "cli/CommandCommonTest#testParseCommonParamsDefaultsLeaveStoreUntouched.json",
        "cli/LegacyParametersTest#testInitializeAppliesConfigDir.json",
        "cli/LegacyParametersTest#testInitializeExpandsTilde.json",
        "cli/LegacyParametersTest#testInitializeWithoutConfigDir.json",
        "cli/LegacyParametersTest#testInitializeAppliesRuntimeFlags.json",
        "cli/LegacyParametersTest#testInitializeLoadsResourceBundle.json",
    ] {
        let spec = golden(relative);
        let output = run_argv(&golden_argv(&spec));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{relative}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let project = golden("cli/MainTest#testConstructCommandParamsProjectAfterOptions.json");
    let mut prefs = omegat_core::cli_params::RuntimePrefs::default();
    prefs.config_dir = project["config_dir"].as_str().map(str::to_string);
    let mut argv = omegat_core::cli_params::construct_command_params(&prefs);
    argv.push(project["project"].as_str().unwrap().to_string());
    let output = run_argv(&argv);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last(),
        Some(format!("Project: {}", project["project"].as_str().unwrap()).as_str())
    );

    let absent = golden("cli/MainTest#testExtractConfigDirAbsent.json");
    let output = run_argv(&["--config-dir=".to_string()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        omegat_core::cli_params::initialize_legacy(&["--config-dir="])
            .config_dir
            .is_some(),
        absent["present"].as_bool().unwrap()
    );
}
