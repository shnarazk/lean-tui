import Lake
open Lake DSL

package «lean-tui-test» where
  version := v!"0.1.0"

-- During development: use local path
require Paperproof from ".." / "paperproof" / "lean"

-- For release: switch back to git
-- require Paperproof from git
--   "https://github.com/Paper-Proof/paperproof.git" @ "main" / "lean"

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"

@[default_target]
lean_lib «Test» where
  roots := #[`test]
