#!/bin/bash
# shellcheck disable=SC2002

set -eu
set -o pipefail
IFS=$'\n\t'

# print useful message on failure
trap 's=$?; echo >&2 "$0: Error on line "$LINENO": $BASH_COMMAND"; exit $s' ERR

# shellcheck disable=SC2034
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

cargo build
mkdir -p "$HOME/.local/bin"
cp target/debug/qro "$HOME/.local/bin"

