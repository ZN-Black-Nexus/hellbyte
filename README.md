# Hellbyte

A tiny, libre, retro first-person shooter written in pure Rust. It plays
like the classic 1993-era shooters: fast strafing, keycards, lifts, secrets,
hitscan husks and fireball-throwing fiends. Every line of code, every
texture, creature, level and word of text is original and free software
(GPL-3.0-or-later).

* **One file per platform.** A single static binary (~0.6 MB) with the whole
  game inside: engine, 5 levels, all textures. Nothing to install, no data
  files, no libraries (on Linux not even libc).
* **Under 1 MB of RAM.** The whole process peaks at about 0.8 MB resident on
  Linux; the engine's mutable state is ~200 KB.
* **Runs on almost any CPU.** Integer-only maths (no FPU needed) and a
  hand-written system-call layer for each architecture. Big- and
  little-endian, 32- and 64-bit, routers to mainframes.
* **Plays anywhere there's a screen or a terminal:** an X11 window, the
  Linux framebuffer, a native Windows window, a native macOS window, or any
  terminal (including over SSH or a serial line).

## The game

Episode 1, *Signal Lost*, five levels: **Intake**, **Coolant Works**, **Relay
Tower**, **Deep Storage** and **The Breach**.

* 8 weapons: fists, power drill, sidearm, scattergun, rotary cannon, rocket
  launcher, pulse rifle and the Arc Cannon.
* 7 creatures, all original designs: husk drones (rifle), enforcers
  (shotgun), heavies (rotary gun), fiends (claws and fireballs), rippers
  (charging jaws), gazers (floating plasma bells) and juggernauts (stone
  golems with molten cores).
* Classic mechanics: momentum and friction, wall sliding, stepping and
  dropping, auto-aim, splash damage, infighting, monsters woken by gunfire,
  doors (normal, fast, locked, secret), lifts, crushers, stairs, teleporters,
  flickering and strobing lights, damaging floors, secrets, par times and a
  tally screen, automap, 5 skill levels, 6 save slots, options.
* Monsters are tiny 3D models rendered into sprites on the fly, so they can
  be seen from 16 angles and animate smoothly without storing any artwork.

## Download / build outputs

`scripts/build-all.sh` produces everything in `dist/`:

| OS | Binaries |
|----|----------|
| Linux (static, no libc) | x86_64, i686, i586 (Pentium/MMX), aarch64, aarch64-BE, ARMv7 hard/soft-float, ARMv6 hard/soft-float, ARMv5TE, ARMv4T, ARMv4 (StrongARM/FA526), ARM big-endian, MIPS32 BE/LE, MIPS64 BE (OpenWrt), MIPS64 BE/LE, PowerPC 32 (+ e500 SPE), PowerPC64 BE/LE, RISC-V 64/32, LoongArch64, s390x, SPARC64, SPARC32, Hexagon, C-SKY |
| Windows | x86_64, x86 (32-bit, subsystem 5.01 so it also starts on very old Windows), ARM64 — native Win32 `.exe`, no C runtime |
| macOS | one universal binary (Apple Silicon + Intel), links only `libSystem` |

The Linux binaries don't depend on the distribution. `linux-x86_64` runs on
any 64-bit PC Linux (Debian/Ubuntu amd64, Fedora, Arch, …) and
`linux-aarch64` on any 64-bit ARM Linux (Debian/Ubuntu arm64, Raspberry Pi
OS 64-bit, …), whatever the kernel's page size (4, 16 or 64 KB). Downloaded
files need `chmod +x` first.

### OpenWrt package architecture → binary

| OpenWrt arch | Use |
|---|---|
| `mips_24kc`, `mips_4kec`, `mips_mips32` | `linux-mips32be` |
| `mipsel_24kc`, `mipsel_74kc`, `mipsel_mips32` | `linux-mips32le` |
| `mips64_octeonplus`, `mips64_mips64r2` | `linux-mips64be-openwrt` |
| `mips64el_mips64r2` | `linux-mips64le` |
| `aarch64_*` | `linux-aarch64` |
| `arm_cortex-a*` (with VFP/NEON) | `linux-armv7-hf` |
| `arm_cortex-a*` (no FPU) | `linux-armv7-sf` |
| `arm_arm1176jzf-s_vfp` | `linux-armv6-hf` |
| `arm_arm926ej-s`, `arm_xscale` | `linux-armv5te` |
| `arm_fa526` | `linux-armv4-strongarm-fa526` |
| `i386_pentium-mmx` | `linux-i586-pentium` |
| `i386_pentium4` | `linux-i686` |
| `powerpc_464fp` | `linux-powerpc32` |
| `powerpc_8548` | `linux-powerpc32-spe-e500` |
| `powerpc64_e5500` | `linux-powerpc64be` |
| `riscv64_riscv64` | `linux-riscv64` |
| `loongarch64_generic` | `linux-loongarch64` |
| `x86_64` | `linux-x86_64` |

Routers have no screen: copy the binary over (`scp`), then play it in your
terminal over SSH: `./hellbyte-linux-mips32le`.

## Playing

```
hellbyte               # window on desktops; framebuffer on consoles; terminal otherwise
hellbyte --term        # force the terminal renderer (works over SSH / serial)
hellbyte --fb          # Linux framebuffer + evdev (needs the video/input groups)
hellbyte --map 3       # jump to a level        --skill 1..5
hellbyte --scale 4     # starting window size  --fullscreen
hellbyte --help
```

| Action | Keys |
|---|---|
| Move / turn | arrows, `W` `A` `S` `D`, mouse |
| Strafe | `A` `D`, `,` `.`, or Alt + turn |
| Fire | Ctrl, `F`, `K`, left mouse |
| Use / open | Space, `E`, right mouse |
| Run | Shift (or *Always run* in Options) |
| Weapons | `1`–`7` (press `1` twice for the drill) |
| Automap | Tab (`+` `-` zoom) |
| Menu / save / load | Esc, F2 / F3, quick save F6 / quick load F9 |
| Pause | `P` |
| Fullscreen | Alt+Enter (Option+Enter on macOS); windows can also be resized or maximized |

Saves live in `~/.local/share/hellbyte` (Linux), `%APPDATA%\Hellbyte`
(Windows) or `~/Library/Application Support/Hellbyte` (macOS).

**Terminal tips:** the game draws with half-block characters, so a smaller
font means more pixels (up to 320x200). Truecolor terminals look best
(`--colors 24|256|16|2` to override). Terminals that support the kitty
keyboard protocol (kitty, foot, WezTerm, Ghostty, recent Alacritty) report
real key releases; in others a held key is inferred from auto-repeat, which
feels a little sticky. Ctrl+C always quits.

## Building

Everything is Rust; no C compiler is involved.

```sh
cargo run --release                                    # Linux dev build with your host toolchain
cargo build --release --target x86_64-unknown-linux-musl   # static, libc-free Linux binary

scripts/fetch-tools.sh    # nightly + rust-src, qemu (tests), GNU ld for SPARC and C-SKY
scripts/build-all.sh      # every OS and CPU into dist/   (scripts/build-all.sh mips  = filter)
scripts/test-qemu.sh      # run every Linux binary under qemu and compare with native
```

Targets other than your host must be built with an explicit `--target`
(the per-target linker settings live in `.cargo/config.toml`). Windows and
macOS binaries can be built from Linux: Win32 is imported with `raw-dylib`
and macOS links against a small hand-written `libSystem` stub
(`crates/hellbyte/macos/libSystem.tbd`), so no Windows or Apple SDK is needed.

## How it fits in so little memory

* No heap at all: the engine (`#![no_std]`) is one static structure of ~200 KB
  in `.bss` — a 320x200 8-bit screen, a fixed pool of 512 objects, and the
  mutable parts of the current level.
* Levels are compiled at build time into BSP trees and blockmaps that stay in
  the binary's read-only data (flash on devices that execute in place).
* Textures are generated at build time; creatures, items, weapons and the sky
  are drawn procedurally at run time.
* Floors and ceilings are drawn column by column as walls are processed, so
  the large "visplane" buffers classic engines used are not needed.

Memory per level, Linux x86_64 (`hellbyte --bench --tics 700 --map N`):
peak resident set 0.75–0.85 MB for the whole process, including its code.

### Even smaller devices

The engine crate compiles for bare-metal microcontrollers
(`thumbv6m-none-eabi`, `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf`,
…). A board port supplies a display (8-bit pixels + a palette), buttons, a
35 Hz tick, and optionally storage for saves — see `crates/hellbyte/src/` for
the existing frontends. Lower `MAX_W`/`MAX_H`/`MAX_MOBJS` in
`crates/engine/src/limits.rs` for parts with less than ~256 KB of RAM.

## Testing

* `scripts/test-qemu.sh` plays the same scripted session on every Linux CPU
  under qemu-user and checks that the final frame and game state hash to the
  same value as the native build. The engine is deterministic, so any
  endianness, word-size or code-generation bug shows up as a mismatch.
* Save → load round trips are bit-exact (same hash after continuing).
* `--shot FILE.png` renders a frame headlessly (`--warp X,Y,ANGLE`, `--show map`,
  `--script w:40,right:10,f:5`) — handy for level design.

## Project layout

```
crates/engine     the game: renderer, physics, AI, weapons, specials, menus, HUD, saves (no_std)
crates/engine/build   build-time generation of trig tables, palette, light maps, textures
crates/mapc       level compiler: layered polygons -> planar map -> BSP + blockmap
crates/rt         memcpy & friends in Rust (no libc anywhere)
crates/hellbyte   platform frontends: Linux (syscalls, X11 protocol, fbdev, terminal),
                  Windows (Win32), macOS (libSystem + Cocoa via the Objective-C runtime)
levels/           the level sources (plain text) and the level-format guide
scripts/          build, test and tooling scripts
```

## Status and known limits

* Linux builds are tested: natively (window via X11, terminal, headless) and
  under qemu for every foreign CPU. The framebuffer backend could not be
  exercised on the build machine (no access to `/dev/fb0`).
* The Windows and macOS builds compile and link but have not been run yet.
* Not built: **m68k** (LLVM's M68k backend is experimental and currently
  miscompiles this code) and **MIPS R6** (LLVM crashes). **C-SKY** builds
  but has no emulator available to test with.
* CPUs without a Rust/LLVM backend cannot be targeted: ARC, SuperH (sh4),
  OpenRISC, Xtensa Linux, MicroBlaze, Nios II, Alpha, PA-RISC, Itanium.
* Wayland desktops are supported through XWayland. There is no sound yet;
  the libre Freedoom project could be a future source of sound effects.

## License

GPL-3.0-or-later. See `LICENSE`. All art is generated by code in this
repository; there are no third-party assets. Hellbyte is an independent
project and is not affiliated with any other game or company.
