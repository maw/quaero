# qae - Quick Search Tool

## Project Overview

`qae` (from Latin "quaero" - I search) is a command-line tool that combines
the power of `ripgrep` (rg) and `fd` to perform unified searches across both
file contents and file names. It's designed to help developers quickly explore
and navigate large, deeply-nested directory structures.

## Goals

- **Unified Search**: Search both file contents (via ripgrep) and file names
  (via fd) with a single command
- **Learning Project**: Built in Rust as a learning vehicle for the language
- **Sophistication**: More than just a shell wrapper - intelligent result
  merging, parallel execution, smart filtering
- **Ergonomics**: Simple, intuitive CLI that feels natural to use daily

## Implementation Language: Rust

### Why Rust?
- Perfect ecosystem fit (rg and fd are both Rust tools)
- Performance and safety
- Great for CLI tools
- Learning opportunity

### Development Approach
1. **Phase 1 (MVP)**: Use ripgrep/fd libraries for content and filename search
2. **Phase 2 (Sophistication)**: Parallel execution, smart merging, advanced
   features

## Core Features (MVP)

### Basic Usage
```bash
# Search for pattern in both file names and contents
qae "search_term"

# Search in specific directory
qae "search_term" /path/to/dir

# Search only file names
qae --names-only "search_term"

# Search only file contents
qae --content-only "search_term"
```

### Command Structure
- **Pattern**: The search term/regex
- **Path**: Optional directory to search (defaults to current directory)
- **Flags**: Control what gets searched and how results are displayed

## Technical Design

### Dependencies
- `clap` - Command-line argument parsing
- `grep-regex` - Regex matcher for ripgrep's searcher
- `grep-searcher` - Content searching (ripgrep's engine)
- `ignore` - Directory walking with .gitignore and .ignore support
- `regex` - Filename pattern matching

### Data Flow (MVP)
1. Parse CLI arguments
2. Build regex matchers for content and filenames
3. Use `ignore::WalkBuilder` to traverse directory tree
4. For each file:
   - Check filename against pattern
   - Use `grep-searcher` to search contents
5. Collect and merge results
6. Format and display output

### Result Structure
Each result should include:
- Match type (filename or content)
- File path
- Line number (for content matches)
- Matched line preview (for content matches)
- Highlighted match within result

## CLI Design

### Arguments
- `PATTERN` - Search pattern (required)
- `[PATH]` - Directory to search (optional, defaults to `.`)

### Flags
- `-n, --names-only` - Only search file names
- `-c, --content-only` - Only search file contents
- `-v, --verbose` - Show detailed output
- `-i, --ignore-case` - Case-insensitive search
- `--hidden` - Include hidden files
- `--no-ignore` - Don't respect .gitignore or .ignore
- `-t, --type FILTER` - Filter by file type (e.g., rust, python)

### Output Format
```
Filename matches:
  src/main.rs
  tests/integration_test.rs

Content matches:
  src/lib.rs:42: pub fn search_pattern(pattern: &str) -> Result<Vec<Match>> {
  src/main.rs:15: let pattern = args.pattern;
```

## Future Enhancements (Post-MVP)

- **Caching**: Cache search results for faster repeated searches
- **Interactive Mode**: Use something like `skim` for interactive result
  filtering
- **Fuzzy Matching**: Optional fuzzy search for filenames
- **Color Themes**: Customizable output colors
- **Result Scoring**: Rank results by relevance
- **Ignore Patterns**: Custom ignore patterns beyond .gitignore
- **Export Results**: Save results to file in various formats

## Development Notes

### Getting Started
```bash
# Create project
cargo new qae
cd qae

# Add dependencies to Cargo.toml
# Start with basic CLI parsing
cargo run -- "test" .
```

### Learning Resources
- The Rust Book: https://doc.rust-lang.org/book/
- ripgrep source: https://github.com/BurntSushi/ripgrep
- fd source: https://github.com/sharkdp/fd
- Command Line Rust: https://www.oreilly.com/library/view/command-line-rust/9781098109424/

### Testing Strategy
- Unit tests for core logic
- Integration tests with real directory structures
- Test with various edge cases (empty dirs, permission errors, etc.)

## Project Status

**Current Phase**: MVP
**Completed**:
1. Project structure with clap CLI parsing
2. Content search using grep-searcher/grep-regex
3. Filename search using regex + ignore crate
4. Combined output with section headers
**Next Steps**:
1. Highlighted matches in output
2. Parallel execution
3. Result scoring/ranking

## Notes

- Focus on getting something working first, optimize later
- Learn Rust concepts incrementally through building features
- Keep code simple and readable - optimize for learning
- Commit frequently with good messages

## Questions to Explore

- How to best merge and deduplicate results?
- What's the right balance of features vs. simplicity?

## Issue Tracking

This project uses **bd (beads)** for issue tracking.
Run `bd prime` for workflow context.

**Quick reference:**
- `bd ready` - Find unblocked work
- `bd create "Title" --type task --priority 2` - Create issue
- `bd update <id> --status in_progress` - Claim work
- `bd close <id>` - Complete work
- `bd list` - See all open issues
