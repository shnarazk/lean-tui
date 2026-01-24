# Lean TUI - Standalone Infoview for Lean 4

## Phase 0: Minimal Working Example (Cursor Tracking)

**Goal**: Prove the concept works with Helix before building the full infoview.

### Architecture

```
Terminal 1 (Helix)              Terminal 2 (TUI Watcher)
┌─────────────────┐             ┌─────────────────┐
│  helix test.lean│             │  lean-tui watch │
│                 │             │                 │
│  (LSP client)   │             │  Line: 5        │
└────────┬────────┘             │  Char: 12       │
         │ stdio                │  File: test.lean│
         ▼                      │  Method: hover  │
┌─────────────────┐             └────────┬────────┘
│  lean-tui serve │                      │
│  (LSP server)   │──────────────────────┘
└─────────────────┘   writes to /tmp/lean-tui.json
```

### Current Structure

```
src/
├── main.rs           # CLI dispatch (serve/tui), tracing setup
├── error.rs          # Vanilla error enum
├── lake_lsp_proxy/   # LSP proxy: Helix ↔ lake serve
│   └── mod.rs
├── lake_ipc/         # Lean RPC protocol types + client
│   ├── mod.rs        # Goal, Hypothesis, RPC constants
│   └── rpc_client.rs # Session management, getInteractiveGoals
├── tui_ipc/          # Proxy ↔ TUI communication
│   ├── mod.rs
│   ├── message.rs    # Message, CursorInfo, Position
│   └── broadcaster.rs# Unix socket broadcaster
└── tui/
    └── mod.rs        # ratatui TUI client
```

### Usage

```bash
# Terminal 1: Start TUI watcher
cargo run -- watch

# Terminal 2: Open Helix (from this directory for .helix/languages.toml)
hx test.lean
# Press 'K' to trigger hover → cursor position appears in Terminal 1
```

---

## Phase 1: Full LSP Proxy

**Goal**: Act as transparent proxy between Helix and `lake serve`, intercepting cursor position.

```
┌─────────────────┐          ┌─────────────────┐          ┌─────────────────┐
│  Helix/Editor   │◀────────▶│   lean-tui      │◀────────▶│  lake serve     │
│  (LSP client)   │  stdio   │  (LSP proxy)    │  stdio   │  (LSP server)   │
└─────────────────┘          └─────────────────┘          └─────────────────┘
                                     │
                                     ▼
                             ┌─────────────────┐
                             │  TUI Display    │
                             │  (goals, hyps)  │
                             └─────────────────┘
```

**Intercepted messages:**
- `textDocument/didOpen` → Track open documents, establish RPC sessions
- `textDocument/didChange` → Re-fetch goals after edits
- `textDocument/hover` → Track cursor position

**Forwarded transparently:** All other LSP requests/responses

---

## Phase 2: Lean RPC Protocol

**Session lifecycle:**
1. `$/lean/rpc/connect` → returns `sessionId`
2. `$/lean/rpc/keepAlive` every 20s
3. `$/lean/rpc/call` for method invocations

**Get interactive goals:**

> **Important**: The `textDocument` and `position` fields appear TWICE - at the top level
> AND inside the inner `params`. This matches lean.nvim's implementation where
> `vim.tbl_extend('error', pos, {...})` merges position into the outer params,
> while also passing it as the RPC method's params.
> See: https://github.com/Julian/lean.nvim/blob/main/lua/lean/rpc.lua#L183-L186

```json
{
  "method": "$/lean/rpc/call",
  "params": {
    "textDocument": {"uri": "file://..."},
    "position": {"line": 5, "character": 10},
    "sessionId": "...",
    "method": "Lean.Widget.getInteractiveGoals",
    "params": {"textDocument": {"uri": "..."}, "position": {"line": 5, "character": 10}}
  }
}
```

---

## Phase 3: TUI Display

**Layout:**
```
┌─────────────────────────────────────┐
│ Goals (1/3)                    [q] │
├─────────────────────────────────────┤
│ x : Nat                             │
│ y : Nat                             │
│ h : x > 0                           │
│ ⊢ x + y = y + x                     │
├─────────────────────────────────────┤
│ [j/k] navigate  [f] filter  [d] diff│
└─────────────────────────────────────┘
```

---

## Tech Stack

| Component | Choice |
|-----------|--------|
| Runtime | tokio |
| LSP | async-lsp 0.2 |
| TUI | ratatui + crossterm |
| Error handling | Vanilla enums (no anyhow/thiserror) |
| Serialization | serde + serde_json |

---

## Implementation Checklist

### Phase 0 (POC) ✅
- [x] CLI with `serve` and `tui` subcommands
- [x] Helix configuration in `.helix/languages.toml`
- [x] Unix socket IPC (replaced JSON file approach)

### Phase 1 (Proxy) ✅
- [x] Spawn `lake serve` as child process
- [x] Forward LSP messages bidirectionally (async-lsp)
- [x] Intercept position-containing requests (hover, definition, completion, etc.)
- [x] Intercept `textDocument/didChange` for insert mode cursor tracking
- [x] Unix socket broadcaster for TUI clients
- [x] File-based logging to `/tmp/lean-tui.log`

### Phase 2 (RPC) 🚧
- [x] Implement `$/lean/rpc/connect` → session management
- [ ] Implement `$/lean/rpc/keepAlive` timer (TODO in code)
- [x] Implement `$/lean/rpc/call` for `getInteractiveGoals`
- [x] Parse `InteractiveGoals` response → `Goal` structs
- [ ] Verify goals display in TUI

### Phase 3 (TUI)
- [x] Basic cursor info display
- [x] Goals placeholder display
- [ ] Render goals with hypotheses
- [ ] Keyboard navigation (j/k)
- [ ] Hypothesis filtering
- [ ] Goal diffing

---

## References

- [async-lsp examples](https://github.com/oxalica/async-lsp/tree/main/examples)
- [lean.nvim RPC](https://github.com/Julian/lean.nvim/blob/main/lua/lean/rpc.lua)
- [Helix languages.toml](https://docs.helix-editor.com/languages.html)
