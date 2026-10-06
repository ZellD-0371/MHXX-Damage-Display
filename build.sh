#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export PATH="/c/Users/Lawrence/.cargo/bin:/c/devkitPro/msys2/usr/bin:/c/devkitPro/devkitARM/bin:$PATH"
export CARGO_TARGET_ARMV7A_NONE_EABI_LINKER="C:/devkitPro/devkitARM/bin/arm-none-eabi-gcc.exe"
export CARGO_NET_OFFLINE=true
cd "$task_root"
cargo forge build --release
mv -- "mhgu_attack_damage_display.nro" "attack_damage_display.nro"
