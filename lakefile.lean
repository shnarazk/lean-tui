import Lake
open Lake DSL

package «lean-tui-test» where
  version := v!"0.1.0"

require LeanDag from git
  "https://github.com/wvhulle/lean-dag.git" @ "main"

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"

@[default_target]
lean_lib «Test» where
  roots := #[`test]
