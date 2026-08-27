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
        .env("OMEGAT_LAUNCH_DRY_RUN", "1")
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

#[cfg(unix)]
#[test]
fn start_launches_desktop_with_config_project_and_scripts_context() {
    use std::os::unix::fs::PermissionsExt;

    let temp = std::env::temp_dir().join(format!("omegat-cli-launch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let config = temp.join("config");
    let scripts = temp.join("named-scripts");
    let project = temp.join("project");
    let capture = temp.join("launch.txt");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    let properties = temp.join("omegat.properties");
    std::fs::write(
        &properties,
        format!(
            "user.language=pt\nuser.country=BR\nscripts_dir={}\n",
            scripts.display()
        ),
    )
    .unwrap();
    let launcher = temp.join("desktop-launcher");
    std::fs::write(
        &launcher,
        "#!/bin/sh\nprintf '%s\\n%s\\n%s\\n%s\\n' \"$OMEGAT_CONFIG_DIR\" \"$OMEGAT_PROJECT\" \"$OMEGAT_SCRIPTS_DIR\" \"$OMEGAT_LOCALE\" > \"$OMEGAT_CAPTURE\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&launcher).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&launcher, permissions).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_omegat"))
        .args([
            "--config-dir",
            config.to_str().unwrap(),
            "--config-file",
            properties.to_str().unwrap(),
            "start",
            project.to_str().unwrap(),
            "--quiet",
        ])
        .env("OMEGAT_DESKTOP_BIN", &launcher)
        .env("OMEGAT_CAPTURE", &capture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for _ in 0..100 {
        if capture.is_file() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read_to_string(capture).unwrap(),
        format!(
            "{}\n{}\n{}\npt-BR\n",
            config.display(),
            project.display(),
            scripts.display(),
        )
    );
    let _ = std::fs::remove_dir_all(temp);
}
