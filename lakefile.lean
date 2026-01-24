import Lake
open Lake DSL

package «lean-tui-test» where
  version := v!"0.1.0"

@[default_target]
lean_lib «Test» where
  roots := #[`test]
