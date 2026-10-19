#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [ "$#" -ne 0 ] && [ "$#" -ne 2 ]; then
    printf '用法：bash test.sh ["Forge NSO" "MHGU ExeFS目录"]\n' >&2
    exit 2
fi
cd "$task_root"
mkdir -p "target"
"${RUSTC:-rustc}" --test --edition=2024 "src/data.rs" -o "target/data-tests.exe"
"./target/data-tests.exe"
node "tools/verify-build.cjs" "$@"
node "tools/verify-settings.cjs"
node "tools/verify-install.cjs"
