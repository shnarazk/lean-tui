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

### Files to Create

```
src/
├── main.rs       # CLI dispatch (serve/watch)
├── error.rs      # Vanilla error enum
├── serve.rs      # LSP server mode
├── watch.rs      # TUI watcher mode
└── shared.rs     # CursorInfo struct, file path
.helix/
└── languages.toml  # Register lean-tui for .lean files
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
```json
{
  "method": "$/lean/rpc/call",
  "params": {
    "textDocument": {"uri": "file://..."},
    "position": {"line": 5, "character": 10},
    "sessionId": 42,
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

### Phase 0 (POC)
- [ ] CLI with `serve` and `watch` subcommands
- [ ] LSP server that handles `initialize` and `hover`
- [ ] Write cursor position to `/tmp/lean-tui.json`
- [ ] TUI watcher that displays cursor position
- [ ] Helix configuration in `.helix/languages.toml`

### Phase 1 (Proxy)
- [ ] Spawn `lake serve` as child process
- [ ] Forward LSP messages bidirectionally
- [ ] Intercept `textDocument/hover` for cursor tracking

### Phase 2 (RPC)
- [ ] Implement `$/lean/rpc/connect`
- [ ] Implement `$/lean/rpc/keepAlive` timer
- [ ] Implement `$/lean/rpc/call` for goals

### Phase 3 (TUI)
- [ ] Render goals with hypotheses
- [ ] Keyboard navigation (j/k)
- [ ] Hypothesis filtering
- [ ] Goal diffing

---

## References

- [async-lsp examples](https://github.com/oxalica/async-lsp/tree/main/examples)
- [lean.nvim RPC](https://github.com/Julian/lean.nvim/blob/main/lua/lean/rpc.lua)
- [Helix languages.toml](https://docs.helix-editor.com/languages.html)
