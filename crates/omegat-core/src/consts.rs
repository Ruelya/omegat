pub const FILE_PROJECT: &str = "omegat.project";
pub const STATUS_TMX: &str = "project_save.tmx";
pub const DEFAULT_SOURCE: &str = "source";
pub const DEFAULT_TARGET: &str = "target";
pub const DEFAULT_GLOSSARY: &str = "glossary";
pub const DEFAULT_W_GLOSSARY: &str = "glossary.txt";
pub const DEFAULT_TM: &str = "tm";
pub const DEFAULT_DICT: &str = "dictionary";
pub const DEFAULT_INTERNAL: &str = "omegat";
pub const AUTO_TM: &str = "auto";
pub const ENFORCE_TM: &str = "enforce";
pub const MT_TM: &str = "mt";
pub const FILES_ORDER: &str = "files_order.txt";
pub const LAST_ENTRY: &str = "last_entry.properties";
pub const BACKUP_EXT: &str = ".bak";
pub const PROJ_VERSION: &str = "1.0";
pub const FUZZY_THRESHOLD: i32 = 30;
pub const MAX_NEAR_STRINGS: usize = 5;
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "**/.svn/**",
    "**/CVS/**",
    "**/.cvs/**",
    "**/.git/**",
    "**/.hg/**",
    "**/.repositories/**",
    "**/desktop.ini",
    "**/Thumbs.db",
    "**/.DS_Store",
    "**/~$*",
];
