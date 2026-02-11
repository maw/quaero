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
fn binary_file_shows_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    // Create a binary file containing a NUL byte alongside the search term.
    let bin_path = tmp.path().join("data.bin");
    fs::write(&bin_path, b"hello\x00world").unwrap();

    let out = qae(&["hello", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(
        text.contains("(binary file matches)"),
        "binary file should show placeholder, got: {text}"
    );
    // Raw binary content should NOT appear.
    assert!(
        !text.contains("hello"),
        "should not print raw binary content, got: {text}"
    );
}

#[test]
fn binary_file_not_in_names_only() {
    let tmp = tempfile::tempdir().unwrap();
    let bin_path = tmp.path().join("data.bin");
    fs::write(&bin_path, b"hello\x00world").unwrap();

    // Names-only should not show binary placeholder (no content search).
    let out = qae(&["--names-only", "hello", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);
    assert!(out.status.success());
    assert!(!text.contains("binary"), "names-only should skip content search");
}

#[test]
fn log_interleaved_with_file_results() {
    // Git log results should sort near related file results, not be appended at the end.
    let tmp = tempfile::tempdir().unwrap();
    // Create two repos: repo-a and repo-z (z sorts after a).
    make_git_repo(tmp.path(), "repo-a", "Fix issue99008 in repo-a", "issue99008 here");
    make_git_repo(tmp.path(), "repo-z", "unrelated commit", "issue99008 also here");

    // Search with -l in both mode: should find file content in both repos + git log in repo-a.
    let out = qae(&["-l", "issue99008", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(out.status.success());
    // repo-a git log should appear near repo-a file results, before repo-z results.
    let git_log_pos = text.find("(git log):").expect("should have git log section");
    let repo_z_pos = text.find("repo-z").expect("should have repo-z results");
    assert!(
        git_log_pos < repo_z_pos,
        "git log for repo-a should appear before repo-z results.\nOutput:\n{text}"
    );
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

// --- Ignore / exclude (-x) ---

#[test]
fn ignore_excludes_files_from_content_search() {
    let out = qae(&["-c", "-x", "*.txt", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should find hello in .rs files");
    assert!(!text.contains("hello.txt"), "should exclude .txt files");
}

#[test]
fn ignore_excludes_files_from_name_search() {
    let out = qae(&["-n", "-x", "*.rs", "greeting", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(!text.contains("greeting.rs"), "should exclude .rs files");
}

#[test]
fn ignore_multiple_patterns() {
    let out = qae(&["-c", "-x", "*.txt", "-x", "*.csv", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should find hello in .rs files");
    assert!(!text.contains("hello.txt"), "should exclude .txt files");
    assert!(!text.contains("data.csv"), "should exclude .csv files");
}

#[test]
fn ignore_composable_with_glob() {
    // Include only .txt files, but exclude hello*
    let out = qae(&["-c", "-g", "*.txt", "-x", "hello*", "world", "tests/fixtures/"]);
    let text = stdout(&out);

    // hello.txt is the only .txt with "world", but it's excluded by -x
    assert!(!text.contains("hello.txt"), "hello.txt should be excluded");
}

#[test]
fn log_only_with_ignore_errors() {
    let out = qae(&["--log-only", "-x", "*.rs", "test", "."]);

    assert!(!out.status.success(), "--log-only -x should fail");
    assert!(
        stderr(&out).contains("--log-only and --ignore are mutually exclusive"),
        "should show appropriate error message"
    );
}

// --- .ignore file support ---

#[test]
fn dot_ignore_file_excludes_from_content_search() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("keep.txt"), "findme here").unwrap();
    fs::write(tmp.path().join("skip.log"), "findme here too").unwrap();
    fs::write(tmp.path().join(".ignore"), "*.log\n").unwrap();

    let out = qae(&["-c", "findme", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(text.contains("keep.txt"), "should find match in non-ignored file");
    assert!(!text.contains("skip.log"), ".ignore should exclude .log files");
}

#[test]
fn dot_ignore_file_excludes_from_name_search() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("hello.txt"), "content").unwrap();
    fs::write(tmp.path().join("hello.log"), "content").unwrap();
    fs::write(tmp.path().join(".ignore"), "*.log\n").unwrap();

    let out = qae(&["-n", "hello", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(text.contains("hello.txt"), "should find non-ignored file by name");
    assert!(!text.contains("hello.log"), ".ignore should exclude .log files from name search");
}

#[test]
fn dot_ignore_file_excludes_directory() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("root.txt"), "findme").unwrap();
    let build_dir = tmp.path().join("build");
    fs::create_dir(&build_dir).unwrap();
    fs::write(build_dir.join("output.txt"), "findme").unwrap();
    fs::write(tmp.path().join(".ignore"), "build/\n").unwrap();

    let out = qae(&["-c", "findme", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(text.contains("root.txt"), "should find match in root");
    assert!(!text.contains("output.txt"), ".ignore should exclude build/ directory");
}

#[test]
fn no_ignore_flag_overrides_dot_ignore() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("data.log"), "findme").unwrap();
    fs::write(tmp.path().join(".ignore"), "*.log\n").unwrap();

    let out = qae(&["-c", "--no-ignore", "findme", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    assert!(text.contains("data.log"), "--no-ignore should override .ignore");
}

// --- rg-compat output mode (--color=ansi) ---

#[test]
fn ansi_output_has_rg_escape_codes() {
    let out = qae(&["--color=ansi", "world", "tests/fixtures/"]);
    let text = stdout(&out);

    // Path should be wrapped in magenta: \x1b[0m\x1b[35m ... \x1b[0m
    assert!(text.contains("\x1b[0m\x1b[35m"), "path should be magenta");
    // Line number should be wrapped in green: \x1b[0m\x1b[32m ... \x1b[0m
    assert!(text.contains("\x1b[0m\x1b[32m"), "line number should be green");
    // Match should be bold red: \x1b[0m\x1b[1m\x1b[31m ... \x1b[0m
    assert!(text.contains("\x1b[0m\x1b[1m\x1b[31m"), "match should be bold red");
}

#[test]
fn ansi_output_format_matches_rg() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("test.txt"), "foo bar baz\n").unwrap();

    let out = qae(&["--color=ansi", "bar", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    // Should be: MAGENTA_path RESET : GREEN_1 RESET : foo BOLD_RED bar RESET  baz
    let path = tmp.path().join("test.txt").to_string_lossy().to_string();
    let expected = format!(
        "\x1b[0m\x1b[35m{path}\x1b[0m:\x1b[0m\x1b[32m1\x1b[0m:foo \x1b[0m\x1b[1m\x1b[31mbar\x1b[0m baz"
    );
    assert_eq!(text.trim(), expected, "ANSI output should be byte-identical to rg format");
}

#[test]
fn ansi_output_multiple_matches_per_line() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("test.txt"), "ab ab ab\n").unwrap();

    let out = qae(&["--color=ansi", "ab", tmp.path().to_str().unwrap()]);
    let text = stdout(&out);

    // Count occurrences of bold-red escape sequence
    let bold_red = "\x1b[0m\x1b[1m\x1b[31m";
    let count = text.matches(bold_red).count();
    assert_eq!(count, 3, "should highlight all 3 occurrences of 'ab'");
}

#[test]
fn ansi_output_binary_file_warning_on_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("data.bin"), b"hello\x00world").unwrap();

    let out = qae(&["--color=ansi", "hello", tmp.path().to_str().unwrap()]);

    // Binary file should produce a WARNING on stderr, not stdout
    let err = stderr(&out);
    assert!(err.contains("WARNING"), "binary file should produce stderr warning");
    assert!(err.contains("data.bin"), "warning should mention the file");

    // stdout should NOT contain binary content
    let text = stdout(&out);
    assert!(!text.contains("hello"), "binary content should not appear on stdout");
}

#[test]
fn ansi_output_is_case_sensitive_by_default() {
    let out = qae(&["--color=ansi", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // greeting.rs has lowercase "hello" — should match
    assert!(text.contains("greeting.rs"), "should find lowercase hello");
    // hello.txt has "Hello" with capital H — should not match
    assert!(!text.contains("hello.txt"), "case-sensitive: should not match Hello");
}

// --- Smart case (--smart-case / -S) ---

#[test]
fn smart_case_lowercase_pattern_is_insensitive() {
    let out = qae(&["-c", "-S", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // "hello" (all lowercase) + smart-case → case-insensitive
    assert!(text.contains("greeting.rs"), "should match lowercase hello");
    assert!(text.contains("hello.txt"), "smart-case: lowercase pattern should match Hello");
}

#[test]
fn smart_case_uppercase_pattern_is_sensitive() {
    let out = qae(&["-c", "-S", "Hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // "Hello" (has uppercase) + smart-case → case-sensitive
    assert!(text.contains("hello.txt"), "should match Hello in hello.txt");
    assert!(!text.contains("greeting.rs"), "smart-case: uppercase pattern should not match lowercase hello");
}

#[test]
fn smart_case_with_ansi_output() {
    let out = qae(&["--color=ansi", "--smart-case", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // Lowercase pattern + smart-case → insensitive → should match "Hello" in hello.txt
    assert!(text.contains("hello.txt"), "smart-case should work in ANSI mode");
}

// --- --case-sensitive overrides ---

#[test]
fn case_sensitive_overrides_smart_case() {
    let out = qae(&["-c", "--smart-case", "--case-sensitive", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    // Even though smart-case would make "hello" insensitive,
    // --case-sensitive takes priority
    assert!(text.contains("greeting.rs"), "should match lowercase hello");
    assert!(!text.contains("hello.txt"), "--case-sensitive should override --smart-case");
}

#[test]
fn case_sensitive_overrides_ignore_case() {
    let out = qae(&["-c", "-i", "--case-sensitive", "hello", "tests/fixtures/"]);
    let text = stdout(&out);

    assert!(text.contains("greeting.rs"), "should match lowercase hello");
    assert!(!text.contains("hello.txt"), "--case-sensitive should override -i");
}

// --- --type-list ---

#[test]
fn type_list_prints_file_types() {
    let out = qae(&["--type-list"]);
    let text = stdout(&out);

    assert!(out.status.success(), "--type-list should succeed");
    // Should contain well-known types
    assert!(text.contains("rust"), "should list rust type");
    assert!(text.contains("*.rs"), "rust type should include *.rs glob");
    assert!(text.contains("python"), "should list python type");
}

#[test]
fn type_list_works_without_pattern() {
    // --type-list should not require a pattern
    let out = qae(&["--type-list"]);
    assert!(out.status.success(), "--type-list should work without a pattern");
    assert!(!stdout(&out).is_empty(), "should produce output");
}

// --- --no-ignore-vcs ---

#[test]
fn no_ignore_vcs_disables_gitignore_but_keeps_dot_ignore() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();

    // Init a git repo
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
        assert!(out.status.success(), "git {:?} failed", args);
    };

    git(&["init"]);

    // .gitignore excludes *.log, .ignore excludes *.tmp
    fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    fs::write(repo.join(".ignore"), "*.tmp\n").unwrap();
    fs::write(repo.join("keep.txt"), "findme").unwrap();
    fs::write(repo.join("skip.log"), "findme").unwrap();
    fs::write(repo.join("skip.tmp"), "findme").unwrap();

    git(&["add", "."]);
    git(&["commit", "-m", "init"]);

    // Without --no-ignore-vcs: both .log and .tmp are excluded
    let out = qae(&["-c", "findme", repo.to_str().unwrap()]);
    let text = stdout(&out);
    assert!(text.contains("keep.txt"));
    assert!(!text.contains("skip.log"), ".gitignore should exclude .log");
    assert!(!text.contains("skip.tmp"), ".ignore should exclude .tmp");

    // With --no-ignore-vcs: .log is included (gitignore disabled), .tmp still excluded (.ignore kept)
    let out = qae(&["-c", "--no-ignore-vcs", "findme", repo.to_str().unwrap()]);
    let text = stdout(&out);
    assert!(text.contains("keep.txt"));
    assert!(text.contains("skip.log"), "--no-ignore-vcs should include .log files");
    assert!(!text.contains("skip.tmp"), ".ignore should still exclude .tmp");
}

// --- rg-compat no-op flags ---

#[test]
fn noop_flags_accepted_silently() {
    // These flags should be accepted without error (deadgrep passes them)
    let out = qae(&[
        "--color=ansi",
        "--line-number",
        "--no-heading",
        "--no-column",
        "--with-filename",
        "--no-config",
        "world",
        "tests/fixtures/",
    ]);

    assert!(out.status.success(), "no-op flags should not cause errors");
    let text = stdout(&out);
    assert!(text.contains("world"), "search should still work with no-op flags");
}

// --- Context flags accepted ---

#[test]
fn context_flags_accepted() {
    // -B and -A should be accepted (even if not yet implemented)
    let out = qae(&["-c", "-B", "2", "-A", "2", "hello", "tests/fixtures/"]);

    assert!(out.status.success(), "-B/-A flags should be accepted");
    let text = stdout(&out);
    assert!(text.contains("greeting.rs"), "search should still work with context flags");
}
