use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::process::{self, Command};

use clap::Parser;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkFinish, SinkMatch};
use ignore::WalkBuilder;

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

    /// Filter files by glob pattern (e.g., -g '*.rs')
    #[arg(short = 'g', long)]
    glob: Option<String>,

    /// Exclude files matching glob pattern (repeatable)
    #[arg(short = 'x', long = "ignore", action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Only match whole words
    #[arg(short = 'w', long)]
    word_regexp: bool,

    /// Include git log matches
    #[arg(short = 'l', long)]
    log: bool,

    /// Only search git logs
    #[arg(long)]
    log_only: bool,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
}

enum ContentMatch {
    Line { line_number: u64, line: String },
    BinaryFile,
}

/// Sink that collects content matches and detects binary files.
struct ContentSink {
    matches: Vec<ContentMatch>,
    saw_binary: bool,
}

impl Sink for ContentSink {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let line_number = mat.line_number().unwrap_or(0);
        let line = String::from_utf8_lossy(mat.bytes())
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string();
        self.matches.push(ContentMatch::Line { line_number, line });
        Ok(true)
    }

    fn finish(&mut self, _searcher: &Searcher, finish: &SinkFinish) -> Result<(), io::Error> {
        if finish.binary_byte_offset().is_some() {
            self.saw_binary = true;
        }
        Ok(())
    }
}

struct GitLogMatch {
    repo: String,
    hash: String,
    message: String,
}

fn build_walker(cli: &Cli) -> io::Result<ignore::Walk> {
    let mut walker = WalkBuilder::new(&cli.path);
    walker
        .hidden(!cli.hidden)
        .git_ignore(!cli.no_ignore)
        .ignore(!cli.no_ignore);

    if cli.glob.is_some() || !cli.exclude.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(&cli.path);
        if let Some(ref glob) = cli.glob {
            overrides
                .add(glob)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        }
        for pattern in &cli.exclude {
            overrides
                .add(&format!("!{pattern}"))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        }
        walker.overrides(
            overrides
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?,
        );
    }

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
        .binary_detection(BinaryDetection::quit(b'\x00'))
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
        let mut sink = ContentSink {
            matches: Vec::new(),
            saw_binary: false,
        };
        let result = searcher.search_path(&matcher, &path, &mut sink);

        if let Err(err) = result {
            eprintln!("qae: {}: {err}", path.display());
            continue;
        }

        if sink.saw_binary {
            // File had matches but also contained binary data.
            // Drop the raw lines and show a summary instead.
            results
                .entry(path_str)
                .or_default()
                .push(ContentMatch::BinaryFile);
        } else if !sink.matches.is_empty() {
            results.insert(path_str, sink.matches);
        }
    }

    Ok(results)
}

/// Discover git repositories relevant to the search path.
///
/// 1. If the search path is inside a git repo, include that repo.
/// 2. Check immediate children of the search path for .git directories.
/// Deduplicate by canonical path.
fn discover_git_repos(search_path: &str) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    let mut seen = BTreeSet::new();

    // Step 1: Check if search path is inside a git repo.
    if let Ok(output) = Command::new("git")
        .args(["-C", search_path, "rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let toplevel = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&toplevel);
            if let Ok(canonical) = path.canonicalize() {
                if seen.insert(canonical) {
                    repos.push(path);
                }
            }
        }
    }

    // Step 2: Check immediate children for .git directories.
    if let Ok(entries) = std::fs::read_dir(search_path) {
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() && child.join(".git").exists() {
                if let Ok(canonical) = child.canonicalize() {
                    if seen.insert(canonical) {
                        repos.push(child);
                    }
                }
            }
        }
    }

    repos
}

fn search_git_log(cli: &Cli) -> io::Result<Vec<GitLogMatch>> {
    let repos = discover_git_repos(&cli.path);
    let mut matches = Vec::new();
    let pattern = prepare_regex_pattern(cli);

    for repo in repos {
        let repo_str = repo.to_string_lossy().to_string();
        let mut cmd = Command::new("git");
        cmd.args(["-C", &repo_str, "log", "--oneline", "-E"]);
        if cli.ignore_case {
            cmd.arg("-i");
        }
        cmd.args(["--grep", &pattern]);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if cli.log_only {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "git is not installed",
                    ));
                }
                eprintln!("qae: git not found, skipping log search");
                return Ok(matches);
            }
            Err(e) => {
                eprintln!("qae: git log in {repo_str}: {e}");
                continue;
            }
        };

        if !output.status.success() {
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some((hash, message)) = line.split_once(' ') {
                matches.push(GitLogMatch {
                    repo: repo_str.clone(),
                    hash: hash.to_string(),
                    message: message.to_string(),
                });
            }
        }
    }

    Ok(matches)
}

/// Convert git log matches into output blocks keyed by repo path for interleaved sorting.
fn git_log_blocks(log_matches: &[GitLogMatch]) -> Vec<(String, Vec<String>)> {
    let mut by_repo: BTreeMap<&str, Vec<&GitLogMatch>> = BTreeMap::new();
    for m in log_matches {
        by_repo.entry(&m.repo).or_default().push(m);
    }
    by_repo
        .into_iter()
        .map(|(repo, matches)| {
            let mut lines = vec![format!("{repo} (git log):")];
            for m in matches {
                lines.push(format!("  {} {}", m.hash, m.message));
            }
            // Sort after all files within the repo directory.
            (format!("{repo}/\x7f"), lines)
        })
        .collect()
}

/// Sort output blocks by key and print with blank lines between multi-line blocks.
fn print_blocks(blocks: &mut Vec<(String, Vec<String>)>) {
    blocks.sort_by(|a, b| a.0.cmp(&b.0));
    let mut prev_multi = false;
    let mut first = true;
    for (_, lines) in blocks.iter() {
        let multi = lines.len() > 1;
        if !first && (multi || prev_multi) {
            println!();
        }
        first = false;
        for line in lines {
            println!("{line}");
        }
        prev_multi = multi;
    }
}

fn run(cli: &Cli) -> io::Result<()> {
    // Validate incompatible flag combinations.
    if cli.log_only && cli.names_only {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--log-only and --names-only are mutually exclusive",
        ));
    }
    if cli.log_only && cli.content_only {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--log-only and --content-only are mutually exclusive",
        ));
    }
    if cli.log_only && cli.glob.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--log-only and --glob are mutually exclusive",
        ));
    }
    if cli.log_only && !cli.exclude.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--log-only and --ignore are mutually exclusive",
        ));
    }

    // Log-only mode.
    if cli.log_only {
        let log_matches = search_git_log(cli)?;
        let mut blocks = git_log_blocks(&log_matches);
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Names-only mode (possibly with --log interleaved).
    if cli.names_only {
        let name_matches = search_names(cli)?;
        let mut blocks: Vec<(String, Vec<String>)> = name_matches
            .into_iter()
            .map(|m| (m.clone(), vec![m]))
            .collect();
        if cli.log {
            blocks.extend(git_log_blocks(&search_git_log(cli)?));
        }
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Content-only mode (possibly with --log interleaved).
    if cli.content_only {
        let content_matches = search_content(cli)?;
        let mut blocks: Vec<(String, Vec<String>)> = content_matches
            .iter()
            .map(|(path, matches)| {
                let mut lines = vec![path.clone()];
                for m in matches {
                    match m {
                        ContentMatch::Line { line_number, line } => {
                            lines.push(format!("  {line_number}:{line}"));
                        }
                        ContentMatch::BinaryFile => {
                            lines.push("  (binary file matches)".to_string());
                        }
                    }
                }
                (path.clone(), lines)
            })
            .collect();
        if cli.log {
            blocks.extend(git_log_blocks(&search_git_log(cli)?));
        }
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Both mode: group by file, optionally with git log interleaved.
    let name_matches: BTreeSet<String> = search_names(cli)?.into_iter().collect();
    let content_matches = search_content(cli)?;

    let all_paths: BTreeSet<&String> = name_matches.iter().chain(content_matches.keys()).collect();

    let mut blocks: Vec<(String, Vec<String>)> = all_paths
        .iter()
        .map(|path| {
            let mut lines = vec![path.to_string()];
            if name_matches.contains(*path) {
                lines.push("  (name match)".to_string());
            }
            if let Some(matches) = content_matches.get(*path) {
                for m in matches {
                    match m {
                        ContentMatch::Line { line_number, line } => {
                            lines.push(format!("  {line_number}:{line}"));
                        }
                        ContentMatch::BinaryFile => {
                            lines.push("  (binary file matches)".to_string());
                        }
                    }
                }
            }
            (path.to_string(), lines)
        })
        .collect();

    if cli.log {
        blocks.extend(git_log_blocks(&search_git_log(cli)?));
    }
    print_blocks(&mut blocks);

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
