use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use omegat_core::error::CoreError;
use omegat_core::prefs::{default_config_dir, Preferences};
use omegat_core::session::ProjectSession;
use omegat_ipc::SearchParams;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser, Debug)]
#[command(name = "omegat", version = omegat_ipc::APP_VERSION, about = "OmegaT computer-assisted translation")]
struct Cli {
    /// Project directory (default: current directory)
    project: Option<PathBuf>,
    /// Java `--config-dir`
    #[arg(long)]
    config_dir: Option<PathBuf>,
    /// Java `--config-file`
    #[arg(long)]
    config_file: Option<PathBuf>,
    /// Java `--resource-bundle`
    #[arg(long)]
    resource_bundle: Option<PathBuf>,
    /// Java `--no-team`
    #[arg(long)]
    no_team: bool,
    /// Java `--disable-project-locking`
    #[arg(long)]
    disable_project_locking: bool,
    /// Java `--disable-location-save`
    #[arg(long)]
    disable_location_save: bool,
    /// Legacy console mode: console-translate | console-stats | console-createpseudotranslatetmx | console-align
    #[arg(long)]
    mode: Option<String>,
    /// Java `--source-pattern`
    #[arg(long)]
    source_pattern: Option<String>,
    /// Java `--pseudotranslatetmx`
    #[arg(long)]
    pseudotranslatetmx: Option<PathBuf>,
    /// Java `--pseudotranslatetype` (`equal` or `empty`)
    #[arg(long)]
    pseudotranslatetype: Option<String>,
    /// Java `--alignDir` (legacy `--mode console-align`)
    #[arg(long = "alignDir")]
    align_dir: Option<PathBuf>,
    /// Java `--output-file`
    #[arg(long = "output-file")]
    output_file: Option<PathBuf>,
    /// Java `--stats-type`
    #[arg(long = "stats-type")]
    stats_type: Option<String>,
    /// Java `--script`
    #[arg(long)]
    script: Option<PathBuf>,
    /// Java `--tag-validation` (`abort` or `warn`)
    #[arg(long = "tag-validation")]
    tag_validation: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        project: Option<PathBuf>,
        /// Java common-parameter quiet mode
        #[arg(long)]
        quiet: bool,
        /// Java common-parameter project-lock switch
        #[arg(long = "no-project-locking")]
        no_project_locking: bool,
        /// Java common-parameter location-save switch
        #[arg(long = "no-location-save")]
        no_location_save: bool,
        /// Java negatable team option
        #[arg(long = "no-team")]
        no_team: bool,
        /// Explicit positive form of the Java negatable team option
        #[arg(long)]
        team: bool,
        /// Java source tokenizer override
        #[arg(long = "ITokenizer")]
        tokenizer_source: Option<String>,
        /// Java target tokenizer override
        #[arg(long = "ITokenizerTarget")]
        tokenizer_target: Option<String>,
        /// Java alternate source filename pattern
        #[arg(long = "alternate-filename-from")]
        alternate_filename_from: Option<String>,
        /// Java alternate target filename pattern
        #[arg(long = "alternate-filename-to")]
        alternate_filename_to: Option<String>,
    },
    Translate {
        project: Option<PathBuf>,
        #[arg(long)]
        source_pattern: Option<String>,
        #[arg(long)]
        quiet: bool,
        #[arg(long = "tag-validation")]
        tag_validation: Option<String>,
        #[arg(long)]
        script: Option<PathBuf>,
    },
    Stats {
        project: Option<PathBuf>,
        #[arg(default_value = "text")]
        r#type: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Pseudo {
        project: Option<PathBuf>,
        #[arg(long, default_value = "equal")]
        r#type: String,
        #[arg(long)]
        output_file: Option<PathBuf>,
    },
    Team {
        #[command(subcommand)]
        cmd: TeamCmd,
    },
    Align {
        source: PathBuf,
        target: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "en")]
        source_lang: String,
        #[arg(long, default_value = "fr")]
        target_lang: String,
        #[arg(long, default_value = "parsewise")]
        mode: String,
        #[arg(long, default_value = "viterbi")]
        algo: String,
        #[arg(long, default_value = "word")]
        counter: String,
        #[arg(long, default_value = "normal")]
        calculator: String,
    },
    Script {
        source: PathBuf,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    Wiki {
        source: PathBuf,
        #[arg(long)]
        dest: PathBuf,
    },
    Convert {
        source: PathBuf,
        dest: PathBuf,
        #[arg(long, default_value = "en")]
        source_lang: String,
        #[arg(long, default_value = "fr")]
        target_lang: String,
    },
    Search {
        project: Option<PathBuf>,
        query: String,
        #[arg(long)]
        regex: bool,
    },
}

#[derive(Subcommand, Debug)]
enum TeamCmd {
    Init {
        source: String,
        target: String,
        dir: Option<PathBuf>,
    },
    Sync {
        project: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse_from(normalize_empty_config_dir(std::env::args_os()));
    if cli.no_team {
        std::env::set_var("OMEGAT_NO_TEAM", "1");
    }
    if let Some(dir) = cli
        .config_dir
        .as_ref()
        .filter(|dir| !dir.as_os_str().is_empty())
    {
        std::env::set_var("OMEGAT_CONFIG_DIR", expand_cli_path(dir));
    }
    if let Some(f) = &cli.config_file {
        let path = expand_cli_path(f);
        std::env::set_var("OMEGAT_CONFIG_FILE", &path);
        apply_config_file(&path)?;
    }
    if let Some(b) = &cli.resource_bundle {
        std::env::set_var("OMEGAT_RESOURCE_BUNDLE", expand_cli_path(b));
    }
    if cli.disable_location_save {
        std::env::set_var("OMEGAT_DISABLE_LOCATION_SAVE", "1");
    }
    if cli.disable_project_locking {
        std::env::set_var("OMEGAT_DISABLE_PROJECT_LOCKING", "1");
    }
    if let Some(mode) = &cli.mode {
        return legacy_mode(mode, &cli);
    }
    match cli.command.unwrap_or(Commands::Start {
        project: cli.project.clone(),
        quiet: false,
        no_project_locking: false,
        no_location_save: false,
        no_team: false,
        team: false,
        tokenizer_source: None,
        tokenizer_target: None,
        alternate_filename_from: None,
        alternate_filename_to: None,
    }) {
        Commands::Start {
            project,
            quiet,
            no_project_locking,
            no_location_save,
            no_team,
            team,
            tokenizer_source,
            tokenizer_target,
            alternate_filename_from,
            alternate_filename_to,
        } => {
            if no_project_locking {
                std::env::set_var("OMEGAT_DISABLE_PROJECT_LOCKING", "1");
            }
            if no_location_save {
                std::env::set_var("OMEGAT_DISABLE_LOCATION_SAVE", "1");
            }
            if no_team {
                std::env::set_var("OMEGAT_NO_TEAM", "1");
            } else if team {
                std::env::remove_var("OMEGAT_NO_TEAM");
            }
            if let Some(value) = tokenizer_source {
                std::env::set_var("OMEGAT_TOKENIZER_SOURCE", value);
            }
            if let Some(value) = tokenizer_target {
                std::env::set_var("OMEGAT_TOKENIZER_TARGET", value);
            }
            if let Some(value) = alternate_filename_from {
                std::env::set_var("OMEGAT_ALTERNATE_FILENAME_FROM", value);
            }
            if let Some(value) = alternate_filename_to {
                std::env::set_var("OMEGAT_ALTERNATE_FILENAME_TO", value);
            }
            launch_desktop(project.or(cli.project), quiet)
        }
        Commands::Translate {
            project,
            source_pattern,
            quiet,
            tag_validation,
            script,
        } => {
            let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
            let prefs = Preferences::load_or_default(&default_config_dir());
            let mut session = ProjectSession::open(&root, prefs)?;
            if let Some(mode) = tag_validation.as_deref().or(cli.tag_validation.as_deref()) {
                apply_tag_validation(&mut session, mode);
            }
            let n = compile_reporting(
                &mut session,
                source_pattern.as_deref().or(cli.source_pattern.as_deref()),
            )?;
            let script_path = script.or(cli.script.clone());
            if let Some(script) = script_path {
                let src = std::fs::read_to_string(script)?;
                let mut state = script_state_from_session(&session, 0);
                let _ = omegat_script::run_source_state(&src, &mut state)?;
            }
            if !quiet {
                println!("Compiled {n} file(s).");
            }
            Ok(())
        }
        Commands::Stats {
            project,
            r#type,
            output,
        } => {
            let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
            let prefs = Preferences::load_or_default(&default_config_dir());
            let session = ProjectSession::open(&root, prefs)?;
            let stats = session.stats();
            let kind = if r#type != "text" {
                r#type
            } else {
                cli.stats_type.clone().unwrap_or(r#type)
            };
            let text = omegat_core::stats::render(&stats, &kind);
            if let Some(p) = output.or(cli.output_file.clone()) {
                std::fs::write(p, &text)?;
            } else {
                print!("{text}");
            }
            Ok(())
        }
        Commands::Pseudo {
            project,
            r#type,
            output_file,
        } => {
            let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
            let prefs = Preferences::load_or_default(&default_config_dir());
            let session = ProjectSession::open(&root, prefs)?;
            let mut tmx = omegat_core::tmx::ProjectTmx::new();
            for e in &session.entries {
                let kind = cli.pseudotranslatetype.as_deref().unwrap_or(&r#type);
                let translation = if kind == "empty" {
                    String::new()
                } else {
                    e.source.clone()
                };
                tmx.insert(omegat_core::tmx::TmxEntry {
                    source: e.source.clone(),
                    translation,
                    ..Default::default()
                });
            }
            let dest = output_file
                .or(cli.pseudotranslatetmx.clone())
                .unwrap_or_else(|| root.join("pseudo.tmx"));
            tmx.write(&dest, &session.props.source_lang, &session.props.target_lang)?;
            println!("Wrote {}", dest.display());
            Ok(())
        }
        Commands::Team { cmd } => match cmd {
            TeamCmd::Init { source, target, dir } => {
                let dir = dir.unwrap_or_else(|| PathBuf::from("."));
                omegat_team::init(&dir, &source, &target)?;
                println!("Initialized team project in {}", dir.display());
                Ok(())
            }
            TeamCmd::Sync { project } => {
                let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
                let props = omegat_core::ProjectProperties::load(&root)?;
                let r = omegat_team::sync(&props)?;
                println!("{}: {}", r.action, r.message);
                Ok(())
            }
        },
        Commands::Align {
            source,
            target,
            output,
            source_lang,
            target_lang,
            mode,
            algo,
            counter,
            calculator,
        } => {
            let cfg = align_cfg(&mode, &algo, &counter, &calculator);
            let tmx = omegat_core::align::align_files_cfg(&source, &target, &source_lang, &target_lang, &cfg)?;
            omegat_core::align::write_aligned_tmx(&tmx, &output, &source_lang, &target_lang)?;
            println!("Aligned TMX written to {}", output.display());
            Ok(())
        }
        Commands::Script { source, project } => {
            let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
            let src = std::fs::read_to_string(&source)?;
            let mut state = if root.join("omegat.project").exists() {
                let session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
                script_state_from_session(&session, 0)
            } else {
                omegat_script::ScriptState::default()
            };
            let out = omegat_script::run_source_state(&src, &mut state)?;
            if !out.is_empty() {
                println!("{out}");
            }
            for line in &state.console {
                println!("{line}");
            }
            Ok(())
        }
        Commands::Wiki { source, dest } => {
            let n = omegat_core::wiki::import_wiki(&source, &dest)?;
            println!("Imported {n} wiki file(s).");
            Ok(())
        }
        Commands::Convert {
            source,
            dest,
            source_lang,
            target_lang,
        } => {
            omegat_core::wiki::convert_project(&source, &dest, &source_lang, &target_lang)?;
            println!("Converted project to {}", dest.display());
            Ok(())
        }
        Commands::Search {
            project,
            query,
            regex,
        } => {
            let root = project.or(cli.project).unwrap_or_else(|| PathBuf::from("."));
            let session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
            let hits = session.search(&SearchParams {
                query,
                regex,
                ..Default::default()
            });
            for h in hits {
                println!("#{} {} [{}] {}", h.index, h.file, h.field, h.text);
            }
            Ok(())
        }
    }
}

fn expand_cli_path(path: &Path) -> PathBuf {
    PathBuf::from(omegat_core::file_util::expand_tilde_home_dir(
        &path.to_string_lossy(),
    ))
}

/// Apply Java `.properties` startup values that must exist before Electron
/// starts. The full path remains in `OMEGAT_CONFIG_FILE` for diagnostics.
fn apply_config_file(path: &Path) -> Result<()> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    let mut language = None;
    let mut country = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "user.language" => language = Some(value.to_string()),
            "user.country" => country = Some(value.to_string()),
            "scripts_dir" if !value.is_empty() => {
                std::env::set_var(
                    "OMEGAT_SCRIPTS_DIR",
                    omegat_core::file_util::expand_tilde_home_dir(value),
                );
            }
            _ => {}
        }
    }
    if let Some(mut locale) = language {
        if let Some(country) = country.filter(|value| !value.is_empty()) {
            locale.push('-');
            locale.push_str(&country);
        }
        std::env::set_var("OMEGAT_LOCALE", locale);
    }
    Ok(())
}

fn effective_scripts_dir(config_dir: &Path, prefs: &Preferences) -> Result<PathBuf> {
    let configured = std::env::var_os("OMEGAT_SCRIPTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&prefs.script_dir));
    let configured = if configured.is_absolute() {
        configured
    } else {
        config_dir.join(configured)
    };
    if let Some(valid) = omegat_core::cli_params::resolve_scripts_folder(Some(&configured)) {
        return Ok(valid);
    }
    let default = omegat_core::cli_params::default_user_scripts_dir(config_dir);
    std::fs::create_dir_all(&default)?;
    Ok(default)
}

fn desktop_command() -> Result<(PathBuf, Vec<OsString>)> {
    if let Some(bin) = std::env::var_os("OMEGAT_DESKTOP_BIN") {
        return Ok((PathBuf::from(bin), Vec::new()));
    }

    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let names: &[&str] = if cfg!(windows) {
                &["OmegaT.exe", "omegat-desktop.exe"]
            } else {
                &["OmegaT", "omegat-desktop"]
            };
            for name in names {
                let candidate = parent.join(name);
                if candidate.is_file() && candidate != current {
                    return Ok((candidate, Vec::new()));
                }
            }
        }
    }

    let desktop = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop");
    let electron = desktop.join(if cfg!(windows) {
        "node_modules/.bin/electron.cmd"
    } else {
        "node_modules/.bin/electron"
    });
    if electron.is_file() {
        return Ok((electron, vec![desktop.into_os_string()]));
    }
    Err(anyhow!(
        "Electron launcher not found; set OMEGAT_DESKTOP_BIN or install apps/desktop dependencies"
    ))
}

fn launch_desktop(project: Option<PathBuf>, quiet: bool) -> Result<()> {
    let config_dir = default_config_dir();
    let prefs = Preferences::load_or_default(&config_dir);
    let scripts_dir = effective_scripts_dir(&config_dir, &prefs)?;
    std::env::set_var("OMEGAT_CONFIG_DIR", &config_dir);
    std::env::set_var("OMEGAT_SCRIPTS_DIR", &scripts_dir);
    if let Some(project) = &project {
        std::env::set_var("OMEGAT_PROJECT", project);
    } else {
        std::env::remove_var("OMEGAT_PROJECT");
    }

    if !quiet {
        println!(
            "OmegaT {} rewrite — launching Electron desktop.",
            omegat_ipc::APP_VERSION
        );
    }
    if let Some(project) = &project {
        println!("Project: {}", project.display());
    }
    if std::env::var_os("OMEGAT_LAUNCH_DRY_RUN").is_some() {
        return Ok(());
    }

    let (bin, args) = desktop_command()?;
    Command::new(bin)
        .args(args)
        .env("OMEGAT_CONFIG_DIR", config_dir)
        .env("OMEGAT_SCRIPTS_DIR", scripts_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(Into::into)
}

fn normalize_empty_config_dir(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args = args.into_iter().peekable();
    let mut normalized = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--config-dir=" {
            continue;
        }
        if arg == "--config-dir"
            && args
                .peek()
                .is_some_and(|value| value.is_empty())
        {
            args.next();
            continue;
        }
        normalized.push(arg);
    }
    normalized
}

fn legacy_mode(mode: &str, cli: &Cli) -> Result<()> {
    match mode {
        "console-translate" => {
            let root = cli.project.clone().unwrap_or_else(|| PathBuf::from("."));
            let mut session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
            if let Some(mode) = cli.tag_validation.as_deref() {
                apply_tag_validation(&mut session, mode);
            }
            compile_reporting(&mut session, cli.source_pattern.as_deref())?;
            Ok(())
        }
        "console-stats" => {
            let root = cli.project.clone().unwrap_or_else(|| PathBuf::from("."));
            let session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
            println!("{}", serde_json::to_string(&session.stats())?);
            Ok(())
        }
        "console-createpseudotranslatetmx" => {
            let root = cli.project.clone().unwrap_or_else(|| PathBuf::from("."));
            let session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
            let empty = cli.pseudotranslatetype.as_deref() == Some("empty");
            let mut tmx = omegat_core::tmx::ProjectTmx::new();
            for e in &session.entries {
                tmx.insert(omegat_core::tmx::TmxEntry {
                    source: e.source.clone(),
                    translation: if empty { String::new() } else { e.source.clone() },
                    ..Default::default()
                });
            }
            let dest = cli
                .pseudotranslatetmx
                .clone()
                .unwrap_or_else(|| root.join("pseudo.tmx"));
            tmx.write(&dest, &session.props.source_lang, &session.props.target_lang)?;
            Ok(())
        }
        "console-align" => legacy_align(cli),
        other => anyhow::bail!("unknown --mode {other}. Supported: console-translate, console-stats, console-createpseudotranslatetmx, console-align"),
    }
}

fn apply_tag_validation(session: &mut ProjectSession, mode: &str) {
    session.prefs.tag_validation = mode.to_string();
    if mode == "warn" {
        let n = session
            .issues()
            .iter()
            .filter(|i| i.kind == "tag")
            .count();
        if n > 0 {
            eprintln!("TAG_VALIDATION: {n} issue(s)");
        }
    }
}

fn compile_reporting(session: &mut ProjectSession, source_pattern: Option<&str>) -> Result<usize> {
    match session.compile(source_pattern) {
        Ok(n) => Ok(n),
        Err(CoreError::TagValidation(msg)) => {
            let line = if msg.contains("TAG_VALIDATION") {
                msg
            } else {
                format!("TAG_VALIDATION: {msg}")
            };
            eprintln!("{line}");
            Err(anyhow!("{line}"))
        }
        Err(e) => Err(e.into()),
    }
}

fn legacy_align(cli: &Cli) -> Result<()> {
    let root = cli.project.clone().unwrap_or_else(|| PathBuf::from("."));
    let align_dir = cli
        .align_dir
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--alignDir is required for --mode console-align"))?;
    let session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
    let cfg = align_cfg("heapwise", "viterbi", "word", "normal");
    let mut n = 0;
    for ent in walkdir::WalkDir::new(&session.props.source_dir).into_iter().flatten() {
        if !ent.file_type().is_file() {
            continue;
        }
        let rel = ent.path().strip_prefix(&session.props.source_dir).unwrap_or(ent.path());
        let other = align_dir.join(rel);
        if !other.exists() {
            continue;
        }
        let dest = session.props.root.join(format!(
            "align-{}.tmx",
            rel.to_string_lossy().replace(['/', '\\'], "_")
        ));
        let tmx = omegat_core::align::align_files_cfg(
            ent.path(),
            &other,
            &session.props.source_lang,
            &session.props.target_lang,
            &cfg,
        )?;
        omegat_core::align::write_aligned_tmx(
            &tmx,
            &dest,
            &session.props.source_lang,
            &session.props.target_lang,
        )?;
        n += 1;
    }
    println!("Aligned {n} file pair(s) from {}", align_dir.display());
    Ok(())
}

fn align_cfg(mode: &str, algo: &str, counter: &str, calculator: &str) -> omegat_core::align::AlignConfig {
    omegat_core::align::AlignConfig {
        mode: match mode {
            "heapwise" => omegat_core::align::AlignMode::Heapwise,
            "id" => omegat_core::align::AlignMode::Id,
            _ => omegat_core::align::AlignMode::Parsewise,
        },
        algo: if algo == "forward-backward" {
            omegat_core::align::AlignAlgo::ForwardBackward
        } else {
            omegat_core::align::AlignAlgo::Viterbi
        },
        counter: if counter == "char" {
            omegat_core::align::Counter::Char
        } else {
            omegat_core::align::Counter::Word
        },
        calculator: if calculator == "poisson" {
            omegat_core::align::CalculatorType::Poisson
        } else {
            omegat_core::align::CalculatorType::Normal
        },
        segment: true,
    }
}

fn script_state_from_session(session: &ProjectSession, index: usize) -> omegat_script::ScriptState {
    let e = session.entries.get(index);
    omegat_script::ScriptState {
        source: e.map(|e| e.source.clone()).unwrap_or_default(),
        translation: e.map(|e| e.translation.clone()).unwrap_or_default(),
        note: e.map(|e| e.note.clone()).unwrap_or_default(),
        index,
        revision: e.map(|e| e.revision).unwrap_or(1),
        source_lang: session.props.source_lang.clone(),
        target_lang: session.props.target_lang.clone(),
        ..omegat_script::ScriptState::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn help_lists_legacy_flags() {
        let mut buf = Vec::new();
        Cli::command().write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();
        for flag in [
            "--no-team",
            "--mode",
            "--config-dir",
            "--config-file",
            "--resource-bundle",
            "--disable-project-locking",
            "--disable-location-save",
            "--source-pattern",
            "--pseudotranslatetmx",
            "--pseudotranslatetype",
            "--alignDir",
            "--output-file",
            "--stats-type",
            "--script",
            "--tag-validation",
        ] {
            assert!(help.contains(flag), "help missing {flag}\n{help}");
        }
    }
}

