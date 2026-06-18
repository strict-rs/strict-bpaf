# AGENTS.md

`comptester` is a test-support library for bpaf's **dynamic shell completion**. It drives a real shell inside a pseudo-terminal, types a command line ending in Tab, and screen-scrapes what the shell renders — so completion can be asserted end to end, the way a user actually sees it.

## How it works

- `src/lib.rs` exposes `zsh_comptest(input)` / `zsh_comptest_with(input, width)`, plus `bash_comptest`, `fish_comptest`, `elvish_comptest`. Each spawns the shell under `ptyprocess` (a real pty), feeds `input` (typically ending in `\t`), and returns the rendered screen.
- `src/vterm.rs` wraps the `vt100` crate (a complete VT102 emulator): `Term::process(bytes)` feeds the shell's raw pty output to a `vt100::Parser`, and `render()` reads the visible screen back out (trailing whitespace trimmed per line), so escape sequences, cursor motion, scroll regions and redraws all resolve to what's on screen — fish's completion pager included. Don't hand-roll escape-sequence handling here; delegate to the emulator.
- Consumed by `../docs2` (under its `comptester` feature) to render the `zsh>` completion blocks, and by completion integration tests.

## Running

- Requires the target shells installed and a usable pty; headless/CI environments lacking them will skip or fail completion tests. There is no regeneration step — this is a helper crate, exercised through the tests and features that call it.
