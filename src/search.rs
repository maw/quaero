use std::collections::BTreeMap;
use std::io;

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkFinish, SinkMatch};
use ignore::WalkBuilder;

use crate::cli::Cli;

pub(crate) enum ContentMatch {
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

pub(crate) fn build_walker(cli: &Cli) -> io::Result<ignore::Walk> {
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
pub(crate) fn prepare_regex_pattern(cli: &Cli) -> String {
    let mut pattern = cli.pattern.clone().expect("pattern is required");
    if cli.fixed_strings {
        pattern = regex::escape(&pattern);
    }
    if cli.word_regexp {
        pattern = format!(r"\b{pattern}\b");
    }
    pattern
}

pub(crate) fn search_names(cli: &Cli) -> io::Result<Vec<String>> {
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
                eprintln!("qro: {err}");
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

/// Suggest a corrected pattern when a regex parse error occurs.
///
/// Returns a human-readable hint if the pattern looks like it uses
/// shell/glob syntax instead of regex syntax.
pub(crate) fn regex_hint(pattern: &str) -> Option<String> {
    // Check for brace alternation: {foo,bar} → (foo|bar)
    if let Some(corrected) = fix_brace_alternation(pattern) {
        return Some(format!(
            "hint: it looks like you used shell brace syntax; try regex alternation instead:\n  qro '{corrected}'"
        ));
    }

    // Check for leading glob star: *foo or **/ → .*
    if pattern.starts_with("**") {
        return Some(
            "hint: `**` is glob syntax; in regex use `.*` to match any characters".to_string(),
        );
    }
    if pattern.starts_with('*') {
        return Some(
            "hint: a leading `*` is glob syntax; in regex use `.*` to match any characters, or `\\*` for a literal asterisk".to_string(),
        );
    }

    None
}

/// Replace `{a,b,c}` with `(a|b|c)` throughout the pattern.
/// Returns `None` if no brace alternation was found.
fn fix_brace_alternation(pattern: &str) -> Option<String> {
    let mut result = String::with_capacity(pattern.len());
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    let mut found = false;

    while i < chars.len() {
        if chars[i] == '{' {
            // Scan forward for a matching '}' that contains at least one ','
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == '}') {
                let close = i + 1 + close;
                let inner: String = chars[i + 1..close].iter().collect();
                if inner.contains(',') {
                    found = true;
                    result.push('(');
                    result.push_str(&inner.replace(',', "|"));
                    result.push(')');
                    i = close + 1;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    if found { Some(result) } else { None }
}

pub(crate) fn search_content(cli: &Cli) -> io::Result<BTreeMap<String, Vec<ContentMatch>>> {
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
                eprintln!("qro: {err}");
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
            eprintln!("qro: {}: {err}", path.display());
            continue;
        }

        if sink.saw_binary && !sink.matches.is_empty() {
            // File had real matches before binary data was detected.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_brace_alternation() {
        let hint = regex_hint("{foo,bar}::baz").unwrap();
        assert!(hint.contains("(foo|bar)::baz"), "got: {hint}");
    }

    #[test]
    fn hint_brace_three_alternatives() {
        let hint = regex_hint("{a,b,c}").unwrap();
        assert!(hint.contains("(a|b|c)"), "got: {hint}");
    }

    #[test]
    fn hint_multiple_brace_groups() {
        let hint = regex_hint("{a,b}::{c,d}").unwrap();
        assert!(hint.contains("(a|b)::(c|d)"), "got: {hint}");
    }

    #[test]
    fn hint_leading_double_star() {
        let hint = regex_hint("**/foo").unwrap();
        assert!(hint.contains("`**`"), "got: {hint}");
        assert!(hint.contains(".*"), "got: {hint}");
    }

    #[test]
    fn hint_leading_star() {
        let hint = regex_hint("*.rs").unwrap();
        assert!(hint.contains("`*`"), "got: {hint}");
    }

    #[test]
    fn no_hint_for_valid_regex() {
        assert!(regex_hint("foo|bar").is_none());
        assert!(regex_hint("foo.*bar").is_none());
        assert!(regex_hint(r"\bword\b").is_none());
    }

    #[test]
    fn no_hint_brace_without_comma() {
        // {3} is a valid regex repetition, not brace alternation
        assert!(regex_hint("a{3}").is_none());
    }
}
