#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export PATH="/c/Users/Lawrence/.cargo/bin:$PATH"
cd "$task_root"
rustc --test --edition=2024 "src/data.rs" -o "target/data-tests.exe"
"./target/data-tests.exe"
node "tools/verify-build.cjs"
