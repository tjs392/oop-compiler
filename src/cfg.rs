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

    pub fn convert_to_ssa(&mut self, function: &mut Function) {
        self.compute_dominator_sets();
        self.insert_phi_functions(function);

        let tree = self.build_dominator_tree();
        let mut stacks: HashMap<String, Vec<String>> = HashMap::new();
        let mut counter: usize = 0;

        self.rename(function, self.entry, &mut stacks, &mut counter, &tree);
    }
    // dom(block) = { block } U { & dom(pred) } for all pred in pred(block) }
    fn compute_dominator_sets(&mut self) {
        let mut dominator_sets: Vec<HashSet<usize>> = vec![HashSet::new(); self.num_blocks];

        dominator_sets[0].insert(0);
        let all_blocks: HashSet<usize> = (0..self.num_blocks).collect();
        for idx in 1..self.num_blocks {
            dominator_sets[idx] = all_blocks.clone();
        }

        let mut changed = true;
        while changed {
            changed = false;

            for idx in 1..self.num_blocks {
                let preds = &self.predecessors[idx];

                if preds.is_empty() {
                    continue;
                }

                let mut new_dom = dominator_sets[preds[0]].clone();
                for &pred in &preds[1..] {
                    new_dom = new_dom.intersection(&dominator_sets[pred]).copied().collect();
                }

                new_dom.insert(idx);

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
        let immediate_dominators = self.compute_immediate_dominators();
        let mut dominance_fronters: Vec<HashSet<usize>> = vec![HashSet::new(); self.num_blocks];

        for idx in 0..self.num_blocks {

            if self.predecessors[idx].len() >= 2 {
                for &pred in &self.predecessors[idx] {
                    let mut runner = pred;
                    while Some(runner) != immediate_dominators[idx] {
                        dominance_fronters[runner].insert(idx);

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

    // param counters tracks how many version of the variable have been created
    // param "stacks" is a stack of each variable "rename" need this because the algorithm pushes the variable rename after backtracking the dominaance tree
    fn rename(&mut self, 
                function: &mut Function, 
                idx: usize, 
                stacks: &mut HashMap<String, Vec<String>>,
                counter: &mut usize,
                tree: &Vec<Vec<usize>>) {
        
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

                *assignment = new_name.clone();
                stacks.entry(old_name.clone()).or_insert_with(Vec::new).push(new_name);
                *pushed.entry(old_name).or_insert(0) += 1;
            }
        }

        rename_control_transfer(&mut block.control_transfer, stacks);

        let this_label = function.blocks[idx].label.clone();
        let successors = self.successors[idx].clone();

        // fill phi arguments
        for succ_idx in successors {
            let succ_block = &mut function.blocks[succ_idx];

            for primitive in &mut succ_block.primitives {
                if let Primitive::Phi { args, .. } = primitive {
                    for (label, val) in args {
                        if label == &this_label {
                            if let Value::Variable(var_name) = val {
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

        for &child in &tree[idx] {
            self.rename(function, child, stacks, counter, tree);
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
            for block in &mut function.blocks {
                for i in 0..block.primitives.len() {
                    if let Some(folded) = Self::try_fold_constant(&block.primitives[i]) {
                        block.primitives[i] = folded;
                        changed = true;
                    }
                }
            }
        }
    }

    fn try_fold_constant(prim: &Primitive) -> Option<Primitive> {
        match prim {

            Primitive::BinOp { dest, lhs, op, rhs } => {
                if let (Value::Constant(left), Value::Constant(right)) = (lhs, rhs) {
                    if let Some(result) = Self::evaluate_binop(op, *left, *right) {
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