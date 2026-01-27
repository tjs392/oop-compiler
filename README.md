# Compiler
**Teiji Schoyen**

## Building
```bash
cargo build --release
# ./comp also builds before running
```

## Running
```bash
# build with constnt folding pepphole optimization
./comp <source.441> > output.ir

# build without constant folding opt
./comp -noopt <source.441> > output.ir
```

### Compiling and running source code using ir441 parser.
```bash
# compile into ir
./comp test_programs/constant_folding.441 > constant_folding.ir

# parse ir
./bin/ir441 exec constant_folding.ir
```

## Peephole Optimization

**Optimization Implemented:** Constant Folding

**Description:** 
I implemented constant folding. This is an optimization that I do on the second pass after converting my CFG to valid SSA. I do another pass where I look for binary operations and evaluate them at compile time to fold them down to a single constant.

**Example:**
```
Unoptimized:
  %0 = 2 * 3
  %1 = 4 * 5  
  %2 = %0 + %1

Optimized:
  %0 = 6
  %1 = 20
  %2 = 26
```

**Code Location:** `src/cfg.rs`
- `fold_constants()`
- `try_fold_constant()`
- `evaluate_binop()`

## Test Programs

Test programs are located in `test_programs/`:
- `constant_folding.441` - tests constant folding
- `factorial.441` - tests recursion
- `polymorphism.441` - tests OOP and polymorphism
- `loop.441` - tests simple while loop

## Known Issues
None