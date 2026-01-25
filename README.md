# Lean-TUI

This is a **terminal-only info view**, comparable to the VS Code info view for [Lean 4](https://lean-lang.org/). It shows:

- The active variable bindings for a developer writing code (term mode).
- The hypotheses and goals for a mathematician proving and formalizing proofs (tactic mode).

**Every editor with an LSP client can be used** in combination with this (Helix, Kate, Zed, Neovim, ...).

![(Screenshot of the prime number theorem proof)](./screenshot.jpg)

Integration with editor for cursor navigation:

- Use `j`, `k` to go up or down in hypotheses
- Click or press enter on hypotheses to jump to type definition in the editor
- Click on goals to go the goal in the editor
- Filter displayed assumptions (see help menu `?`)

Optional previous -> current -> next proof state layout (shows changes in proof state):

- Display multiple goals below each-other as rows in a grid
- Toggle display previous and next proof state as columns in grid with `p` and `n` (also works in term-mode)

Close with `q`

## Installation

You can choose between:

- Using multiple terminals (emulator windows) side-by-side (a terminal app is typically provided on every OS)
- Using a single terminal window with a terminal multiplexer (you'll need to install `zellij` or `tmux` with your system package manager)

If you have never used Lean before, install `elan`, the Lean compiler toolchain manager. Run at least a `lake build` or `lake run`in your Lean test project to make sure your Lean code has its dependencies fetched (otherwise the LSP will not work)

Install Rust (sorry, but currently my main language):

```bash
cargo install --git https://codeberg.org/wvhulle/lean-tui
```

(Might take a long time first time because the Tree-Sitter parser is compiled)

Make sure `~/.cargo/bin` is in your path.

## Configuration

Use your favorite (modal) editor that has a built-in LSP client (I recommend Helix, but Neovim, Zed, Kate also seem to have one).

Go into the settings of your editor and configure the LSP command for lean to be `cargo run -- serve`. This will spawn a proxy LSP that intercepts communication with the Lake LSP every time you open a Lean file. Make sure to disable any other Lean LSP as this one will replace it and extend it.

## Usage

Open your Lean file in your chosen editor.

Split terminal. Or tile another terminal window next to your editor window. Launch the TUI in same directory in the second terminal with `lean-tui tui`.

Switch back to your editor:

1. Move your cursor somewhere in a proof or function
2. Enter "insert mode" in a proof
3. Start typing.

(Note that the goals in the TUI update but only if you actually perform edits in insert mode or use the hover action)

## Debugging

Follow logs with:

```bash
tail -f /tmp/lean-tui.log
```

_I developed this because not everyone wants to be stuck in the Microsoft ecosystem. Many people have an efficient workflow in their own (often modal) text editor. There existed a [Lean plugin for Neovim](https://github.com/Julian/lean.nvim) already, but not yet for the other ones. This is my attempt at a more generic one, not bound to any editor in particular, and usable from any terminal window._
