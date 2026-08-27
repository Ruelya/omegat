//! Java `org.omegat.cli` / `Main.extractConfigDir` / `Main.constructCommandParams`.

use crate::file_util::expand_tilde_home_dir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonParams {
    pub project_locking: bool,
    pub location_save: bool,
    pub no_team: bool,
    pub tokenizer_source: Option<String>,
    pub tokenizer_target: Option<String>,
}

impl Default for CommonParams {
    fn default() -> Self {
        Self {
            project_locking: true,
            location_save: true,
            no_team: false,
            tokenizer_source: None,
            tokenizer_target: None,
        }
    }
}

/// Apply CLI flags the way `CommandCommon.parseCommonParams` writes
/// `RuntimePreferences`.
pub fn parse_common_params(flags: &[&str]) -> CommonParams {
    let mut p = CommonParams::default();
    let mut i = 0;
    while i < flags.len() {
        match flags[i] {
            "--no-project-locking" | "--disable-project-locking" => p.project_locking = false,
            "--no-location-save" | "--disable-location-save" => p.location_save = false,
            "--no-team" => p.no_team = true,
            "--team" => p.no_team = false,
            "--ITokenizer" => {
                if let Some(v) = flags.get(i + 1) {
                    p.tokenizer_source = Some((*v).into());
                    i += 1;
                }
            }
            "--ITokenizerTarget" => {
                if let Some(v) = flags.get(i + 1) {
                    p.tokenizer_target = Some((*v).into());
                    i += 1;
                }
            }
            other if other.starts_with("--config-dir=") => {}
            "--config-dir" => i += 1,
            _ => {}
        }
        i += 1;
    }
    p
}

/// Java `Main.extractConfigDir` (`--config-dir` / `--config-dir=`).
/// Empty `--config-dir=` is treated as absent.
pub fn extract_config_dir(args: &[&str]) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        let value = if *arg == "--config-dir" {
            args.get(i + 1).map(|s| (*s).to_string())
        } else {
            arg.strip_prefix("--config-dir=").map(str::to_string)
        };
        if let Some(v) = value {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// Runtime preference snapshot used by `constructCommandParams` / `initialize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePrefs {
    pub config_dir: Option<String>,
    pub config_file: Option<String>,
    pub resource_bundle: Option<String>,
    pub project_locking: bool,
    pub location_save: bool,
    pub no_team: bool,
    pub quiet: bool,
    pub tokenizer_source: Option<String>,
    pub tokenizer_target: Option<String>,
    pub alternate_filename_from: Option<String>,
    pub alternate_filename_to: Option<String>,
}

impl Default for RuntimePrefs {
    fn default() -> Self {
        Self {
            config_dir: None,
            config_file: None,
            resource_bundle: None,
            project_locking: true,
            location_save: true,
            no_team: false,
            quiet: false,
            tokenizer_source: None,
            tokenizer_target: None,
            alternate_filename_from: None,
            alternate_filename_to: None,
        }
    }
}

/// Java `Main.constructCommandParams`.
pub fn construct_command_params(p: &RuntimePrefs) -> Vec<String> {
    let mut command = Vec::new();
    if let Some(d) = &p.config_dir {
        command.push("--config-dir".into());
        command.push(d.clone());
    }
    if let Some(f) = &p.config_file {
        command.push("--config-file".into());
        command.push(f.clone());
    }
    if let Some(b) = &p.resource_bundle {
        command.push("--resource-bundle".into());
        command.push(b.clone());
    }
    if !p.project_locking {
        command.push("--disable-project-locking".into());
    }
    if !p.location_save {
        command.push("--disable-location-save".into());
    }
    if p.no_team {
        command.push("--no-team".into());
    }
    command.push("start".into());
    if p.quiet {
        command.push("--quiet".into());
    }
    if let Some(t) = &p.tokenizer_source {
        command.push("--ITokenizer".into());
        command.push(t.clone());
    }
    if let Some(t) = &p.tokenizer_target {
        command.push("--ITokenizerTarget".into());
        command.push(t.clone());
    }
    if let (Some(from), Some(to)) = (&p.alternate_filename_from, &p.alternate_filename_to) {
        command.push("--alternate-filename-from".into());
        command.push(from.clone());
        command.push("--alternate-filename-to".into());
        command.push(to.clone());
    }
    command
}

/// Java `LegacyParameters.initialize` — apply parsed flags onto runtime prefs.
pub fn initialize_legacy(args: &[&str]) -> RuntimePrefs {
    let mut p = RuntimePrefs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--config-dir" => {
                if let Some(v) = args.get(i + 1).filter(|value| !value.is_empty()) {
                    p.config_dir = Some(expand_tilde_home_dir(v));
                    i += 1;
                }
            }
            other if let Some(v) = other.strip_prefix("--config-dir=") => {
                if !v.is_empty() {
                    p.config_dir = Some(expand_tilde_home_dir(v));
                }
            }
            "--config-file" => {
                if let Some(v) = args.get(i + 1) {
                    p.config_file = Some((*v).into());
                    i += 1;
                }
            }
            "--resource-bundle" => {
                if let Some(v) = args.get(i + 1) {
                    p.resource_bundle = Some((*v).into());
                    i += 1;
                }
            }
            "--disable-project-locking" => p.project_locking = false,
            "--disable-location-save" => p.location_save = false,
            "--no-team" => p.no_team = true,
            "--quiet" => p.quiet = true,
            _ => {}
        }
        i += 1;
    }
    p
}

/// Java `ScriptingWindow` scripts folder: a non-directory preference is ignored
/// (bug #775 — no NPE / empty window).
pub fn resolve_scripts_folder(pref: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    pref.filter(|p| p.is_dir()).map(|p| p.to_path_buf())
}

/// Java `StaticUtils.getUserScriptsDir`: `<config>/scripts`.
pub fn default_user_scripts_dir(config_dir: &std::path::Path) -> std::path::PathBuf {
    config_dir.join("scripts")
}
