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
    #[arg(required_unless_present = "completions")]
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

    /// Include hidden files
    #[arg(long)]
    pub hidden: bool,

    /// Don't respect .gitignore
    #[arg(long)]
    pub no_ignore: bool,

    /// Filter by file type (e.g., rust, python)
    #[arg(short = 't', long = "type")]
    pub file_type: Option<String>,

    /// Treat pattern as a literal string, not a regex
    #[arg(short = 'F', long)]
    pub fixed_strings: bool,

    /// Filter files by glob pattern (e.g., -g '*.rs')
    #[arg(short = 'g', long)]
    pub glob: Option<String>,

    /// Exclude files matching glob pattern (repeatable)
    #[arg(short = 'x', long = "ignore", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Only match whole words
    #[arg(short = 'w', long)]
    pub word_regexp: bool,

    /// Include git log matches
    #[arg(short = 'l', long)]
    pub log: bool,

    /// Only search git logs
    #[arg(long)]
    pub log_only: bool,

    /// Show detailed output
    #[arg(short, long)]
    pub verbose: bool,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,
}
