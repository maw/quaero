use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::process;

use clap::Parser;
use globset::GlobBuilder;
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

    /// Treat pattern as a literal string, not a regex
    #[arg(short = 'F', long)]
    fixed_strings: bool,

    /// Treat pattern as a shell glob (implies --names-only)
    #[arg(short = 'g', long)]
    glob: bool,

    /// Only match whole words
    #[arg(short = 'w', long)]
    word_regexp: bool,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
}

struct ContentMatch {
    line_number: u64,
    line: String,
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

/// Prepare the regex pattern based on CLI flags (-F escapes, -w adds \b).
fn prepare_regex_pattern(cli: &Cli) -> String {
    let mut pattern = cli.pattern.clone();
    if cli.fixed_strings {
        pattern = regex::escape(&pattern);
    }
    if cli.word_regexp {
        pattern = format!(r"\b{pattern}\b");
    }
    pattern
}

fn search_names(cli: &Cli) -> io::Result<Vec<String>> {
    let mut matches = Vec::new();

    if cli.glob {
        let has_separator = cli.pattern.contains('/');
        let glob = GlobBuilder::new(&cli.pattern)
            .case_insensitive(cli.ignore_case)
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let matcher = glob.compile_matcher();

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
            let candidate = if has_separator {
                path.to_string_lossy().to_string()
            } else {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            };

            if matcher.is_match(Path::new(&candidate)) {
                matches.push(path.display().to_string());
            }
        }
    } else {
        let pattern = prepare_regex_pattern(cli);
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(cli.ignore_case)
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

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
    }

    Ok(matches)
}

fn search_content(cli: &Cli) -> io::Result<BTreeMap<String, Vec<ContentMatch>>> {
    let pattern = prepare_regex_pattern(cli);
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(cli.ignore_case)
        .build(&pattern)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .build();

    let mut results: BTreeMap<String, Vec<ContentMatch>> = BTreeMap::new();

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
        let path_str = path.display().to_string();
        let result = searcher.search_path(
            &matcher,
            &path,
            Lossy(|lnum, line| {
                let line = line
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string();
                results
                    .entry(path_str.clone())
                    .or_default()
                    .push(ContentMatch {
                        line_number: lnum,
                        line,
                    });
                Ok(true)
            }),
        );

        if let Err(err) = result {
            eprintln!("qae: {}: {err}", path.display());
        }
    }

    Ok(results)
}

fn run(cli: &Cli) -> io::Result<()> {
    if cli.glob && cli.content_only {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--glob only applies to filename matching",
        ));
    }
    if cli.glob && cli.fixed_strings {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--glob and --fixed-strings are mutually exclusive",
        ));
    }
    if cli.glob && cli.word_regexp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--glob and --word-regexp are mutually exclusive",
        ));
    }

    if cli.names_only || cli.glob {
        let name_matches = search_names(cli)?;
        for m in &name_matches {
            println!("{m}");
        }
        return Ok(());
    }

    if cli.content_only {
        let content_matches = search_content(cli)?;
        for (path, matches) in &content_matches {
            println!("{path}");
            for m in matches {
                println!("  {}:{}", m.line_number, m.line);
            }
            println!();
        }
        return Ok(());
    }

    // Both mode: group by file
    let name_matches: BTreeSet<String> = search_names(cli)?.into_iter().collect();
    let content_matches = search_content(cli)?;

    let all_paths: BTreeSet<&String> = name_matches.iter().chain(content_matches.keys()).collect();

    let mut first = true;
    for path in &all_paths {
        if !first {
            println!();
        }
        first = false;

        println!("{path}");
        if name_matches.contains(*path) {
            println!("  (name match)");
        }
        if let Some(matches) = content_matches.get(*path) {
            for m in matches {
                println!("  {}:{}", m.line_number, m.line);
            }
        }
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
