inductive MyNat where
| zero: MyNat
| succ: (n: MyNat) -> MyNat
open MyNat


example (h: 1 = 2): (succ zero) = zero := by

 sorry
