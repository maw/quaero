use std::collections::BTreeSet;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use crate::cli::Cli;
use crate::search::prepare_regex_pattern;

pub(crate) struct GitLogMatch {
    pub repo: String,
    pub hash: String,
    pub date: String,
    pub message: String,
}

/// Discover git repositories relevant to the search path.
///
/// 1. If the search path is inside a git repo, include that repo.
/// 2. Check immediate children of the search path for .git directories.
/// Deduplicate by canonical path.
pub(crate) fn discover_git_repos(search_path: &str) -> Vec<PathBuf> {
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

pub(crate) fn search_git_log(cli: &Cli) -> io::Result<Vec<GitLogMatch>> {
    let repos = discover_git_repos(&cli.path);
    let mut matches = Vec::new();
    let pattern = prepare_regex_pattern(cli);

    for repo in repos {
        let repo_str = repo.to_string_lossy().to_string();
        let mut cmd = Command::new("git");
        cmd.args([
            "-C",
            &repo_str,
            "log",
            "--format=%h %ad %s",
            "--date=short",
            "-E",
        ]);
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
                eprintln!("qro: git not found, skipping log search");
                return Ok(matches);
            }
            Err(e) => {
                eprintln!("qro: git log in {repo_str}: {e}");
                continue;
            }
        };

        if !output.status.success() {
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let mut parts = line.splitn(3, ' ');
            if let (Some(hash), Some(date), Some(message)) =
                (parts.next(), parts.next(), parts.next())
            {
                matches.push(GitLogMatch {
                    repo: repo_str.clone(),
                    hash: hash.to_string(),
                    date: date.to_string(),
                    message: message.to_string(),
                });
            }
        }
    }

    Ok(matches)
}
