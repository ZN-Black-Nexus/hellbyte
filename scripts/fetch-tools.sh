#!/usr/bin/env bash
# Fetch the extra tools used by build-all.sh / test-qemu.sh into
# ~/.cache/hellbyte-tools (override with HELLBYTE_TOOLS). No root needed.
#
#   * Rust nightly + rust-src        (cores for tier-3 CPUs are built from source)
#   * qemu-user                      (only for *testing* foreign-CPU Linux builds)
#   * GNU ld for SPARC               (rust-lld has no SPARC backend)
#   * GNU ld for C-SKY               (built from the GNU binutils source)
#
# The qemu/binutils part uses Debian's `apt-get download`; on other systems
# install qemu-user and sparc64 binutils with your package manager instead.
set -eu
T=${HELLBYTE_TOOLS:-$HOME/.cache/hellbyte-tools}
mkdir -p "$T"
cd "$T"

echo "== rust nightly + rust-src"
rustup toolchain install nightly --profile minimal --component rust-src,llvm-tools

if command -v apt-get >/dev/null 2>&1; then
  if [ ! -x qemu/usr/bin/qemu-mips ]; then
    echo "== qemu-user (for tests only)"
    apt-get download qemu-user
    dpkg-deb -x qemu-user_*.deb qemu
  fi
  if [ ! -x binutils/usr/bin/sparc64-linux-gnu-ld ]; then
    echo "== binutils for SPARC"
    apt-get download binutils-sparc64-linux-gnu
    for f in binutils-*.deb; do dpkg-deb -x "$f" binutils; done
  fi
else
  echo "(no apt-get: install qemu-user and sparc64-linux-gnu binutils yourself)"
fi

if [ ! -x csky/bin/csky-linux-gnuabiv2-ld ]; then
  echo "== binutils for C-SKY (from source, a few minutes)"
  V=2.45
  [ -f binutils-$V.tar.xz ] || curl -fL -o binutils-$V.tar.xz https://ftp.gnu.org/gnu/binutils/binutils-$V.tar.xz
  rm -rf binutils-$V build-csky && tar xf binutils-$V.tar.xz && mkdir build-csky && cd build-csky
  ../binutils-$V/configure --target=csky-linux-gnuabiv2 --prefix="$T/csky" --disable-nls --disable-werror \
    --disable-gdb --disable-gdbserver --disable-sim --disable-gprof --disable-gprofng --disable-gold \
    --disable-libctf --disable-plugins --without-zstd >/dev/null
  make -j"$(nproc)" all-ld all-gas >/dev/null && make install-ld install-gas >/dev/null
fi
echo "tools ready in $T"
