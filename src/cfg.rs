use crate::ir::{ControlTransfer, Function, Primitive, Value};
use std::collections::HashMap;
use std::collections::HashSet;

pub struct CFG {

    // bb label -> index in function.blocks
    block_map: HashMap<String, usize>,

    // 2d array
    // index of successor bb -> indices of predecessor bbs
    predecessors: Vec<Vec<usize>>,

    // index of predecessor bb -> indices of successor bbs
    successors: Vec<Vec<usize>>,

    // entry bb indx
    entry: usize,

    dominators: Vec<HashSet<usize>>,

    num_blocks: usize,
}

// cfgs will be made per function
impl CFG {
    pub fn new(function: &Function) -> Self {
        let num_blocks = function.blocks.len();
        let mut block_map = HashMap::new();
        let mut predecessors: Vec<Vec<usize>> = vec![vec![]; function.blocks.len()];
        let mut successors: Vec<Vec<usize>> = vec![vec![]; function.blocks.len()];

        // here we will just build the block map so we have o(1) access to the labels/index
        for (idx, block) in function.blocks.iter().enumerate() {
            block_map.insert(block.label.clone(), idx);
        }

        // this second pass will
        // 1. compute predecessors and successors for dominance frontiers
        for (idx, block) in function.blocks.iter().enumerate() {
            // build out predecessors and successors
            match &block.control_transfer {

                ControlTransfer::Branch { cond: _, then_lab, else_lab } => {
                    let then_idx = block_map.get(then_lab).unwrap();
                    let else_idx = block_map.get(else_lab).unwrap();
                    
                    successors[idx].push(*then_idx);
                    successors[idx].push(*else_idx);
                    
                    predecessors[*then_idx].push(idx);
                    predecessors[*else_idx].push(idx);
                }

                ControlTransfer::Jump { target } => {
                    let target_idx = block_map.get(target).unwrap();

                    successors[idx].push(*target_idx);
                    predecessors[*target_idx].push(idx);
                }

                ControlTransfer::Fail { .. } => {}

                ControlTransfer::Return { .. } => {}
            }
        }

        CFG { 
            block_map,
            predecessors,
            successors,
            entry: 0,
            dominators: vec![],
            num_blocks,
        }
    }

    pub fn convert_to_ssa(&mut self, function: &mut Function, var_types: &mut HashMap<String, crate::ast::Type>) {
        self.compute_dominator_sets();
        self.insert_phi_functions(function);

        let tree = self.build_dominator_tree();
        let mut stacks: HashMap<String, Vec<String>> = HashMap::new();
        let mut counter: usize = 0;

        self.rename(function, self.entry, &mut stacks, &mut counter, &tree, var_types);
    }


    // dom(block) = { block } U { & dom(pred) } for all pred in pred(block) }
    // algorithm is:
    // initialize the entry block's dominatr set to just itself
    // initialize every other block's dominator set to all blocks
    // iteration:
    //              for each block, intersect all predecessor dominator sets then add block itself
    //              repeat until we get fixed point convergence
    fn compute_dominator_sets(&mut self) {
        // initlize the empty domoinator set for each block
        let mut dominator_sets: Vec<HashSet<usize>> = vec![HashSet::new(); self.num_blocks];

        // entry block only dominates itself
        dominator_sets[0].insert(0);

        // every non entry block's dominator set is just all the blocks, this is just the initial assumption
        // the iteration will shrink the sets down with the algo above
        // this prevents any dominator information loss
        let all_blocks: HashSet<usize> = (0..self.num_blocks).collect();
        for idx in 1..self.num_blocks {
            dominator_sets[idx] = all_blocks.clone();
        }

        // fixed point iter: just keep going until we dont see a change
        // intersection only removes elements it does not grow a set, so we are 
        // guaranteed to shrink and converge :D
        let mut changed = true;
        while changed {
            changed = false;

            // always gotta start at idx 1 to skip the entry block
            for idx in 1..self.num_blocks {
                let preds = &self.predecessors[idx];

                // if a block's preds is empty, then it is unreachable dead code. 
                // idk how this could appear anyway but just check
                if preds.is_empty() {
                    continue;
                }

                // new dom is the dominator of the first predecessor
                let mut new_dom = dominator_sets[preds[0]].clone();

                // interesect this with each of the other predecessor's dominator sets
                // this is very important cause a block only dominates another block if
                //      that said block dominates ALL predecessors of the given block
                // basically, every path to the block goes through the predecessor, then it DOMINATES
                for &pred in &preds[1..] {
                    new_dom = new_dom.intersection(&dominator_sets[pred]).copied().collect();
                }

                // insert itself cause it dominates itself o.o
                new_dom.insert(idx);
                
                // this is the fixed point iteration check, if it changed keep going
                // this check on convergence
                if new_dom != dominator_sets[idx] {
                    dominator_sets[idx] = new_dom;
                    changed = true;
                }
            }
        }

        self.dominators = dominator_sets;
    }

    fn compute_immediate_dominators(&self) -> Vec<Option<usize>> {
        let mut immediate_dominators: Vec<Option<usize>> = vec![];
        // entey block has no dominators
        immediate_dominators.push(None);

        for idx in 1..self.num_blocks {
            // get dom set for current block
            let dom_set = &self.dominators[idx];

            // compute strict dominators (just all its dominators - the curr block)
            let mut strict_dominators = dom_set.clone();
            strict_dominators.remove(&idx);

            // find the largest dominator set
            // we want the largest dominator set because this represents the "deepest" node in the tree
            // so it is mathematically the most immediate dominator
            let immediate_dominator = strict_dominators.iter()
                .max_by_key(|&&dominator| self.dominators[dominator].len())
                .copied();
            
            immediate_dominators.push(immediate_dominator);
        }

        immediate_dominators
    }

    /*
    from https://en.wikipedia.org/wiki/Static_single-assignment_form:
    for each node b
        dominance_frontier(b) := {}
    for each node b
        if the number of immediate predecessors of b ≥ 2
            for each p in immediate predecessors of b
                runner := p
                while runner ≠ idom(b)
                    dominance_frontier(runner) := dominance_frontier(runner) ∪ { b }
                    runner := idom(runner)
    */
    fn compute_dominance_frontiers(&self) -> Vec<HashSet<usize>> {
        // first immediate dominators
        // this is the closest strict dominator
        // the last block that must be visited on every entry into the block
        let immediate_dominators = self.compute_immediate_dominators();
        let mut dominance_fronters: Vec<HashSet<usize>> = vec![HashSet::new(); self.num_blocks];

        for idx in 0..self.num_blocks {

            // i do this check because we only are about the join points, so if a block only has like 
            // 1 predecessor, why do we even care because the phi would be useless
            if self.predecessors[idx].len() >= 2 {
                for &pred in &self.predecessors[idx] {

                    // walk up the dominator tree from the pred away from the join point
                    // every block does not strictly dominate, but does have a path to the block
                    // thats a dominance frontier
                    let mut runner = pred;
                    while Some(runner) != immediate_dominators[idx] {
                        dominance_fronters[runner].insert(idx);

                        // this only stops on the entry block
                        if let Some(immediate_dominator) = immediate_dominators[runner] {
                            runner = immediate_dominator;
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        dominance_fronters
    }

    /*
        -= inserting phi functions =-

        1. first, we need to scan each block to find which variables are defined in the block
            - build map: variable -> set of blocks that define the variable

        2. then, for each variable with definitions, we can use the computed dominance frontier to find where different defs will merge
            to find where they merge:
            - if block 1 defines the variable, and block 2 is in its dominance frontier,
                then block 2 can be reached by paths that dont go through block 1
            - this means that block 2 can get that variable from somewhere else, so we need to add a phi function
            
        3. then, we can use a work queue for adding the phi function, just add to work queue and process it l8r
            - add x = phi(stuf) in block 2, then block 2 defines x
            - so check 2's dominance frontier
            - keep going until queue is empty

        4. just need one more pass to rename the variables given their new phi placement
    */

    // need to find assignments inside of blocks
    fn collect_assignments(&self, function: &Function) -> HashMap<String, HashSet<usize>> {
        let mut assignments: HashMap<String, HashSet<usize>> = HashMap::new();

        for (idx, block) in function.blocks.iter().enumerate() {
            for primitive in &block.primitives {
                if let Primitive::Assign { dest, .. } = primitive {
                    assignments.entry(dest.clone())
                        .or_insert_with(HashSet::new)
                        .insert(idx);
                }
            }
        }
        assignments
    }

    // algorithm:
    //      first compute the dominance frontiers of ebvery block
    //      the dominance frontier is just the set of block ]s where the block's dominance ends
    //      ie blocks that the block does not strictly dominate
    //      but have a predecessor dominated by the block, these are the join paths
    fn insert_phi_functions(&mut self, function: &mut Function) {

        let dominance_frontiers = self.compute_dominance_frontiers();

        // for example: x is assigned in blocks {0, 2, 3}
        let assignments = self.collect_assignments(function);

        // block idx -> set of variables that require a phi func
        let mut phis: HashMap<usize, HashSet<String>> = HashMap::new();

        for (var, assigning_blocks) in &assignments {
            // note: we need a work stack because technically a phi function is an assignment in itself
            //       so after inserting a phi function, we need to put that new assignment on top of the stack
            let mut work_stack: Vec<usize> = assigning_blocks.iter().copied().collect();
            let mut has_phi_func: HashSet<usize> = HashSet::new();

            while let Some(idx) = work_stack.pop() {
                // need to check every block in the dominance frontier of the block at idx
                for &frontier in &dominance_frontiers[idx] {

                    // if there is no phi function here yet, we need to place one
                    if !has_phi_func.contains(&frontier) {
                        has_phi_func.insert(frontier);

                        // here record and tell the work queue that 
                        // this fronter block needs a phi function for 
                        // variable "var"
                        phis
                            .entry(frontier)
                            .or_insert_with(HashSet::new)
                            .insert(var.clone());
                        work_stack.push(frontier);
                    }
                }
            }
        }

        // now we'll insert the phi functions where we need to
        for (idx, vars) in phis {
            let predecessors = &self.predecessors[idx];
            
            let pred_labels: Vec<String> = predecessors.iter()
                .map(|&pred_idx| function.blocks[pred_idx].label.clone())
                .collect();

            let block = &mut function.blocks[idx];

            for var in vars {
                let args: Vec<(String, Value)> = pred_labels.iter()
                    .map(|label| (label.clone(), Value::Variable(var.clone())))
                    .collect();
                
                // last, just add the phi to the beginning of the block
                block.primitives.insert(0, Primitive::Phi {
                    dest: var,
                    args,
                });
            }
        }
    }

    /*
    source: https://www.cs.cornell.edu/courses/cs6120/2022sp/lesson/6/
    I will be implementing an algorithm similar to the one described here
    */
    // first i need to invert my dominators so i have parent -> child
    fn build_dominator_tree(&self) -> Vec<Vec<usize>> {
        let immediates = self.compute_immediate_dominators();
        let mut tree: Vec<Vec<usize>> = vec![vec![]; self.num_blocks];

        for (child, parent) in immediates.iter().enumerate() {
            if let Some(p) = parent {
                tree[*p].push(child);
            }
        }
        tree
    }

    // algorithm:
    // backtrack rename variables to ssa form by walking the dom tree
    // stacks: for each original var name, a stack of ssa version names, the top ppof the stack is always the most recent def visible. at the current point i the odminator tree
    fn rename(&mut self, 
                function: &mut Function, 
                idx: usize, 
                stacks: &mut HashMap<String, Vec<String>>,
                counter: &mut usize,
                tree: &Vec<Vec<usize>>,
                var_types: &mut HashMap<String, crate::ast::Type>) {
        
        // this is for backtracking
        // we need to track how mny versions we push onto each variable's stack in the current block
        // so we can pop the right number when backtracking to restore stack
        let mut pushed: HashMap<String, usize> = HashMap::new();

        let block = &mut function.blocks[idx];
        for primitive in &mut block.primitives {

            //parse all the primitives
            // first rename the usages of a variable
            // sof or %a = %x + %y, the usages of the variable are %x and %y, but %a is the assignment
            rename_uses(primitive, stacks);

            // next rename the assignment
            if let Some(assignment) = get_dest(primitive) {
                let old_name = assignment.clone();
                let new_name = counter.to_string();
                *counter += 1;

                if let Some(typ) = var_types.get(&old_name).cloned() {
                    var_types.insert(new_name.clone(), typ);
                }

                *assignment = new_name.clone();
                stacks.entry(old_name.clone()).or_insert_with(Vec::new).push(new_name);
                *pushed.entry(old_name).or_insert(0) += 1;
            }
        }

        rename_control_transfer(&mut block.control_transfer, stacks);

        let this_label = function.blocks[idx].label.clone();
        let successors = self.successors[idx].clone();

        // fill phi arguments
        // each phi has one argument per pred
        // can find the argument slot that corresponds to this block
        // and fill it with the current top of the stack
        // the top po fthe stack is just the most recent variable
        for succ_idx in successors {
            let succ_block = &mut function.blocks[succ_idx];

            for primitive in &mut succ_block.primitives {
                // only care abt phi funcs
                // phis are always at the beginning of a block
                if let Primitive::Phi { args, .. } = primitive {
                    
                    // every phi is (label, value) pair
                    //  ex: x = phi(then5, x, else6, x) where then5 and else6 are predecessor labels
                    for (label, val) in args {
                        if label == &this_label {
                            if let Value::Variable(var_name) = val {

                                // look up original variable name, and replace it with the current ssa version
                                if let Some(stack) = stacks.get(var_name.as_str()) {
                                    if let Some(current) = stack.last() {
                                        *var_name = current.clone();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // do children then pop when done w children
        for &child in &tree[idx] {
            self.rename(function, child, stacks, counter, tree, var_types);
        }

        for (var, count) in pushed {
            let stack = stacks.get_mut(&var).unwrap();
            for _ in 0..count {
                stack.pop();
            }
        }
    }

    // constant folding - pretty self explanatory
    // for now just doing it in a separate pass, after ssa
    pub fn fold_constants(&mut self, function: &mut Function) {
        let mut changed = true;

        while changed {
            changed = false;
            
            let mut const_map: HashMap<String, i64> = HashMap::new();
            
            for block in &mut function.blocks {
                for i in 0..block.primitives.len() {
                    if let Primitive::Assign { dest, value: Value::Constant(c) } = &block.primitives[i] {
                        const_map.insert(dest.clone(), *c);
                    }
                    
                    if let Some(folded) = Self::try_fold_constant(&block.primitives[i], &const_map) {
                        block.primitives[i] = folded;
                        changed = true;
                    }
                }
            }
        }
    }

    fn try_fold_constant(prim: &Primitive, const_map: &HashMap<String, i64>) -> Option<Primitive> {
        match prim {
            Primitive::BinOp { dest, lhs, op, rhs } => {
                let left_val = match lhs {
                    Value::Constant(c) => Some(*c),
                    Value::Variable(v) => const_map.get(v).copied(),
                    _ => None,
                };
                
                let right_val = match rhs {
                    Value::Constant(c) => Some(*c),
                    Value::Variable(v) => const_map.get(v).copied(),
                    _ => None,
                };
                
                if let (Some(left), Some(right)) = (left_val, right_val) {
                    if let Some(result) = Self::evaluate_binop(op, left, right) {
                        return Some(Primitive::Assign {
                            dest: dest.clone(),
                            value: Value::Constant(result),
                        });
                    }
                }
                None
            }
            _ => None
        }
    }

    fn evaluate_binop(op: &str, left: i64, right: i64) -> Option<i64> {
        match op {
            "+" => Some(left.wrapping_add(right)),
            "-" => Some(left.wrapping_sub(right)),
            "*" => Some(left.wrapping_mul(right)),
            "/" => {
                if right == 0 {
                    None
                } else {
                    Some(left / right)
                }
            }
            "|" => Some(left | right),
            "&" => Some(left & right),
            "^" => Some(left ^ right),
            "==" => Some(if left == right { 1 } else { 0 }),
            "<" => Some(if left < right { 1 } else { 0 }),
            ">" => Some(if left > right { 1 } else { 0 }),
            _ => None,
        }
    }

    // value numbering does redundant computation elimination
    // ex:      %a = %x + %y
    //          %b = %x + %y
    //  becomes %b = %a
    pub fn value_numbering(&mut self, function: &mut Function) {

        // just doing vlaue numbering per basic block
        for block in &mut function.blocks {
            // we can keep a map of all equal expressions to value numbers
            let mut expr_to_valnum: HashMap<(String, usize, usize), usize> = HashMap::new();

            // straightforward, so we can get the valnum for vars
            // variable name -> its value number
            let mut var_to_valnum: HashMap<String, usize> = HashMap::new();
            // value number -> variable that computed it
            let mut valnum_to_var: HashMap<usize, String> = HashMap::new();
            
            // hold constants now
            let mut const_to_valnum: HashMap<i64, usize> = HashMap::new();

            // tracker for valnums
            let mut valnum_count: usize = 0;

            for i in 0..block.primitives.len() {
                match &block.primitives[i] {
                    Primitive::BinOp { dest, lhs, op, rhs } => {
                        // get val nums for both operands, if we've seen
                        // say %x before and it has val num 3, then lhs_vn = 3
                        let lhs_vn = Self::get_valnum(lhs, &mut var_to_valnum, &mut const_to_valnum, &mut valnum_to_var, &mut valnum_count);
                        let rhs_vn = Self::get_valnum(rhs, &mut var_to_valnum, &mut const_to_valnum, &mut valnum_to_var, &mut valnum_count);

                        // expression is identified by (operator, lhs_valmium, rhsvalnum)
                        // two expression are equal if they do the same op on 
                        // operands w/ the same valnums even if the variables names are different
                        let expr_key = (op.clone(), lhs_vn, rhs_vn);
                        
                        if let Some(&existing_vn) = expr_to_valnum.get(&expr_key) {
                            // okay so we've seem this expression before, so just look up the variabnle where the
                            // evaluation is stored
                            let var = valnum_to_var.get(&existing_vn).unwrap().clone();
                            let dest = dest.clone();
                            var_to_valnum.insert(dest.clone(), existing_vn);

                            // instead of a binop primitive, just do an assign wit the new valnum variable
                            block.primitives[i] = Primitive::Assign {
                                dest,
                                value: Value::Variable(var),
                            };
                        } else {
                            let vn = valnum_count;
                            valnum_count += 1;
                            expr_to_valnum.insert(expr_key, vn);
                            var_to_valnum.insert(dest.clone(), vn);
                            valnum_to_var.insert(vn, dest.clone());
                        }
                    }

                    Primitive::Assign { dest, value } => {
                        // ex: %a = %b or %a = 5
                        let vn = Self::get_valnum(value, &mut var_to_valnum, &mut const_to_valnum, &mut valnum_to_var, &mut valnum_count);
                        var_to_valnum.insert(dest.clone(), vn);
                        if !valnum_to_var.contains_key(&vn) {
                            valnum_to_var.insert(vn, dest.clone());
                        }
                    }

                    Primitive::Load { dest, .. } |
                    Primitive::Call { dest, .. } |
                    Primitive::Alloc { dest, .. } |
                    Primitive::GetElt { dest, .. } |
                    Primitive::Phi { dest, .. } => {
                        let vn = valnum_count;
                        valnum_count += 1;
                        var_to_valnum.insert(dest.clone(), vn);
                        valnum_to_var.insert(vn, dest.clone());
                    }

                    _ => {}
                }
            }
        }
    }

    fn get_valnum(val: &Value, var_to_valnum: &mut HashMap<String, usize>, const_to_valnum: &mut HashMap<i64, usize>, valnum_to_var: &mut HashMap<usize, String>, valnum_count: &mut usize) -> usize {

        match val {
            Value::Variable(n) => {
                if let Some(&valnum) = var_to_valnum.get(n) {
                    valnum
                } else {
                    let valnum = *valnum_count;
                    var_to_valnum.insert(n.clone(), valnum);
                    valnum_to_var.insert(valnum, n.clone());
                    valnum
                }
            }

            // constants that are the same get the same value number
            // ie if the constant is 10, then expressions using the constant 10 will match
            Value::Constant(c) => {
                if let Some(&valnum) = const_to_valnum.get(c) {
                    valnum
                } else {
                    let valnum = *valnum_count;
                    *valnum_count += 1;
                    const_to_valnum.insert(*c, valnum);
                    valnum
                }
            }
            
            // globals are treated like variables
            // ie global @vtablA gets the same value number
            // so expressions witht hose work
            Value::Global(name) => {
                if let Some(&valnum) = var_to_valnum.get(name) {
                    valnum
                } else {
                    let valnum = *valnum_count;
                    *valnum_count += 1;
                    var_to_valnum.insert(name.clone(), valnum);
                    valnum_to_var.insert(valnum, name.clone());
                    valnum
                }
            }

        }
    }
}

fn get_dest(prim: &mut Primitive) -> Option<&mut String> {
    match prim {
        Primitive::Assign { dest, .. } => Some(dest),
        Primitive::BinOp { dest, .. } => Some(dest),
        Primitive::Call { dest, .. } => Some(dest),
        Primitive::Phi { dest, .. } => Some(dest),
        Primitive::Alloc { dest, .. } => Some(dest),
        Primitive::GetElt { dest, .. } => Some(dest),
        Primitive::Load { dest, .. } => Some(dest),
        _ => None,
    }
}

// need to rename control transfers too because branch and return create usages
fn rename_control_transfer(transfer: &mut ControlTransfer, stacks: &HashMap<String, Vec<String>>) {
    match transfer {
        ControlTransfer::Branch { cond, ..} => {
            rename_value(cond, stacks);
        }
        ControlTransfer::Return { val } => {
            rename_value(val, stacks);
        }
        ControlTransfer::Jump { .. } => {}
        ControlTransfer::Fail { .. } => {}
    }
}

fn rename_uses(prim: &mut Primitive, stacks: &HashMap<String, Vec<String>>) {
    match prim {

        Primitive::Assign { value, .. } => {
            rename_value(value, stacks);
        }

        Primitive::BinOp { lhs, rhs, .. } => {
            rename_value(lhs, stacks);
            rename_value(rhs, stacks);
        }

        Primitive::Call { func, receiver, args, .. } => {
            rename_value(func, stacks);
            rename_value(receiver, stacks);
            for arg in args {
                rename_value(arg, stacks);
            }
        }

        Primitive::Print { val } => {
            rename_value(val, stacks);
        }

        Primitive::GetElt { arr, idx, .. } => {
            rename_value(arr, stacks);
            rename_value(idx, stacks);
        }

        Primitive::SetElt { arr, idx, val } => {
            rename_value(arr, stacks);
            rename_value(idx, stacks);
            rename_value(val, stacks);
        }

        Primitive::Load { addr, .. } => {
            rename_value(addr, stacks);
        }

        Primitive::Store { addr, val } => {
            rename_value(addr, stacks);
            rename_value(val, stacks);
        }

        Primitive::Phi { .. } => { }

        Primitive::Alloc { .. } => { }
    }
}

fn rename_value(val: &mut Value, stacks: &HashMap<String, Vec<String>>) {
    if let Value::Variable(name) = val {
        if let Some(stack) = stacks.get(name) {
            if let Some(current) = stack.last() {
                *name = current.clone();
            }
        }
    }
}