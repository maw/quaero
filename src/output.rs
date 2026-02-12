use std::collections::BTreeMap;

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
