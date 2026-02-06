use clap::Parser;

/// qae - Quick search combining ripgrep and fd
///
/// Search both file contents and file names with a single command.
#[derive(Parser, Debug)]
#[command(name = "qae", version, about)]
struct Cli {
    /// Search pattern (regex)
    pattern: String,

    /// Directory to search (defaults to current directory)
    #[arg(default_value = ".")]
    path: String,

    /// Only search file names
    #[arg(short = 'n', long)]
    names_only: bool,

    /// Only search file contents
    #[arg(short = 'c', long)]
    content_only: bool,

    /// Case-insensitive search
    #[arg(short, long)]
    ignore_case: bool,

    /// Include hidden files
    #[arg(long)]
    hidden: bool,

    /// Don't respect .gitignore
    #[arg(long)]
    no_ignore: bool,

    /// Filter by file type (e.g., rust, python)
    #[arg(short = 't', long = "type")]
    file_type: Option<String>,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("{cli:?}");
    }

    println!("Searching for {:?} in {}", cli.pattern, cli.path);
}
