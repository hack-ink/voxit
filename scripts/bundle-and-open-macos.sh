#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

cargo bundle --release -p voxit
open target/release/bundle/osx/Voxit.app
