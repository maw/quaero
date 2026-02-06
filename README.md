# qae

A quick search tool that combines [ripgrep](https://github.com/BurntSushi/ripgrep) and [fd](https://github.com/sharkdp/fd) to search both file contents and file names with a single command.

The name comes from Latin *quaero* — "I search".

## Installation

```bash
cargo install --path .
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
