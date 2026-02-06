# qae

A quick search tool that combines [ripgrep](https://github.com/BurntSushi/ripgrep) and [fd](https://github.com/sharkdp/fd) to search both file contents and file names with a single command.

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
```

## Status

Early development — CLI skeleton only.
