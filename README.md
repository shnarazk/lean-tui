# Lean-TUI

Standalone TUI infoview for Lean 4 theorem prover.

Shows proof structure, hypotheses and goals - comparable to the VS Code infoview but for any terminal.

See below (or go to [codeberg](https://codeberg.org/wvhulle/lean-tui)) for a screenshot of proof state of a simple but incomplete Lean proof:

```lean
import Mathlib.Data.Set.Basic

theorem commutativityOfIntersections
    (s t : Set Nat) : s ∩ t = t ∩ s := by
  ext x
  apply Iff.intro

  intro h1
  rw [Set.mem_inter_iff, and_comm] at h1
  exact h1

  intro h2
  rw [Set.mem_inter_iff, and_comm] at h2

  -- exact h2
```

Screenshots of different display modes

| Linear                       | Graph                            |
| ---------------------------- | -------------------------------- |
| ![](./imgs/flat_list.png)    | ![](./imgs/tactic_tree.png)      |
| ![](./imgs/before_after.png) | ![](./imgs/semantic_tableau.png) |

There are different display modes. Linear modes do not show much proof structure:

- Plain list: simplest display mode with just a list of open goals
- Before after: current active goal state and previous and next goal state

There are two additional modes that show more graph structure:

- Tactic tree: tree of the tactic structure next to active hypotheses and goals
- Semantic tableau: proof shown as a semantic tableau

Switch proof display styles: `[`, `]`

## Installation

### 1. Compiler toolchains

If you have never used Lean before, install `elan`, the Lean compiler toolchain manager. Run at least a `lake build` or `lake run`in your Lean test project to make sure your Lean code has its dependencies fetched (otherwise the LSP will not work)

Install Rust (through [`rustup`](https://rustup.rs/)) if you haven't compiled Rust programs before.

### 2. Install this TUI

Then install this crate as a binary in your user with:

```bash
cargo install lean-tui
```

If `~/.cargo/bin` is in your path, you can now run this program with `lean-tui`.

### 3. Add `LeanDag` dependency

Add [LeanDag](https://github.com/wvhulle/lean-dag) as a Lake dependency.

In your `lakefile.toml`:

```toml
[[require]]
name = "LeanDag"
git = "https://github.com/wvhulle/lean-dag.git"
rev = "main"
```

### 4. Build `lean-dag`

Fetch the source code of the latest version (Lean only hosts on Git, does not provide prebuilt dependencies):

```bash
lake update LeanDag
```

Now you have two options: modify your source code and add:

```lean
import LeanDag
```

If you don't want to modify, you can build the `lean-dag` binary (it will stay deep in the build artifacts directory):

```bash
lake build LeanDag/lean-dag
```

## Configuration

Go into the settings of your editor and configure the LSP command for lean to be `lean-tui proxy`.

For example, for Helix, it would look like this:

```toml
# .helix/languages.toml
[language-server.lean-tui]
args = ["proxy"]
command = "lean-tui"

[[language]]
language-servers = ["lean-tui"]
name = "lean"
```

Make sure to disable any other Lean LSP as this one will replace it and extend it.

## Usage

### 1. Split your terminal

You can choose between:

- Using multiple terminals (emulator windows) side-by-side (a terminal app is typically provided on every OS)
- Using a single terminal window with a terminal multiplexer (you'll need to install `zellij` or `tmux` with your system package manager)

Open your Lean file in your favorite (modal) editor that has a built-in LSP client (I recommend Helix, but Neovim, Zed, Kate also seem to have one).

Split terminal. Launch the TUI in same directory in the second terminal with `lean-tui view`.

### 2. Start writing proofs

Switch back to your editor:

1. Move your cursor somewhere in a proof or function
2. Enter "insert mode" in a proof
3. Start typing or hover

### 3. Play with Lean-TUI

Switch to the TUI. Key bindings:

| Key | Action |
|-----|--------|
| `↑/↓` | Navigate hypotheses and goals |
| `g` | Go to where item was introduced |
| `y` | Copy to clipboard (OSC 52) |
| `[/]` | Switch display mode |
| `?` | Help menu |
| `q` | Quit |

The TUI follows your cursor in the editor automatically.

## Why?

I developed this because not everyone wants to be stuck in the Microsoft ecosystem. Many people have an efficient workflow in their own (often modal) text editor. There existed a [Lean plugin for Neovim](https://github.com/Julian/lean.nvim) already, but not yet for the other ones. This is my attempt at a more generic one, not bound to any editor in particular, and usable from any terminal window.

Let me know if you tried it out and encountered any issues! PRs are also welcome.

## How does it work?

This program spawns a proxy LSP that intercepts communication between your editor and Lean's build system.
**lean-dag** is a custom LSP server that runs alongside Lake. It uses Lean's internal APIs to extract detailed proof information (tactic applications, goal transformations, goto locations) that isn't available through standard LSP.

## Debugging

If you see errors in the editor like "incompatible headers", you can try

1. Close both the TUI view and the LSP client and restart.
2. Rebuilding `lean-dag`

If that does not help and you have time, follow along with the logs while reproducing the bug (and paste the output in a bug report):

```bash
tail -f ~/.cache/lean-tui/proxy.log # Proxy server only (for debugging RPC deserialization)
tail -f ~/.cache/lean-tui/tui.log # TUI front-end only (for debugging the Ratatui-side)
tail -f ~/.cache/lean-tui/lean-dag.log # Lean RPC server (for debugging Lean-side)
```

Some editors also have debug logs for the LSP client. For Helix:

```bash
tail -f ~/.cache/helix/helix.log
```
