use anyhow::Result;
use clap::{Parser, Subcommand};
use omegat_core::prefs::{default_config_dir, Preferences};
use omegat_core::session::ProjectSession;
use omegat_ipc::SearchParams;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "omegat", version = omegat_ipc::APP_VERSION, about = "OmegaT computer-assisted translation")]
struct Cli {
    /// Project directory (default: current directory)
    project: Option<PathBuf>,
    #[arg(long)]
    config_dir: Option<PathBuf>,
    #[arg(long)]
    no_team: bool,
    #[arg(long)]
    disable_project_locking: bool,
    /// Legacy console mode
    #[arg(long)]
    mode: Option<String>,
    #[arg(long)]
    source_pattern: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        project: Option<PathBuf>,
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
    let cli = Cli::parse();
    if cli.no_team {
        std::env::set_var("OMEGAT_NO_TEAM", "1");
    }
    if let Some(dir) = &cli.config_dir {
        std::env::set_var("OMEGAT_CONFIG_DIR", dir);
    }
    if let Some(mode) = &cli.mode {
        return legacy_mode(mode, &cli);
    }
    match cli.command.unwrap_or(Commands::Start {
        project: cli.project.clone(),
    }) {
        Commands::Start { project } => {
            println!(
                "OmegaT {} rewrite — launch the Electron app (apps/desktop) or use `omegat translate`.",
                omegat_ipc::APP_VERSION
            );
            if let Some(p) = project.or(cli.project) {
                println!("Project: {}", p.display());
            }
            Ok(())
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
            if let Some(mode) = tag_validation.as_deref() {
                let issues = session.issues();
                let tags: Vec<_> = issues.iter().filter(|i| i.kind == "tag" && i.severity == "error").collect();
                if !tags.is_empty() && mode == "abort" {
                    anyhow::bail!("tag validation failed ({} issues)", tags.len());
                }
            }
            let n = session.compile(source_pattern.as_deref().or(cli.source_pattern.as_deref()))?;
            if let Some(script) = script {
                let src = std::fs::read_to_string(script)?;
                let _ = omegat_script::run_source(&src, &serde_json::json!({"event":"COMPILE"}));
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
            let text = match r#type.as_str() {
                "json" => serde_json::to_string_pretty(&stats)?,
                "xml" => format!(
                    "<stats segments=\"{}\" translated=\"{}\" words=\"{}\"/>\n",
                    stats.segments, stats.translated, stats.source_words
                ),
                _ => format!(
                    "files={} segments={} translated={} unique={} source_words={} target_words={}\n",
                    stats.files,
                    stats.segments,
                    stats.translated,
                    stats.unique_segments,
                    stats.source_words,
                    stats.target_words
                ),
            };
            if let Some(p) = output {
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
                let translation = if r#type == "empty" {
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
            let dest = output_file.unwrap_or_else(|| root.join("pseudo.tmx"));
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
        } => {
            let cfg = omegat_core::align::AlignConfig {
                mode: match mode.as_str() {
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
            };
            let tmx = omegat_core::align::align_files_cfg(&source, &target, &source_lang, &target_lang, &cfg)?;
            omegat_core::align::write_aligned_tmx(&tmx, &output, &source_lang, &target_lang)?;
            println!("Aligned TMX written to {}", output.display());
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
                source: true,
                translation: true,
                glossary: false,
                tmx: false,
                replace: None,
            });
            for h in hits {
                println!("#{} {} [{}] {}", h.index, h.file, h.field, h.text);
            }
            Ok(())
        }
    }
}

fn legacy_mode(mode: &str, cli: &Cli) -> Result<()> {
    match mode {
        "console-translate" => {
            let root = cli.project.clone().unwrap_or_else(|| PathBuf::from("."));
            let mut session = ProjectSession::open(&root, Preferences::load_or_default(&default_config_dir()))?;
            session.compile(cli.source_pattern.as_deref())?;
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
            let mut tmx = omegat_core::tmx::ProjectTmx::new();
            for e in &session.entries {
                tmx.insert(omegat_core::tmx::TmxEntry {
                    source: e.source.clone(),
                    translation: e.source.clone(),
                    ..Default::default()
                });
            }
            tmx.write(&root.join("pseudo.tmx"), &session.props.source_lang, &session.props.target_lang)?;
            Ok(())
        }
        "console-align" => anyhow::bail!("use `omegat align --mode parsewise --algo viterbi`"),
        other => anyhow::bail!("unknown --mode {other}. Supported: console-translate, console-stats, console-createpseudotranslatetmx, console-align"),
    }
}

