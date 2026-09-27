#!/usr/bin/env bash
# Run a scripted game session on every built Linux binary under qemu-user and
# check that the resulting frame/state hash matches the native x86_64 run.
set -u
cd "$(dirname "$0")/.."
. scripts/targets.sh
Q=${QEMU_DIR:-$HOME/.cache/hellbyte-tools/qemu/usr/bin}
ARGS=(--bench --tics 700 --script "w:40,right:30,w:60,left:50,w:100,f:40,e:3,w:60")
ref=$(dist/hellbyte-linux-x86_64 "${ARGS[@]}" | sed -E 's/.*hash //')
echo "reference hash (x86_64 native): $ref"
pass=0; failn=0
# Oldest matching core per binary, so instructions from a newer ISA trap.
# ARMv4T runs on arm926: qemu's only v4T model (ti925t) aborts in user mode.
# The StrongARM build is the same code with BX rewritten, and sa1110 (ARMv4,
# no BX/Thumb) rejects every v5 instruction, so it covers the v4T build too.
declare -A CPU=( [linux-powerpc32-spe-e500]="-cpu e500v2" [linux-armv4-strongarm-fa526]="-cpu sa1110" [linux-armv4t]="-cpu arm926" [linux-armv5te]="-cpu arm926" [linux-armv6-hf]="-cpu arm1176" [linux-armv6-sf]="-cpu arm1176" )
while IFS='|' read -r t name q extra; do
  [ -z "$t" ] && continue
  bin=dist/hellbyte-$name
  [ -f "$bin" ] || continue
  if [ "$q" = "-" ]; then printf "  %-34s (no qemu for this CPU; build-only)\n" "$name"; continue; fi
  t0=$(date +%s%N)
  out=$(timeout 300 "$Q/qemu-$q" ${CPU[$name]:-} "$bin" "${ARGS[@]}" 2>&1)
  ms=$(( ($(date +%s%N) - t0) / 1000000 ))
  h=$(echo "$out" | sed -nE 's/.*hash ([0-9a-f]+).*/\1/p')
  if [ "$h" = "$ref" ]; then r="OK"; pass=$((pass+1)); else r="MISMATCH ($h) ${out:0:60}"; failn=$((failn+1)); fi
  printf "  %-34s %-6s %6d ms emulated\n" "$name" "$r" "$ms"
done <<< "$LINUX_TARGETS"
echo "passed $pass, failed $failn"
