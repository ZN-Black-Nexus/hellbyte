#!/usr/bin/env bash
# Build Hellbyte for every supported platform/CPU into dist/.
#   scripts/build-all.sh            build everything
#   scripts/build-all.sh mips       only targets whose triple or name matches "mips"
# Needs: rustup nightly with rust-src (cores for tier-3 targets are built from source).
set -u
cd "$(dirname "$0")/.."
. scripts/targets.sh
ONLY=${1:-}
mkdir -p dist
LLVM_BIN=$(ls -d "$(rustc +nightly --print sysroot)"/lib/rustlib/*/bin | head -1)
ok=0; fail=0
build() { # triple name extra-rustflags
  local t=$1 name=$2 extra=$3 tdir="target/variants/$2"
  local env=()
  local v4bx=0
  if [ "$extra" = "V4BX" ]; then
    # ARMv4 (no BX): build as ARMv4T with symbols, then rewrite BX -> MOV PC.
    v4bx=1; extra=""; env=(env CARGO_PROFILE_RELEASE_STRIP=false)
  fi
  if [ -n "$extra" ]; then
    # Append variant flags to the per-target flags from .cargo/config.toml.
    local base
    base=$(awk -v t="[target.$t]" '$0==t{f=1;next} f&&/^rustflags/{print;exit}' .cargo/config.toml | sed -E 's/^rustflags = \[//; s/\]$//; s/"//g; s/, / /g')
    env=(env "CARGO_TARGET_$(echo "$t" | tr 'a-z.-' 'A-Z__')_RUSTFLAGS=$base $extra")
  fi
  local log
  log=$("${env[@]}" cargo +nightly build -q --release --target "$t" --target-dir "$tdir" \
        -Z build-std=core,compiler_builtins 2>&1)
  if [ -f "$tdir/$t/release/hellbyte" ] && ! echo "$log" | grep -q "^error"; then
    if [ $v4bx = 1 ]; then
      python3 scripts/fix_v4bx.py "$tdir/$t/release/hellbyte" "dist/hellbyte-$name.tmp" >/dev/null &&
      "$LLVM_BIN/llvm-strip" --strip-all -o "dist/hellbyte-$name" "dist/hellbyte-$name.tmp" && rm -f "dist/hellbyte-$name.tmp"
    else
      cp "$tdir/$t/release/hellbyte" "dist/hellbyte-$name"
    fi
    printf "  %-34s %-30s %8d bytes\n" "$name" "$t" "$(stat -c %s "dist/hellbyte-$name")"
    ok=$((ok+1))
  else
    printf "  %-34s %-30s FAILED\n" "$name" "$t"
    echo "$log" | grep -E "^error" -A8 | head -20 | sed 's/^/      /'
    fail=$((fail+1))
  fi
}
echo "Linux (static, no libc):"
while IFS='|' read -r t name q extra; do
  [ -z "$t" ] && continue
  if [ -n "$ONLY" ] && [[ "$t" != *$ONLY* && "$name" != *$ONLY* ]]; then continue; fi
  build "$t" "$name" "$extra"
done <<< "$LINUX_TARGETS"
echo
echo "Windows (native Win32 .exe, no C runtime):"
for spec in "x86_64-pc-windows-msvc|windows-x86_64" "i686-pc-windows-msvc|windows-x86-32bit" "aarch64-pc-windows-msvc|windows-arm64"; do
  t=${spec%%|*}; name=${spec##*|}
  if [ -n "$ONLY" ] && [[ "$t" != *$ONLY* && "$name" != *$ONLY* ]]; then continue; fi
  log=$(cargo +nightly build -q --release --target "$t" -Z build-std=core,compiler_builtins 2>&1)
  if [ -f "target/$t/release/hellbyte.exe" ] && ! echo "$log" | grep -q "^error"; then
    cp "target/$t/release/hellbyte.exe" "dist/hellbyte-$name.exe"
    printf "  %-34s %-30s %8d bytes\n" "$name.exe" "$t" "$(stat -c %s "dist/hellbyte-$name.exe")"; ok=$((ok+1))
  else
    printf "  %-34s %-30s FAILED\n" "$name" "$t"; echo "$log" | grep -E "^error" -A8 | head -20 | sed 's/^/      /'; fail=$((fail+1))
  fi
done
echo
echo "macOS (native Mach-O, links only libSystem):"
mac=()
for spec in "aarch64-apple-darwin|macos-arm64" "x86_64-apple-darwin|macos-x86_64"; do
  t=${spec%%|*}; name=${spec##*|}
  if [ -n "$ONLY" ] && [[ "$t" != *$ONLY* && "$name" != *$ONLY* ]]; then continue; fi
  log=$(cargo +nightly build -q --release --target "$t" -Z build-std=core,compiler_builtins 2>&1)
  if [ -f "target/$t/release/hellbyte" ] && ! echo "$log" | grep -q "^error"; then
    cp "target/$t/release/hellbyte" "dist/hellbyte-$name"; mac+=("dist/hellbyte-$name")
    printf "  %-34s %-30s %8d bytes\n" "$name" "$t" "$(stat -c %s "dist/hellbyte-$name")"; ok=$((ok+1))
  else
    printf "  %-34s %-30s FAILED\n" "$name" "$t"; echo "$log" | grep -E "^error" -A8 | head -20 | sed 's/^/      /'; fail=$((fail+1))
  fi
done
if [ ${#mac[@]} -eq 2 ]; then
  python3 scripts/lipo.py dist/hellbyte-macos-universal "${mac[@]}" | sed 's/^/  /'
fi
echo
echo "built $ok, failed $fail"
