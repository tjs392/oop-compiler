use crate::ir::{ControlTransfer, Program};
use std::collections::HashMap;
use std::collections::HashSet;

pub struct CFG {

    // bb label -> index in program.blocks
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

impl CFG {
    pub fn new(program: &Program) -> Self {
        let num_blocks = program.blocks.len();
        let mut block_map = HashMap::new();
        let mut predecessors: Vec<Vec<usize>> = vec![vec![]; program.blocks.len()];
        let mut successors: Vec<Vec<usize>> = vec![vec![]; program.blocks.len()];

        // here we will just build the block map so we have o(1) access to the labels/index
        for (idx, block) in program.blocks.iter().enumerate() {
            block_map.insert(block.label.clone(), idx);
        }

        // this second pass will
        // 1. compute predecessors and successors for dominance frontiers
        for (idx, block) in program.blocks.iter().enumerate() {
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

}