# Target list shared by build-all.sh and test-qemu.sh.
# Format: <rust target triple>|<dist name>|<qemu binary or ->|<extra rustflags>
# m68k is left out for now: LLVM's M68k backend is experimental (miscompiles and
# overflows 16-bit relocations on a binary this size). Re-add it once LLVM matures:
#   m68k-unknown-linux-gnu|linux-m68k|m68k|-C target-cpu=M68020
LINUX_TARGETS="
x86_64-unknown-linux-musl|linux-x86_64|x86_64|
i686-unknown-linux-musl|linux-i686|i386|
i586-unknown-linux-musl|linux-i586-pentium|i386|
aarch64-unknown-linux-musl|linux-aarch64|aarch64|
aarch64_be-unknown-linux-musl|linux-aarch64be|aarch64_be|
armv7-unknown-linux-musleabihf|linux-armv7-hf|arm|
armv7-unknown-linux-musleabi|linux-armv7-sf|arm|
arm-unknown-linux-musleabihf|linux-armv6-hf|arm|
arm-unknown-linux-musleabi|linux-armv6-sf|arm|
armv5te-unknown-linux-musleabi|linux-armv5te|arm|
armv4t-unknown-linux-gnueabi|linux-armv4t|arm|
armv4t-unknown-linux-gnueabi|linux-armv4-strongarm-fa526|arm|V4BX
armeb-unknown-linux-gnueabi|linux-armeb|armeb|
mips-unknown-linux-musl|linux-mips32be|mips|-C target-cpu=mips32
mipsel-unknown-linux-musl|linux-mips32le|mipsel|-C target-cpu=mips32
mips64-openwrt-linux-musl|linux-mips64be-openwrt|mips64|
mips64-unknown-linux-muslabi64|linux-mips64be|mips64|
mips64el-unknown-linux-muslabi64|linux-mips64le|mips64el|
powerpc-unknown-linux-musl|linux-powerpc32|ppc|
powerpc-unknown-linux-muslspe|linux-powerpc32-spe-e500|ppc|
powerpc64-unknown-linux-musl|linux-powerpc64be|ppc64|
powerpc64le-unknown-linux-musl|linux-powerpc64le|ppc64le|
riscv64gc-unknown-linux-musl|linux-riscv64|riscv64|
riscv32gc-unknown-linux-musl|linux-riscv32|riscv32|
loongarch64-unknown-linux-musl|linux-loongarch64|loongarch64|
s390x-unknown-linux-musl|linux-s390x|s390x|
sparc64-unknown-linux-gnu|linux-sparc64|sparc64|
sparc-unknown-linux-gnu|linux-sparc32|sparc32plus|
hexagon-unknown-linux-musl|linux-hexagon|hexagon|
csky-unknown-linux-gnuabiv2|linux-csky|-|
"
