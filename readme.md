# MaBoy GameBoy Emulator

MaBoy is a fast and (mostly) accurate GameBoy emulator for Windows, written in Rust. It focuses heavily on performance, but without ever sacrificing emulation accuracy. At the same time, it attempts to minimize power consumption to make sure that you can use it on your laptop.

*This project is a work in progress. Currently, it can only run a limited amount of ROMs due to missing MBC support. Pokémon Games are not yet supported because of this.*

## Features

- [X] Fast
- [X] Accurate
- [X] Detailed debug output
- [X] CPU debugger w/ breakpoints, step command
- [X] GUI
  - [X] Open file dialog
  - [ ] Resizable game window
- [ ] High Compatibility: Not yet, some MBC implementations missing
- [ ] Configuration (Colors, Timing, ...)
- [ ] Visual Debuggers (Memory, VRAM, OAM RAM)
- [ ] Cross-platform support
- [ ] Well-documented source code

## Hardware Emulation

- [X] CPU
  - [X] Cycle-accurate
  - [X] Interrupt handling
- [X] PPU (almost cycle-accurate)
- [X] MBCs
  - [X] ROM only
  - [X] MBC1
  - [ ] All others
- [ ] APU
- [ ] Serial Port