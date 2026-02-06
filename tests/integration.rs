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
