use std::io;
use std::path::Path;
use std::process;

use clap::Parser;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Lossy;
use grep_searcher::SearcherBuilder;
use ignore::WalkBuilder;

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

fn build_walker(cli: &Cli) -> io::Result<ignore::Walk> {
    let mut walker = WalkBuilder::new(&cli.path);
    walker
        .hidden(!cli.hidden)
        .git_ignore(!cli.no_ignore);

    if let Some(ref ft) = cli.file_type {
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add_defaults();
        types_builder.select(ft);
        let types = types_builder
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        walker.types(types);
    }

    Ok(walker.build())
}

fn search_names(cli: &Cli) -> io::Result<Vec<String>> {
    let re = regex::RegexBuilder::new(&cli.pattern)
        .case_insensitive(cli.ignore_case)
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let mut matches = Vec::new();

    for entry in build_walker(cli)? {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("qae: {err}");
                continue;
            }
        };

        if entry.path().is_dir() {
            continue;
        }

        let path = entry.path();
        if re.is_match(&path.to_string_lossy()) {
            matches.push(path.display().to_string());
        }
    }

    Ok(matches)
}

fn search_content(cli: &Cli) -> io::Result<()> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(cli.ignore_case)
        .build(&cli.pattern)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .build();

    for entry in build_walker(cli)? {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("qae: {err}");
                continue;
            }
        };

        if entry.path().is_dir() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let result = searcher.search_path(
            &matcher,
            &path,
            Lossy(|lnum, line| {
                print_content_match(&path, lnum, line);
                Ok(true)
            }),
        );

        if let Err(err) = result {
            eprintln!("qae: {}: {err}", path.display());
        }
    }

    Ok(())
}

fn print_content_match(path: &Path, line_number: u64, line: &str) {
    let line = line.trim_end_matches('\n').trim_end_matches('\r');
    println!("{}:{line_number}:{line}", path.display());
}

fn run(cli: &Cli) -> io::Result<()> {
    let search_both = !cli.names_only && !cli.content_only;

    if cli.names_only || search_both {
        let name_matches = search_names(cli)?;
        if !name_matches.is_empty() {
            if search_both {
                println!("Filename matches:");
            }
            for m in &name_matches {
                if search_both {
                    println!("  {m}");
                } else {
                    println!("{m}");
                }
            }
        }
    }

    if cli.content_only || search_both {
        if search_both {
            println!();
            println!("Content matches:");
        }
        search_content(cli)?;
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("{cli:?}");
    }

    if let Err(err) = run(&cli) {
        eprintln!("qae: {err}");
        process::exit(1);
    }
}
