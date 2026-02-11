# qae

A quick search tool that combines some of the functionality of
[ripgrep](https://github.com/BurntSushi/ripgrep) and
[fd](https://github.com/sharkdp/fd) to search both file contents and file
names with a single command.

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
# Binary is at target/release/qae
```

## Usage

```bash
# Search file names and contents
qae "pattern"

# Search in a specific directory
qae "pattern" /path/to/dir

# Only file names
qae -n "pattern"

# Only file contents
qae -c "pattern"

# Case-insensitive, include hidden files
qae -i --hidden "pattern"

# Filter by file type
qae -t rust "pattern"

# Literal string search (no regex)
qae -F "exact.match"

# Glob matching on file names
qae -g "*.rs"

# Whole-word matching
qae -w "main"

# Include git log (commit messages) in search
qae -l "refactor"

# Search only git log
qae --log-only "bugfix"
```

## Ignore files

qae respects `.ignore` files, which use the same glob syntax as `.gitignore`
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
