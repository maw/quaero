use std::fs;
use std::path::{Path, PathBuf};
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

/// Create a temporary git repo with a known commit and return its path.
/// The commit message will contain `msg_marker`. The file will contain `file_content`.
fn make_git_repo(parent: &Path, name: &str, msg_marker: &str, file_content: &str) -> PathBuf {
    let repo = parent.join(name);
    fs::create_dir_all(&repo).unwrap();

    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test")
            .output()
            .expect("git command failed");
        assert!(out.status.success(), "git {:?} failed: {}", args,
            String::from_utf8_lossy(&out.stderr));
    };

    git(&["init"]);
    fs::write(repo.join("file.txt"), file_content).unwrap();
    git(&["add", "file.txt"]);
    git(&["commit", "-m", msg_marker]);

    repo
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

// --- Glob file filter (-g) ---

#[test]
fn glob_filters_content_search() {
    // Only search .rs files for "hello"
    let out = qae(&["-c", "-g", "*.rs", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should find hello in .rs files");
    assert!(!text.contains("hello.txt"), "should not search .txt files");
}

#[test]
fn glob_filters_name_search() {
    let out = qae(&["-n", "-g", "*.txt", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should find hello.txt");
    assert!(!text.contains("greeting.rs"), "should not include .rs files");
}

#[test]
fn glob_filters_default_mode() {
    let out = qae(&["-g", "*.txt", "world", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should find matches in .txt files");
    assert!(!text.contains("greeting.rs"), "should not include .rs files");
}

#[test]
fn glob_with_fixed_strings() {
    // -g and -F should work together
    let out = qae(&["-c", "-g", "*.rs", "-F", "(", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should find ( in .rs files");
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
fn log_only_with_names_only_errors() {
    let out = qae(&["--log-only", "-n", "test", "."]);

    assert!(!out.status.success(), "--log-only -n should fail");
    assert!(
        stderr(&out).contains("--log-only and --names-only are mutually exclusive"),
        "should show appropriate error message"
    );
}

#[test]
fn log_only_with_content_only_errors() {
    let out = qae(&["--log-only", "-c", "test", "."]);

    assert!(!out.status.success(), "--log-only -c should fail");
    assert!(
        stderr(&out).contains("--log-only and --content-only are mutually exclusive"),
        "should show appropriate error message"
    );
}

#[test]
fn log_only_with_glob_errors() {
    let out = qae(&["--log-only", "-g", "*.rs", "test", "."]);

    assert!(!out.status.success(), "--log-only --glob should fail");
    assert!(
        stderr(&out).contains("--log-only and --glob are mutually exclusive"),
        "should show appropriate error message"
    );
}

// --- Git log search ---

#[test]
fn log_only_finds_commit_message() {
    let tmp = tempfile::tempdir().unwrap();
    make_git_repo(tmp.path(), "repo-a", "Fix issue99001 in auth", "some content");

    let out = qae(&["--log-only", "issue99001", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.contains("issue99001"), "should find pattern in commit message");
    assert!(text.contains("(git log):"), "should show git log section header");
}

#[test]
fn log_only_case_insensitive() {
    let tmp = tempfile::tempdir().unwrap();
    make_git_repo(tmp.path(), "repo-a", "Fix ISSUE99002 in auth", "some content");

    let out = qae(&["--log-only", "-i", "issue99002", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.contains("ISSUE99002"), "should find case-insensitive match");
}

#[test]
fn log_only_fixed_strings() {
    let tmp = tempfile::tempdir().unwrap();
    // Commit message contains literal "(test99003)" — parens are regex metacharacters
    make_git_repo(tmp.path(), "repo-a", "Fix (test99003) bug", "some content");

    let out = qae(&["--log-only", "-F", "(test99003)", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.contains("test99003"), "should find literal pattern");
}

#[test]
fn log_flag_in_default_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = make_git_repo(tmp.path(), "repo-a", "Fix issue99004 in auth", "issue99004 content here");

    let out = qae(&["-l", "issue99004", repo.to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    // Should find both file content and git log
    assert!(text.contains("file.txt"), "should find file content match");
    assert!(text.contains("(git log):"), "should show git log section");
    assert!(text.contains("issue99004"), "should show commit message");
}

#[test]
fn log_flag_no_git_repo_silently_skips() {
    let tmp = tempfile::tempdir().unwrap();
    // Create a plain file, no git repo
    fs::write(tmp.path().join("test.txt"), "hello99005").unwrap();

    let out = qae(&["-l", "-c", "hello99005", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.contains("test.txt"), "should still find file content");
    assert!(!text.contains("(git log)"), "should not show git log section");
}

#[test]
fn log_discovers_child_repos() {
    let tmp = tempfile::tempdir().unwrap();
    make_git_repo(tmp.path(), "repo-a", "Fix issue99006 in repo-a", "unrelated");
    make_git_repo(tmp.path(), "repo-b", "Fix issue99006 in repo-b", "unrelated");

    let out = qae(&["--log-only", "issue99006", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.contains("repo-a"), "should find match in repo-a");
    assert!(text.contains("repo-b"), "should find match in repo-b");
}

#[test]
fn log_only_no_match_produces_empty_output() {
    let tmp = tempfile::tempdir().unwrap();
    make_git_repo(tmp.path(), "repo-a", "Fix something else", "some content");

    let out = qae(&["--log-only", "zzz_nomatch99007_zzz", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(text.is_empty(), "no matches should produce empty output");
}
