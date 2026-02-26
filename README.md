# Compiler
**Teiji Schoyen**

## Requirements
- Rust

## Building
```bash
cargo build --release
```

## Running
```bash
./comp <source.441> > output.ir

./comp --no-ssa <source.441> > output.ir
./comp --no-vn <source.441> > output.ir
./comp --no-fold <source.441> > output.ir
```

### Compiling and running and getting perf traces
```bash
./comp test_programs/stack_typed.441 > stack_typed.ir
./bin/ir441 exec stack_typed.ir
./bin/ir441 perf stack_typed.ir

# to compile using the old untyped compiler:
./comp_untyped

# To get raw PERF stats 
./type_comparison_perf

# ^ this just gives the comparisons
# ir441 and untyped compiler have binaries in ./bin to use
```

## Type Checking

I added type annotations to the source language. Every variable, field, and method argument now includes a type (`int` or a class name). Methods have a `returning` clause for the return type, and null literals look like `null:ClassName`.

The type checker is in `src/typechecker.rs` and runs before IR generation. It catches:
- References to undeclared class types
- Binary ops on non-integer operands
- Field reads/writes on wrong class types or nonexistent fields
- Method calls with wrong argument count/types
- Printing non-integers
- If/while conditions that aren't integers
- Assignments where expression type doesn't match the variable type

## Type-Based Optimizations

Since the type checker guarantees correctness before we get to the backend, I was able to cut out a bunch of runtime checks in the IR builder (`src/ir_builder.rs`):

**1. No more integer tagging.** Constants are just raw values now.

**2. No more field map indirection.** Object layout changed from `[vtable_ptr, field_map_ptr, fields... etc]` to `[vtable_ptr, fields...]`. We know the type at compile time so we can just compute the slot offset directly.
```
BEFORE: load field_map -> getelt(field_map, id) -> check offset -> getelt(obj, offset)
AFTER:  null check -> getelt(obj, known_slot)
```

**3. No more method existence check.** Type checker already verified every method call, so the NoSuchMethod fail branch is gone. Just null check the receiver, then vtable lookup and call.
```
BEFORE: tag check -> load vtable -> getelt method -> check method != 0 -> call
AFTER:  null check -> load vtable -> getelt method -> call
```

**4. CFG/SSA variables tagged with types.** Every temp variable gets a type entry in a `var_types` map. During SSA renaming the types get copied to the new SSA names.

## Performance Results

Tested with a linked-list stack program that pushes values 1–20, then pops and prints them all

| Metric | Untyped (M2) | Typed (M3) | Reduction |
|--------|-------------|-----------|-----------|
| Fast ALU ops | 1394 | 538 | 61% |
| Slow ALU ops | 653 | 264 | 60% |
| Conditional branches | 673 | 327 | 51% |
| Unconditional branches | 347 | 306 | 12% |
| Mem reads | 629 | 265 | 58% |
| Mem writes | 125 | 103 | 18% |
| Calls | 82 | 82 | same |
| Allocs | 22 | 22 | same |

Biggest wins are ALU ops (no tag math), branches (no tag/field/method checks), and memory reads (no field map loads). Calls and allocs stay the same

## Test Programs

Test programs are in `test_programs/`:
- `stack_typed.441` - linked-list stack (typed, for milestone 3)
- `stack_untyped.441` - same program (untyped, for milestone 2 comparison)
- `typed_test.441` - basic type checker test
- `constant_folding.441` - tests constant folding
- `factorial.441` - tests recursion
- `polymorphism.441` - tests OOP and polymorphism
- `loop.441` - tests simple while loop
