# qro

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/maw/quaero)

A quick search tool that recursively searches content, filenames, and git log
messages.

The name comes from Latin *quaero* — "I search".

## Installation

```bash
# Install to ~/.cargo/bin (default)
cargo install --path .

# Or install to ~/.local/bin
cargo install --path . --root ~/.local
```

## Building from source

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release
# Binary is at target/release/qro
```

## Usage

```bash
# Search file names and contents
qro "pattern"

# Search in a specific directory
qro "pattern" /path/to/dir

# Only file names
qro -n "pattern"

# Only file contents
qro -c "pattern"

# Case-insensitive, include hidden files
qro -i --hidden "pattern"

# Filter by file type
qro -t rust "pattern"

# Literal string search (no regex)
qro -F "exact.match"

# Glob matching on file names
qro -g "*.rs"

# Whole-word matching
qro -w "main"

# Include git log (commit messages) in search
qro -l "refactor"

# Search only git log
qro --log-only "bugfix"
```

## Ignore files

qro respects `.ignore` files, which use the same glob syntax as `.gitignore`
but are not tied to git. Place a `.ignore` file in any directory to exclude
files and directories from search results:

```
# .ignore
*.log
build/
node_modules/
```

This is useful for excluding build artifacts, logs, or vendor directories
without polluting `.gitignore`. The `--no-ignore` flag disables both
`.gitignore` and `.ignore` processing.

Precedence (highest to lowest):
1. Command-line flags (`-x`, `-g`, `--no-ignore`)
2. `.ignore`
3. `.gitignore`
4. `.git/info/exclude`
5. Global gitignore

## Status

Early development.

## Similar tools

- [ripgrep](https://github.com/BurntSushi/ripgrep): faster than qro and more
  fully featured.
- [ag](https://github.com/ggreer/the_silver_searcher)
- [ack](https://github.com/petdance/ack3)
- [fd](https://github.com/sharkdp/fd)
