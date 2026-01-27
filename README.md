# Lean-TUI

(**Warning**: early release, under development)

This is a **terminal-only (TUI) info view**, comparable to the VS Code info view for [Lean 4](https://lean-lang.org/).

It shows:

- The active variable bindings for a developer writing code (term mode).
- The proof structure, hypotheses and goals for a mathematician proving and formalizing proofs (tactic mode).

See below (or go to [codeberg](https://codeberg.org/wvhulle/lean-tui)) for a screenshot of this program rendering the proof state of a simple but incomplete Lean proof:

```lean
import Mathlib.Data.Set.Basic
import Paperproof

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

| Tree-Sitter                  | PaperProof                            |
| ---------------------------- | ------------------------------------- |
| ![](./imgs/before_after.png) | ![](./imgs/deduction_style.png)       |
| ![](./imgs/goal_tree.png)    | ![](./imgs/step_hypotheses_goals.png) |

Display styles:

- Goal tree: open goals as a tree
- Before after: current active goal state and previous and next goal state
- Deduction tree: proof shown as a deduction tree (with Paperproof)
- Steps: steps shown next to all active hypotheses and goals

Switch proof display styles: `[`, `]`

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

Paperproof is optional and runs inside Lake LSP. It has access to detailed information about the Lean program itself.

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

### 3. Import PaperProof (optional)

If you want to use the more detailed display mode, add [PaperProof](https://github.com/Paper-Proof/paperproof/tree/main) as a Lake dependency.

In your `lakefile.toml`:

```toml
[[require]]
name = "Paperproof"
git = "https://github.com/Paper-Proof/paperproof.git"
subDir = "lean"
rev = "main"
```

Fetch the source code for dependencies: `lake update Paperproof`.

Add this line to the Lean source code of which you want to visualize proof state:

```lean
import Paperproof
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

You can choose between:

- Using multiple terminals (emulator windows) side-by-side (a terminal app is typically provided on every OS)
- Using a single terminal window with a terminal multiplexer (you'll need to install `zellij` or `tmux` with your system package manager)

Open your Lean file in your favorite (modal) editor that has a built-in LSP client (I recommend Helix, but Neovim, Zed, Kate also seem to have one).

Split terminal. Launch the TUI in same directory in the second terminal with `lean-tui view`.

Switch back to your editor:

1. Move your cursor somewhere in a proof or function
2. Enter "insert mode" in a proof
3. Start typing or hover

Switch to the TUI. Key bindings:

- Use `j`, `k` to go up or down in hypotheses
- Use `d` to jump to term definition
- Use `t`to jump to type definition
- Help menu `?`
- Close with `q`

Normally, you **should not need to switch** often from now on, as the Lean-TUI window will follow your edits/hovers in the editor by default.

*Under development: filtering certain types of hypotheses with keyboard shortcuts (`i` for class instance terms and so on).*

## Debugging

Follow `lean-tui` logs with:

```bash
tail -f ~/.cache/lean-tui/proxy.log
```

Some editors also have debug logs for the LSP client. For Helix:

```bash
tail -f ~/.cache/helix/helix.log
```

## Why?

I developed this because not everyone wants to be stuck in the Microsoft ecosystem. Many people have an efficient workflow in their own (often modal) text editor. There existed a [Lean plugin for Neovim](https://github.com/Julian/lean.nvim) already, but not yet for the other ones. This is my attempt at a more generic one, not bound to any editor in particular, and usable from any terminal window.

Let me know if you tried it out and encountered any issues! PRs are also welcome.
