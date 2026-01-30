import Lake
open Lake DSL

package «lean-tui-test» where
  version := v!"0.1.0"

-- During development: use local path
require LeanDag from ".." / "lean-dag"

-- For release: switch back to git
-- require LeanDag from git
--   "https://github.com/user/lean-dag.git" @ "main"

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"

@[default_target]
lean_lib «Test» where
  roots := #[`test]
