inductive MyNat where
| zero: MyNat
| succ: MyNat -> MyNat

example: 1 = 1 := by

  rfl
