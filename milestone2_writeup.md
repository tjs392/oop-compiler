## Milestone 2

### Part 1: Optimized SSA Transformation

I implemented SSA transformation using dominator trees and dominance frontiers from the start, so I do not have a naive implementation to compare against. However, the results demonstrate that the phi placement is optimal.

`test_programs/ssa_test.441`: Assigns 6 variables (`a, b, c, d, e, x`), then only reassigns `x` inside an if/else branch.

see `results/ssa_no_ssa.ir` and `results/ssa_opt.ir` for the single phi node insertion.

A naive SSA transformation would insert 6 phi nodes at the merge point. My optimized approach inserts only 1.

### Part 2: Value Numbering

I implemented local (single basic block) value numbering. Within each basic block, the pass assigns value numbers to expressions and reuses previously computed results when a redundant expression is detected.

`test_programs/valnum.441`: Computes `c = (a + b)`, `d = (a + b)`, `e = (c + d)`, `f = (c + d)`, then prints `e` and `f`.

see `results/valnum_no_vn.ir` and `results/valnum_vn.ir`:

Without value numbering, the final block computes two separate divisions to untag both results:
```
%40 = %31 / 2
print(%40)
%41 = %39 / 2
print(%41)
```

With value numbering, the pass recognizes that `%31` and `%39` share the same value number, so `%39 / 2` is redundant and gets replaced with a reference to the already-computed result:
```
%40 = %31 / 2
print(%40)
%41 = %40
print(%41)
```

This eliminates one division instruction. Unfortunately right now each binary operation produces multi block type check sequence which limits what local value numbering can optimize.