use clap::Parser;
use clap_complete::Shell;

/// qae - Quick search combining ripgrep and fd
///
/// Search both file contents and file names with a single command.
#[derive(Parser, Debug)]
#[command(name = "qae", version, about, after_help = "\
Ignore files:\n  \
qae respects .ignore files (same syntax as .gitignore) for excluding\n  \
files and directories from search results. Place a .ignore file in any\n  \
directory; patterns apply to that directory and its children. This is\n  \
independent of git — useful for excluding build artifacts, logs, etc.\n  \
in non-git directories or without polluting .gitignore.\n\n  \
Precedence (highest to lowest):\n    \
1. Command-line flags (-x, -g, --no-ignore)\n    \
2. .ignore\n    \
3. .gitignore\n    \
4. .git/info/exclude\n    \
5. Global gitignore")]
pub(crate) struct Cli {
    /// Search pattern (regex)
    #[arg(required_unless_present_any = ["completions", "type_list"])]
    pub pattern: Option<String>,

    /// Directory to search (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Only search file names
    #[arg(short = 'n', long)]
    pub names_only: bool,

    /// Only search file contents
    #[arg(short = 'c', long)]
    pub content_only: bool,

    /// Case-insensitive search
    #[arg(short, long)]
    pub ignore_case: bool,

    /// Case-sensitive search (overrides --ignore-case and --smart-case)
    #[arg(long)]
    pub case_sensitive: bool,

    /// Smart case: case-insensitive unless pattern contains uppercase
    #[arg(short = 'S', long)]
    pub smart_case: bool,

    /// Include hidden files
    #[arg(long)]
    pub hidden: bool,

    /// Don't respect .gitignore or .ignore
    #[arg(long)]
    pub no_ignore: bool,

    /// Don't respect version control ignore files (.gitignore)
    #[arg(long)]
    pub no_ignore_vcs: bool,

    /// Filter by file type (e.g., rust, python)
    #[arg(short = 't', long = "type")]
    pub file_type: Option<String>,

    /// Treat pattern as a literal string, not a regex
    #[arg(short = 'F', long)]
    pub fixed_strings: bool,

    /// Filter files by glob pattern (repeatable, e.g., -g '*.rs')
    #[arg(short = 'g', long, action = clap::ArgAction::Append)]
    pub glob: Vec<String>,

    /// Exclude files matching glob pattern (repeatable)
    #[arg(short = 'x', long = "ignore", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Only match whole words
    #[arg(short = 'w', long)]
    pub word_regexp: bool,

    /// Include git log matches [kept for backwards compat; on by default now]
    #[arg(short = 'l', long, hide = true)]
    pub log: bool,

    /// Disable git log search [git log is searched by default]
    #[arg(long = "no-log")]
    pub no_log: bool,

    /// Only search git logs
    #[arg(long)]
    pub log_only: bool,

    /// Show detailed output
    #[arg(short, long)]
    pub verbose: bool,

    /// Color output mode (never, auto, always, ansi)
    #[arg(long, value_name = "WHEN")]
    pub color: Option<String>,

    /// List supported file types and exit
    #[arg(long)]
    pub type_list: bool,

    /// Lines of context before each match (not yet implemented)
    #[arg(short = 'B', long)]
    pub before_context: Option<usize>,

    /// Lines of context after each match (not yet implemented)
    #[arg(short = 'A', long)]
    pub after_context: Option<usize>,

    // rg-compat no-op flags (accepted silently for deadgrep compatibility)
    #[arg(long, hide = true)]
    pub no_config: bool,

    #[arg(long, hide = true)]
    pub line_number: bool,

    #[arg(long, hide = true)]
    pub no_column: bool,

    #[arg(long, hide = true)]
    pub with_filename: bool,

    #[arg(long, hide = true)]
    pub no_heading: bool,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,
}

impl Cli {
    /// Whether git log search should be performed.
    /// On by default; --no-log disables, -l re-enables.
    pub fn wants_log(&self) -> bool {
        !self.no_log || self.log
    }

    /// Determine if the search should be case-insensitive based on flag precedence.
    ///
    /// Precedence (highest to lowest):
    /// 1. --case-sensitive -> case-sensitive
    /// 2. --ignore-case -> case-insensitive
    /// 3. --smart-case -> case-insensitive if pattern has no uppercase chars
    /// 4. default -> case-sensitive
    pub fn is_case_insensitive(&self) -> bool {
        if self.case_sensitive {
            return false;
        }
        if self.ignore_case {
            return true;
        }
        if self.smart_case {
            let pattern = self.pattern.as_deref().unwrap_or("");
            return !pattern.chars().any(|c| c.is_uppercase());
        }
        false
    }
}
