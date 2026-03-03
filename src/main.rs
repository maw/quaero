mod cli;
mod git;
mod output;
mod search;

use std::collections::BTreeSet;
use std::io;
use std::process;

use clap::{CommandFactory, Parser};

use cli::Cli;
use git::{filter_git_log_matches, search_git_log};
use output::{git_log_blocks, print_blocks};
use search::{
    build_exclude_regexes, filter_content_matches, filter_name_matches, prepare_regex_pattern,
    regex_hint, search_content, search_names, ContentMatch,
};

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

    // Build exclusion regexes once (empty vecs if no flags given).
    let has_excludes = !cli.dont_match.is_empty() || !cli.filter_out.is_empty();
    let (dont_match_res, filter_out_res) = if has_excludes {
        build_exclude_regexes(cli)?
    } else {
        (vec![], vec![])
    };
    let search_re = if has_excludes {
        let pattern = prepare_regex_pattern(cli);
        Some(
            regex::RegexBuilder::new(&pattern)
                .case_insensitive(cli.ignore_case)
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?,
        )
    } else {
        None
    };

    // Log-only mode.
    if cli.log_only {
        let mut log_matches = search_git_log(cli)?;
        if let Some(ref re) = search_re {
            log_matches =
                filter_git_log_matches(log_matches, re, &dont_match_res, &filter_out_res);
        }
        let mut blocks = git_log_blocks(&log_matches);
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Names-only mode (possibly with --log interleaved).
    if cli.names_only {
        let mut name_matches = search_names(cli)?;
        if let Some(ref re) = search_re {
            name_matches =
                filter_name_matches(name_matches, re, &dont_match_res, &filter_out_res);
        }
        let mut blocks: Vec<(String, Vec<String>)> = name_matches
            .into_iter()
            .map(|m| (m.clone(), vec![m]))
            .collect();
        if cli.wants_log() {
            let mut log_matches = search_git_log(cli)?;
            if let Some(ref re) = search_re {
                log_matches = filter_git_log_matches(
                    log_matches,
                    re,
                    &dont_match_res,
                    &filter_out_res,
                );
            }
            blocks.extend(git_log_blocks(&log_matches));
        }
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Content-only mode (possibly with --log interleaved).
    if cli.content_only {
        let mut content_matches = search_content(cli)?;
        if let Some(ref re) = search_re {
            content_matches = filter_content_matches(
                content_matches,
                re,
                &dont_match_res,
                &filter_out_res,
            );
        }
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
            let mut log_matches = search_git_log(cli)?;
            if let Some(ref re) = search_re {
                log_matches = filter_git_log_matches(
                    log_matches,
                    re,
                    &dont_match_res,
                    &filter_out_res,
                );
            }
            blocks.extend(git_log_blocks(&log_matches));
        }
        print_blocks(&mut blocks);
        return Ok(());
    }

    // Both mode: group by file, optionally with git log interleaved.
    let mut name_vec = search_names(cli)?;
    let mut content_matches = search_content(cli)?;
    if let Some(ref re) = search_re {
        name_vec = filter_name_matches(name_vec, re, &dont_match_res, &filter_out_res);
        content_matches = filter_content_matches(
            content_matches,
            re,
            &dont_match_res,
            &filter_out_res,
        );
    }
    let name_matches: BTreeSet<String> = name_vec.into_iter().collect();

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
        let mut log_matches = search_git_log(cli)?;
        if let Some(ref re) = search_re {
            log_matches =
                filter_git_log_matches(log_matches, re, &dont_match_res, &filter_out_res);
        }
        blocks.extend(git_log_blocks(&log_matches));
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
        if let Some(pattern) = &cli.pattern {
            if let Some(hint) = regex_hint(pattern) {
                eprintln!("\n{hint}");
            }
        }
        process::exit(1);
    }
}
