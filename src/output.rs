use std::collections::BTreeMap;

use regex::Regex;

use crate::git::GitLogMatch;

/// Convert git log matches into output blocks keyed by repo path for interleaved sorting.
pub(crate) fn git_log_blocks(log_matches: &[GitLogMatch]) -> Vec<(String, Vec<String>)> {
    let mut by_repo: BTreeMap<&str, Vec<&GitLogMatch>> = BTreeMap::new();
    for m in log_matches {
        by_repo.entry(&m.repo).or_default().push(m);
    }
    by_repo
        .into_iter()
        .map(|(repo, matches)| {
            let mut lines = vec![format!("{repo} (git log):")];
            for m in matches {
                lines.push(format!("  {} {} {}", m.hash, m.date, m.message));
            }
            // Sort after all files within the repo directory.
            (format!("{repo}/\x7f"), lines)
        })
        .collect()
}

/// Highlight regex matches within a line using ANSI escape codes (rg-compatible).
fn highlight_matches(line: &str, re: &Regex) -> String {
    let mut result = String::new();
    let mut last_end = 0;
    for mat in re.find_iter(line) {
        result.push_str(&line[last_end..mat.start()]);
        result.push_str("\x1b[0m\x1b[1m\x1b[31m");
        result.push_str(mat.as_str());
        result.push_str("\x1b[0m");
        last_end = mat.end();
    }
    result.push_str(&line[last_end..]);
    result
}

/// Format a single content match line in rg's ANSI output format.
pub(crate) fn format_rg_line(path: &str, line_num: u64, content: &str, re: &Regex) -> String {
    let highlighted = highlight_matches(content, re);
    format!("\x1b[0m\x1b[35m{path}\x1b[0m:\x1b[0m\x1b[32m{line_num}\x1b[0m:{highlighted}")
}

/// Sort output blocks by key and print with blank lines between multi-line blocks.
pub(crate) fn print_blocks(blocks: &mut Vec<(String, Vec<String>)>) {
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
