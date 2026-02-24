mod cli;
mod git;
mod output;
mod search;

use std::collections::BTreeSet;
use std::io;
use std::process;

use clap::{CommandFactory, Parser};

use cli::Cli;
use git::search_git_log;
use output::{format_rg_line, git_log_blocks, print_blocks};
use search::{prepare_regex_pattern, search_content, search_names, ContentMatch};

fn run(cli: &Cli) -> io::Result<()> {
    // --type-list: print file type definitions and exit.
    if cli.type_list {
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add_defaults();
        for def in types_builder.definitions() {
            println!("{}: {}", def.name(), def.globs().join(", "));
        }
        return Ok(());
    }

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
    if cli.log_only && !cli.glob.is_empty() {
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

    // rg-compat ANSI output mode (used by deadgrep.el).
    if cli.color.as_deref() == Some("ansi") {
        let content_matches = search_content(cli)?;
        let pattern = prepare_regex_pattern(cli);
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(cli.is_case_insensitive())
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        for (path, matches) in &content_matches {
            for m in matches {
                match m {
                    ContentMatch::Line { line_number, line } => {
                        println!("{}", format_rg_line(path, *line_number, line, &re));
                    }
                    ContentMatch::BinaryFile => {
                        eprintln!(
                            "WARNING: stopped searching binary file \x1b[0m\x1b[35m{path}\x1b[0m after match"
                        );
                    }
                }
            }
        }
        return Ok(());
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
        if cli.wants_log() {
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
        if cli.wants_log() {
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

    if cli.wants_log() {
        blocks.extend(git_log_blocks(&search_git_log(cli)?));
    }
    print_blocks(&mut blocks);

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        clap_complete::generate(shell, &mut Cli::command(), "qro", &mut io::stdout());
        return;
    }

    if cli.verbose {
        eprintln!("{cli:?}");
    }

    if let Err(err) = run(&cli) {
        eprintln!("qro: {err}");
        process::exit(1);
    }
}
