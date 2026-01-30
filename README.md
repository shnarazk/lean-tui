# Lean-TUI

(**Warning**: early release, under development)

This is a **terminal-only (TUI) info view**, comparable to the VS Code info view for [Lean 4](https://lean-lang.org/).

It shows:

- The proof structure, hypotheses and goals for a mathematician proving and formalizing proofs (tactic mode in Lean).
- The active variable bindings for a developer writing code (called "term-mode" in Lean).

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

| Tree-Sitter                  | Paper-proof                      |
| ---------------------------- | -------------------------------- |
| ![](./imgs/flat_list.png)    | ![](./imgs/tactic_tree.png)      |
| ![](./imgs/before_after.png) | ![](./imgs/semantic_tableau.png) |

There are different display modes. The modes that work best with 'just Tree-Sitter' (syntactic):

- Plain list: simplest display mode with just a list of open goals
- Before after: current active goal state and previous and next goal state

There are two additional modes that also work with Tree-Sitter, but it is best to add LeanDag to your Lean file to get more detailed Lean-specific information:

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

### 3. Import LeanDag (optional)

If you want to use the more detailed display mode, add [LeanDag](https://github.com/wvhulle/lean-dag) as a Lake dependency.

In your `lakefile.toml`:

```toml
[[require]]
name = "LeanDag"
git = "https://github.com/wvhulle/lean-dag.git"
rev = "main"
```

Fetch the source code for dependencies: `lake update LeanDag`.

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

- Use the arrows or click on hypotheses and goals
- Use `d` to jump to term definition
- Use `t`to jump to type definition

Proof states that are incomplete will be yellow and the ones you are currently working on blue.
Normally, you **should not need to switch** often from now on, as the Lean-TUI window will follow your edits/hovers in the editor by default.

_Under development: filtering certain types of hypotheses with keyboard shortcuts (`i` for class instance terms and so on)._

More shortcuts:

- Help menu `?`
- Close with `q`

## Debugging

Follow `lean-tui` logs with:

```bash
tail -f ~/.cache/lean-tui/proxy.log # Proxy server only
tail -f ~/.cache/lean-tui/tui.log # TUI front-end only
```

Some editors also have debug logs for the LSP client. For Helix:

```bash
tail -f ~/.cache/helix/helix.log
```

## Why?

I developed this because not everyone wants to be stuck in the Microsoft ecosystem. Many people have an efficient workflow in their own (often modal) text editor. There existed a [Lean plugin for Neovim](https://github.com/Julian/lean.nvim) already, but not yet for the other ones. This is my attempt at a more generic one, not bound to any editor in particular, and usable from any terminal window.

Let me know if you tried it out and encountered any issues! PRs are also welcome.

## How does it work?

This program will spawn a proxy LSP that intercepts communication with the Lake LSP every time you open a Lean file.

```mermaid
flowchart LR
    subgraph Terminal 1
        subgraph Editor
            LSPClient[LSP Client]
        end
    end

    subgraph Terminal 2
        TUI[lean-tui view]
    end

    subgraph Proxy Process
        Proxy[lean-tui proxy]
    end

    LSPClient <--> |stdio| Proxy
    Proxy <--> |LSP + Lean RPC| Lake[Lake LSP]
    Proxy --> |Unix socket| TUI
```

Using LeanDag as a datasource is optional. When it is added, it runs inside the default LSP server provided by Lake (the standard build tool for Lean). In this way, it has access to more detailed information about the Lean program itself. This is useful because Lean has very expressive "elaboration" mechanism to extend its own syntax and convert it into math.
