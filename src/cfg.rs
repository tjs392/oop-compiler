use crate::ir::{BasicBlock, Program};
use crate::ir;
use std::collections::HashMap;

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
}

impl CFG {

    pub fn new(program: &Program) -> Self {
        CFG { 
            block_map: HashMap::<String, usize>::new(), 
            predecessors: vec![vec![]], 
            successors: vec![vec![]], 
            entry: 0,
        }
    }
}