use std::process::{Command, Output};

fn qae(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qae"))
        .args(args)
        .output()
        .expect("failed to run qae")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

// --- Content search (-c) ---

#[test]
fn content_search_finds_matching_lines() {
    let out = qae(&["-c", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // greeting.rs contains println!("hello")
    assert!(text.contains("greeting.rs"), "should find greeting.rs");
    assert!(text.contains("hello"), "should show matching line");

    // hello.txt contains "Hello" but default is case-sensitive
    // "hello" won't match "Hello" without -i
    assert!(
        !text.contains("hello.txt"),
        "case-sensitive: should not match Hello"
    );
}

#[test]
fn content_search_grouped_format() {
    let out = qae(&["-c", "world", "tests/fixtures/"]);
    let text = stdout(&out);

    // hello.txt has two lines with "world"
    assert!(text.contains("hello.txt"));
    assert!(text.contains("1:Hello, world!"));
    assert!(text.contains("2:Goodbye, world!"));
}

// --- Name search (-n) ---

#[test]
fn name_search_finds_file_by_name() {
    let out = qae(&["-n", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should find hello.txt by name");
}

#[test]
fn name_search_finds_nested_file() {
    let out = qae(&["-n", "nested", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("nested.txt"),
        "should find nested.txt in subdir"
    );
}

// --- Both mode (default) ---

#[test]
fn both_mode_shows_name_match_annotation() {
    let out = qae(&["hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // hello.txt matches by name ("hello" in path) — but not content (case-sensitive)
    assert!(text.contains("hello.txt"));
    assert!(text.contains("(name match)"));

    // greeting.rs matches by content (println!("hello"))
    assert!(text.contains("greeting.rs"));
}

// --- Case-insensitive (-i) ---

#[test]
fn case_insensitive_search() {
    let out = qae(&["-c", "-i", "HELLO", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("hello.txt"),
        "should match Hello case-insensitively"
    );
    assert!(
        text.contains("greeting.rs"),
        "should match hello case-insensitively"
    );
}

// --- Hidden files (--hidden) ---

#[test]
fn hidden_files_excluded_by_default() {
    let out = qae(&["-c", "secret", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        !text.contains("hidden_file"),
        "hidden file should not appear by default"
    );
}

#[test]
fn hidden_files_included_with_flag() {
    let out = qae(&["-c", "--hidden", "secret", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("hidden_file"),
        "hidden file should appear with --hidden"
    );
}

// --- No matches ---

#[test]
fn no_matches_produces_empty_output() {
    let out = qae(&["zzz_no_match_zzz", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.is_empty(), "no matches should produce empty output");
    assert!(out.status.success(), "should exit successfully");
}

// --- File type filter (-t) ---

#[test]
fn type_filter_limits_to_matching_files() {
    let out = qae(&["-c", "-t", "rust", "greet", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("greeting.rs"),
        "should find match in .rs file"
    );
    // data.csv and hello.txt should not appear even if they had a match
}

// --- Fixed strings (-F) ---

#[test]
fn fixed_strings_treats_dot_literally() {
    // Without -F, "hello.txt" would match "helloBtxt" etc. since . is any char
    // With -F, only a literal "hello.txt" matches
    let out = qae(&["-n", "-F", "hello.txt", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should match literal hello.txt");
}

#[test]
fn fixed_strings_content_search() {
    // Search for literal "(" which is a regex metacharacter
    let out = qae(&["-c", "-F", "(", "tests/fixtures/"]);
    let text = stdout(&out);

    // greeting.rs has println!("hello") which contains "("
    assert!(
        text.contains("greeting.rs"),
        "should find literal ( in greeting.rs"
    );
}

// --- Glob (--glob / -g) ---

#[test]
fn glob_matches_by_extension() {
    let out = qae(&["--glob", "*.rs", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should find .rs files");
    assert!(!text.contains("hello.txt"), "should not find .txt files");
}

#[test]
fn glob_matches_by_prefix() {
    let out = qae(&["-g", "hello*", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should find hello.txt");
    assert!(!text.contains("greeting.rs"), "should not find greeting.rs");
}

#[test]
fn glob_case_insensitive() {
    let out = qae(&["-g", "-i", "HELLO*", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("hello.txt"),
        "case-insensitive glob should find hello.txt"
    );
}

#[test]
fn glob_implies_names_only() {
    // --glob without --names-only should still only search names
    let out = qae(&["--glob", "*.txt", "tests/fixtures/"]);
    let text = stdout(&out);

    // Should list matching filenames, not content
    assert!(text.contains("hello.txt"));
    // Should not show content match annotations
    assert!(!text.contains("(name match)"));
}

// --- Word regexp (-w) ---

#[test]
fn word_regexp_matches_whole_words() {
    let out = qae(&["-c", "-w", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // greeting.rs has println!("hello") — "hello" is a whole word
    assert!(
        text.contains("greeting.rs"),
        "-w should match whole word 'hello'"
    );
}

#[test]
fn word_regexp_rejects_partial_matches() {
    // "ello" appears inside "hello" but is not a whole word
    let out = qae(&["-c", "-w", "ello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        !text.contains("greeting.rs"),
        "-w should not match partial word 'ello'"
    );
    assert!(
        !text.contains("hello.txt"),
        "-w should not match partial word 'ello'"
    );
}

#[test]
fn fixed_strings_and_word_regexp_combined() {
    // -F -w: literal pattern, whole word
    let out = qae(&["-c", "-F", "-w", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(
        text.contains("greeting.rs"),
        "-F -w should match whole word 'hello'"
    );
}

// --- Error cases ---

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn glob_with_content_only_errors() {
    let out = qae(&["--glob", "-c", "*.rs", "tests/fixtures/"]);

    assert!(!out.status.success(), "--glob -c should fail");
    assert!(
        stderr(&out).contains("--glob only applies to filename matching"),
        "should show appropriate error message"
    );
}

#[test]
fn glob_with_fixed_strings_errors() {
    let out = qae(&["--glob", "-F", "*.rs", "tests/fixtures/"]);

    assert!(!out.status.success(), "--glob -F should fail");
    assert!(
        stderr(&out).contains("--glob and --fixed-strings are mutually exclusive"),
        "should show appropriate error message"
    );
}

#[test]
fn glob_with_word_regexp_errors() {
    let out = qae(&["--glob", "-w", "*.rs", "tests/fixtures/"]);

    assert!(!out.status.success(), "--glob -w should fail");
    assert!(
        stderr(&out).contains("--glob and --word-regexp are mutually exclusive"),
        "should show appropriate error message"
    );
}
