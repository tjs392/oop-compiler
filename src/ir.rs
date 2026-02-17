#[derive(Debug, Clone)]
pub enum Value {
    Constant(i64),
    Variable(String),
    Global(String)
}

#[derive(Debug, Clone)]
pub enum Primitive {
    
    // %v = 69  or  %v = %x
    // %v = i where i is a local variable or constant
    Assign {
        dest: String,
        value: Value,
    },

    // %v = %w OP %x for OP in +, -, *, /, | (bitwise or), & (bitwise and), ^ (bitwise xor), ==
    BinOp {
        dest: String,
        lhs: Value,
        op: String,
        rhs: Value,
    },

    // %v = call(%func, %receiver, %arg1, %arg2, ...)
    // where %func is a local holding a code address, %receiver is the receiver of a method call
    Call {
        dest: String,
        func: Value,
        receiver: Value,
        args: Vec<Value>,
    },

    // %v = phi(name, %x, name, %y, ...) is a phi function. 
    // There must be at least 4 arguments (at least two predecessor blocks), and there must be an even number of arguments.
    Phi {
        dest: String,
        args: Vec<(String, Value)>
    },

    // %v = alloc(n) where n is a constant integer, representing the number of pointer-sized fields to allocate. 
    // Alternatively, allocates an array of n value slots.
    Alloc {
        dest: String,
        size: i64,
    },

    // print(%v) prints the value passed, either a register (as shown) or a constant
    Print {
        val: Value,
    },

    // %v = getelt(%a, i) retrieves the i-th element of an array pointed to by %a. i may be a constant or a variable
    GetElt {
        dest: String,
        arr: Value,
        idx: Value,
    },

    // setelt(%a, i, i2) sets the i-th element of an array pointed to by %a to i2. 
    // i and i2 may be constants, local variables, or globals
    SetElt {
        arr: Value,
        idx: Value,
        val: Value,
    },

    // %v = load(%base) loads the 8 bytes at the address pointed to by %base
    Load {
        dest: String,
        addr: Value,
    },

    // store(%base, i) stores i at address %base. i may be a local variable, global, or constant
    Store {
        addr: Value,
        val: Value,
    },
}

#[derive(Debug, Clone)]
pub enum ControlTransfer {

    // jump <name> is an unconditional branch to the block with the specified name. 
    // The name does not include the colon (the colon for each block’s syntax marks the end of the block name).
    Jump {
        target: String,
    },

    // if %v then <name> else <name> branches to the first name if %v is true, otherwise the second name
    Branch {
        cond: Value,
        then_lab: String,
        else_lab: String,
    },

    // ret %v or ret n for some int literal n
    Return {
        val: Value,
    },

    // fail m crashes the program. m must be one of:
    //  NotAPointer (to indicate a pointer operation like field access or method invocation was used with a non-pointer)
    //  NotANumber (to indicate an arithmetic operation was attempted with a non-number value)
    //  NoSuchField
    //  NoSuchMethod
    Fail {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub primitives: Vec<Primitive>,
    pub control_transfer: ControlTransfer,
}

// https://github.com/rust-lang/rust/blob/main/compiler/rustc_middle/src/mir/mod.rs
// rustc represents "Body" like functoin
// one closed entity with the basic blocks that ar einside of it
// this will allow me for easier CFG -> SSA conversion 
// here just going to define "function" like a simple Body struct with basic blocks
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<String>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone)]
pub struct GlobalArray {
    pub name: String,
    pub vals: Vec<String>,
}

#[derive(Debug)]
pub struct Program {
    pub globals: Vec<GlobalArray>,
    pub functions: Vec<Function>,
}

/*
pub struct Cat {
    /\_/\
    |0-O|           ==__\
  ==  ^ ==  _____       \ \
    \ -  \    __  \      \ \
    __( \           ( \___/ |
  <______> ___ (______)____/
}
*/

// Code Gen to stdout rn
impl Program {
    pub fn print(&self) {
        const INDENT: &str = "  ";

        // data/global array section
        println!("data:");
        for global in &self.globals {
            print!("global array {}: {{ ", global.name);
            for (i, val) in global.vals.iter().enumerate() {
                if i > 0 { print!(", "); }
                print!("{}", val);
            }
            println!(" }}");
        }

        // basic block (code) section
        println!("\ncode:");
        for function in &self.functions {
            print!("\n{}", function.name);
            if !function.args.is_empty() {
                print!("({})", function.args.join(", "));
            }
            println!(":");

            for (i, block) in function.blocks.iter().enumerate() {
                if i > 0 {
                    println!("\n{}:", block.label);
                }

                for prim in &block.primitives {
                    println!("{}{}", INDENT, self.format_primitive(prim));
                }

                println!("{}{}", INDENT, self.format_control_transfer(&block.control_transfer));
            }
        }
    }
    fn format_primitive(&self, prim: &Primitive) -> String {
        match prim {

            Primitive::Assign { dest, value } => {
                format!("%{} = {}", dest, self.format_value(value))
            },

            Primitive::BinOp { dest, lhs, op, rhs } => {
                format!("%{} = {} {} {}", dest, self.format_value(lhs), op, self.format_value(rhs))
            },

            Primitive::Call { dest, func, receiver, args } => {
                if args.is_empty() {
                    format!("%{} = call({}, {})",
                        dest,
                        self.format_value(func),
                        self.format_value(receiver),
                    )
                } else {
                    let args_string: String =
                        args.iter()
                            .map(|a| self.format_value(a))
                            .collect::<Vec<String>>()
                            .join(", ");

                    format!("%{} = call({}, {}, {})",
                        dest,
                        self.format_value(func),
                        self.format_value(receiver),
                        args_string,
                    )
                }
            },

            Primitive::Phi { dest, args } => {
                let args_string: String = 
                    args.iter()
                        .map(|(label, val)| format!("{}, {}", label, self.format_value(val)))
                        .collect::<Vec<String>>()
                        .join(", ");

                format!("%{} = phi({})", dest, args_string)
            },

            Primitive::Alloc { dest, size } => {
                format!("%{} = alloc({})", dest, size)
            },

            Primitive::Print { val } => {
                format!("print({})", self.format_value(val))
            },

            Primitive::GetElt { dest, arr, idx } => {
                format!("%{} = getelt({}, {})", dest, self.format_value(arr), self.format_value(idx))
            },

            Primitive::SetElt { arr, idx, val } => {
                format!("setelt({}, {}, {})", self.format_value(arr), self.format_value(idx), self.format_value(val))
            },

            Primitive::Load { dest, addr } => {
                format!("%{} = load({})", dest, self.format_value(addr))
            },

            Primitive::Store { addr, val } => {
                format!("store({}, {})", self.format_value(addr), self.format_value(val))
            }
        }
    }

    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::Constant(num) => num.to_string(),

            Value::Variable(var) => format!("%{}", var),

            Value::Global(global) => format!("@{}", global),
        }
    }

    fn format_control_transfer(&self, control: &ControlTransfer) -> String {
        match control {
            ControlTransfer::Jump { target } => {
                format!("jump {}", target)
            },

            ControlTransfer::Branch { cond, then_lab, else_lab } => {
                format!("if {} then {} else {}", self.format_value(cond), then_lab, else_lab)
            },

            ControlTransfer::Return { val } => {
                format!("ret {}", self.format_value(val))
            },

            ControlTransfer::Fail { message } => {
                format!("fail {}", message)
            }
        }
    }
}






