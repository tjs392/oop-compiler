use crate::expression::Expression;
use crate::statement::Statement;
use crate::ast;
use crate::ir::{self, BasicBlock, Primitive, Value, ControlTransfer, GlobalArray};
use std::collections::HashMap;

pub struct CodeGenerator {
    temp_counter: usize,

    block_counter: usize,

    current_block: BasicBlock,

    blocks: Vec<BasicBlock>,

    globals: Vec<GlobalArray>,

    class_metadata_map: HashMap<String, ClassMetadata>,

    global_field_ids: HashMap<String, usize>,

    global_method_ids: HashMap<String, usize>,

    current_block_has_explicit_return: bool,
}

struct ClassMetadata {
    field_count: usize,
    // ex: field name -> index in fieldsA array
    // global array fieldsA: { 2, 0 }
    field_map: HashMap<String, usize>,
    // ex: method name -> index in vtblA array
    // global array vtblA: { mA }
    vtable_map: HashMap<String, usize>,
}

impl CodeGenerator {

    pub fn new() -> Self {
        CodeGenerator { 
            temp_counter: 0, 
            block_counter: 0, 
            // Going to just initialize as a basic block that returns 0
            // This is because I don't want to unwrap eberything when I access curr
            current_block: BasicBlock {
                label: "main".to_string(),
                args: vec![],
                primitives: vec![],
                control_transfer: ControlTransfer::Return {
                    val: Value::Constant(0),
                },
            },
            blocks: vec![], 
            globals: vec![],
            class_metadata_map: HashMap::new(),
            global_field_ids: HashMap::new(),
            global_method_ids: HashMap::new(),
            current_block_has_explicit_return: false,
        }
    }

    fn gen_class_metadata(&mut self, program: &ast::Program) {

        /*
        TODO: Ask prof about this logic
        On this first pass, I am assigning global field ids
        This is because the field array looks like this:
        
        ------------------
            class A [
                fields x
                ...
            ]
            class B [
                fields y
                ...
            ]
            
            compile to IR
            vvvvvv

            data:
            global array fieldsA: { 2, 0 }
            global array fieldsB: { 0, 2 }
        -------------------
        
        I need to track global field ids and have my global array be a constant size of len(all variables across classes)
        This is for runtime polymorphism

        TLDR:   Every class's field array must have an entry for EVERY global field id for EVERY class
                Where 0 means it is inaccessible by the class


        -----------------------------------
        How field access works with this
        ---------------------------------
        At compile time:
            look up x in global_field_ids -> get global id (say in this example it returns 1,)
            generate a getelt instruction "getelt(field_map_addr, 1)"
        */
        let mut next_field_id = 0;
        let mut next_method_id = 0;
        for class in &program.classes {
            for field in &class.fields {
                if !self.global_field_ids.contains_key(field) {
                    self.global_field_ids.insert(field.clone(), next_field_id);
                    next_field_id += 1;
                }
            }

            for method in &class.methods {
                if !self.global_method_ids.contains_key(&method.name) {
                    self.global_method_ids.insert(method.name.clone(), next_method_id);
                    next_method_id += 1;
                }
            }
        }

        let total_fields = self.global_field_ids.len();
        let total_methods = self.global_method_ids.len();

        for class in &program.classes {
            let mut field_map = HashMap::new();
            for (i, field) in class.fields.iter().enumerate() {
                field_map.insert(field.clone(), 2 + i);
            }

            let mut vtable_map = HashMap::new();
            for (i, method) in class.methods.iter().enumerate() {
                vtable_map.insert(method.name.clone(), i);
            }

            let mut vtable_vals: Vec<String> = vec!["0".to_string(); total_methods];
            for method in &class.methods {
                let global_id = *self.global_method_ids.get(&method.name).unwrap();
                vtable_vals[global_id] = format!("{}{}", method.name, class.name);
            }

            self.globals.push(GlobalArray { 
                name: format!("vtbl{}", class.name), 
                vals: vtable_vals,
            });

            let mut field_offsets: Vec<String> = vec!["0".to_string(); total_fields];

            for (field_name, slot_offset) in &field_map {
                let global_id = *self.global_field_ids.get(field_name).unwrap();
                field_offsets[global_id] = slot_offset.to_string();
            }

            self.globals.push(GlobalArray { 
                name: format!("fields{}", class.name), 
                vals: field_offsets,
            });

            let metadata = ClassMetadata {
                field_count: class.fields.len(),
                field_map,
                vtable_map,
            };

            self.class_metadata_map.insert(class.name.clone(), metadata);
        }
    }

    fn gen_unique_variable(&mut self, prefix: &str) -> String {
        let name = format!("{}{}", prefix, self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn gen_unique_label(&mut self, prefix: &str) -> String {
        let label = format!("{}{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }

    fn push_instruction(&mut self, primitive: Primitive) {
        self.current_block.primitives.push(primitive);
    }

    fn finish_block(&mut self, transfer: ControlTransfer, next_label: String) {
        self.current_block.control_transfer = transfer;
        // clone here acts as a move from current block -> blocks
        self.blocks.push(self.current_block.clone());

        self.current_block = BasicBlock {
            label: next_label,
            args: vec![],
            primitives: vec![],
            control_transfer: ControlTransfer::Return {
                val: Value::Constant(0),
            },
        };
        self.current_block_has_explicit_return = false;
    }

    // need to return value for generation of nested expressions and statements
    fn gen_expression(&mut self, expression: &Expression) -> Value {
        match expression {

            Expression::Constant(n) => {
                Value::Constant(*n)
            }

            Expression::Variable(name) => {
                Value::Variable(name.clone())
            }

            Expression::Binop { lhs, op, rhs } => {
                let left = self.gen_expression(lhs);
                let right = self.gen_expression(rhs);

                let result = self.gen_unique_variable("result");

                self.push_instruction(Primitive::BinOp { 
                    dest: result.clone(), 
                    lhs: left, 
                    op: op.to_string(), 
                    rhs: right
                });

                Value::Variable(result)
            }

            Expression::ThisExpr => {
                Value::Variable("this".to_string())
            }

            Expression::ClassRef(class_name) => {
                /*
                    # x = new A
                    %x0 = alloc(3)    # vtable, field map, field x
                    store(%x0, @vtblA)
                    %1 = %x0 + 8
                    store(%1, @fieldsA)
                */
                let metadata = self.class_metadata_map.get(class_name)
                    .expect(&format!("Class {} not found", class_name));

                let alloc_size = 2 + metadata.field_count as i64;
                let obj_addr = self.gen_unique_variable("objAddr");
                self.push_instruction(Primitive::Alloc { 
                    dest: obj_addr.clone(), 
                    size: alloc_size, 
                });

                self.push_instruction(Primitive::Store { 
                    addr: Value::Variable(obj_addr.clone()),
                    val: Value::Global(format!("vtbl{}", class_name)),
                });

                let fields_addr = self.gen_unique_variable("fieldsAddr");
                self.push_instruction(Primitive::BinOp { 
                    dest: fields_addr.clone(), 
                    lhs: Value::Variable(obj_addr.clone()), 
                    op: "+".to_string(), 
                    rhs: Value::Constant(8), 
                });

                self.push_instruction(Primitive::Store { 
                    addr: Value::Variable(fields_addr.clone()), 
                    val: Value::Global(format!("fields{}", class_name)),
                });

                Value::Variable(obj_addr)
            }

            // TODO: DRY -> There is repeated code for field access on field read & write. Congregate this into helper function
            Expression::FieldRead { base, field_name } => {
                /*
                    Field read is a bit confusing this is how it works:

                    -- Compile Time --
                        - Look up field_name in global_fields_ids -> get the global id.
                            This is the index in the global field arrays at which it is at
                            Ex: Let's say 'x' is the first field declared in the first class, it will be at idx 0
                                Let's say 'y' is the FIRST field declared in the SECOND class, it will be at idx 1

                    -- Run Time--
                        - We will use this value at run time and search the field array. If the value at this location is 0
                            then we want to fail because it's not accessible to the calling class
                        - If it is something else, ie it will be 2 for the first field of any class, then we
                            can you that value as an 8 * val offset for the mem addr for this

                    Example code to generate:
                    # !x.x = 3 (unoptimized)
                    %2 = %x0 & 1
                    if %2 then badptr2 else firstStoreX
                    firstStoreX:
                    %3 = %x0 + 8         # Address to 2nd slot for *field* map
                    %4 = load(%3)       # Load field map
                    %5 = getelt(%4, 0)  # Look up field id 0, which I assume is x
                    if %5 then firstStoreXWorks else badfield2
                    firstStoreXWorks:
                    setelt(%x0, %5, 3)
                */

                // base_val is the address of the object instance that we want to read its field
                let base_val = self.gen_expression(base);

                // check the tag to make sure its last bit is not 1 (badptr)
                let tag = self.gen_unique_variable("tag");
                self.push_instruction(Primitive::BinOp { 
                    dest: tag.clone(), 
                    lhs: base_val.clone(), 
                    op: "&".to_string(), 
                    rhs: Value::Constant(1), 
                });

                let bad_ptr_label = self.gen_unique_label("badptr");
                let continue_label = self.gen_unique_label("firstStore");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(tag), 
                        then_lab: bad_ptr_label.clone(), 
                        else_lab: continue_label.clone() 
                    },
                    continue_label.clone()
                );

                // load the field map address
                let field_map_addr = self.gen_unique_variable("fieldMapAddr");
                self.push_instruction(Primitive::BinOp { 
                    dest: field_map_addr.clone(), 
                    lhs: base_val.clone(), 
                    op: "+".to_string(), 
                    rhs: Value::Constant(8), 
                });

                let field_map = self.gen_unique_variable("fieldMap");
                self.push_instruction(Primitive::Load { 
                    dest: field_map.clone(), 
                    addr: Value::Variable(field_map_addr),
                });

                // look up the offset using the global field id
                let global_idx = *self.global_field_ids.get(field_name)
                    .expect(&format!("Field {} nt ofund", field_name));
                let offset = self.gen_unique_variable("offset");
                self.push_instruction(Primitive::GetElt { 
                    dest: offset.clone(), 
                    arr: Value::Variable(field_map), 
                    idx: Value::Constant(global_idx as i64), 
                });

                // check field exists for the class (offset != 0)
                let bad_field_label = self.gen_unique_label("badfield");
                let load_label = self.gen_unique_label("load");
                
                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(offset.clone()), 
                        then_lab: load_label.clone(), 
                        else_lab: bad_field_label.clone()
                    }, 
                    load_label.clone()
                );

                // load in the value
                let result = self.gen_unique_variable("result");
                self.push_instruction(Primitive::GetElt { 
                    dest: result.clone(), 
                    arr: base_val, 
                    idx: Value::Variable(offset), 
                });

                // fail labels
                let final_label = self.gen_unique_label("final");
                self.finish_block(
                    ControlTransfer::Jump {target: final_label.clone() },
                    bad_ptr_label.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    bad_field_label.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NoSuchField".to_string() },
                    final_label.clone()
                );

                Value::Variable(result)
            }

            Expression::FieldWrite { base, field_name, value } => {
                /*
                # !x.x = 3 (unoptimized)
                %2 = %x0 & 1
                if %2 then badptr2 else firstStoreX
                firstStoreX:
                %3 = %x0 + 8         # Address to 2nd slot for *field* map
                %4 = load(%3)       # Load field map
                %5 = getelt(%4, 0)  # Look up field id 0, which I assume is x
                if %5 then firstStoreXWorks else badfield2
                firstStoreXWorks:
                setelt(%x0, %5, 3)
                 */

                let base_val = self.gen_expression(base);
                let val = self.gen_expression(value);

                // check the tag to make sure its last bit is not 1 (badptr)
                // %2 = %x0 & 1
                let tag = self.gen_unique_variable("tag");
                self.push_instruction(Primitive::BinOp { 
                    dest: tag.clone(), 
                    lhs: base_val.clone(), 
                    op: "&".to_string(), 
                    rhs: Value::Constant(1), 
                });

                // if %2 then badptr2 else firstStoreX
                // firstStoreX:
                let bad_ptr_label = self.gen_unique_label("badptr");
                let continue_label = self.gen_unique_label("firstStore");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(tag), 
                        then_lab: bad_ptr_label.clone(), 
                        else_lab: continue_label.clone() 
                    },
                    continue_label.clone()
                );

                // %3 = %x0 + 8
                let field_map_addr = self.gen_unique_variable("fieldMapAddr");
                self.push_instruction(Primitive::BinOp { 
                    dest: field_map_addr.clone(), 
                    lhs: base_val.clone(), 
                    op: "+".to_string(), 
                    rhs: Value::Constant(8), 
                });

                // %4 = load(%3)       # Load field map
                let field_map = self.gen_unique_variable("fieldMap");
                self.push_instruction(Primitive::Load { 
                    dest: field_map.clone(), 
                    addr: Value::Variable(field_map_addr),
                });

                // %5 = getelt(%4, 0)  # Look up field id 0, which I assume is x
                let global_idx = *self.global_field_ids.get(field_name)
                    .expect(&format!("Field {} not found", field_name));
                let offset = self.gen_unique_variable("offset");
                self.push_instruction(Primitive::GetElt { 
                    dest: offset.clone(), 
                    arr: Value::Variable(field_map), 
                    idx: Value::Constant(global_idx as i64),
                });

                // if %5 then firstStoreXWorks else badfield2
                let bad_field_label = self.gen_unique_label("badfield");
                let load_label = self.gen_unique_label("load");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(offset.clone()), 
                        then_lab: load_label.clone(), 
                        else_lab: bad_field_label.clone()
                    }, 
                    load_label.clone()
                );

                // firstStoreXWorks:
                // setelt(%x0, %5, 3)
                self.push_instruction(Primitive::SetElt { 
                    arr: base_val, 
                    idx: Value::Variable(offset),  
                    val: val, 
                });

                // fail labels
                let final_label = self.gen_unique_label("final");
                self.finish_block(
                    ControlTransfer::Jump {target: final_label.clone() },
                    bad_ptr_label.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    bad_field_label.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NoSuchField".to_string() },
                    final_label.clone()
                );

                Value::Constant(0)
            }
            
            Expression::MethodCall { base, method_name, args } => {
                let base = self.gen_expression(base);
                /*
                # print(x.m())
                %7 = %x0 & 1
                if %7 then badptr3 else l1
                l1:
                %8 = load(%x0)         # load vtable (note: offset 0, not offset 8)
                %9 = getelt(%8, 0)  # lookup method id 0 (the only method here)
                if %9 then callAndPrint else badmethod
                callAndPrint:
                %10 = call(%9, %x0)
                print(%10)
                */

                // For printing
                // %7 = %x0 & 1
                let tag = self.gen_unique_variable("tag");
                self.push_instruction(Primitive::BinOp { 
                    dest: tag.clone(), 
                    lhs: base.clone(), 
                    op: "&".to_string(), 
                    rhs: Value::Constant(1),
                });

                // if %7 then badptr3 else l1
                let badptr = self.gen_unique_label("badptr");
                let load = self.gen_unique_label("load");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(tag), 
                        then_lab: badptr.clone(), 
                        else_lab: load.clone(), 
                    },
                    load.clone(),
                );

                // %8 = load (%x0)
                let vtable = self.gen_unique_variable("vtable");
                self.push_instruction(Primitive::Load { 
                    dest: vtable.clone(), 
                    addr: base.clone(),  
                });

                // %9 = getelt(%8, 0)
                let global_method_id = *self.global_method_ids.get(method_name)
                    .expect(&format!("Method {} not found", method_name));
                let method_ptr = self.gen_unique_variable("methodPtr");
                self.push_instruction(Primitive::GetElt { 
                    dest: method_ptr.clone(), 
                    arr: Value::Variable(vtable), 
                    idx: Value::Constant(global_method_id as i64),
                });

                // if %9 then callAndPrint else badmethod
                let badmethod = self.gen_unique_label("badmethod");
                let call_and_print = self.gen_unique_label("callAndPrint");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(method_ptr.clone()), 
                        then_lab: call_and_print.clone(), 
                        else_lab: badmethod.clone() 
                    },
                    call_and_print.clone()
                );


                // callAndPrint:
                // %10 = call(%9, %x0)

                let result = self.gen_unique_variable("callResult");
                let arguments: Vec<Value> = args
                    .iter()
                    .map(|a| self.gen_expression(a))
                    .collect();
                
                self.push_instruction(Primitive::Call { 
                    dest: result.clone(), 
                    func: Value::Variable(method_ptr.clone()), 
                    receiver: base,
                    args: arguments,
                });

                // fail labels
                let final_label = self.gen_unique_label("final");
                self.finish_block(
                    ControlTransfer::Jump { target: final_label.clone() },
                    badptr.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    badmethod.clone()
                );

                self.finish_block(
                    ControlTransfer::Fail { message: "NoSuchMethod".to_string() },
                    final_label.clone()
                );
                
                Value::Variable(result)
            }
        }
    }

    fn gen_statement(&mut self, statement: &Statement) {
        match statement {

            Statement::Assignment { variable, expression } => {
                let val = self.gen_expression(expression);

                self.push_instruction(Primitive::Assign {
                    dest: variable.clone(), 
                    value: val,
                });
            }

            Statement::Discard(expr) => {
                self.gen_expression(expr);
            }

            Statement::Print(expression) => {
                let val = self.gen_expression(expression);

                self.push_instruction(Primitive::Print { val });
            }

            Statement::Return(expression) => {
                let val = self.gen_expression(expression);

                self.current_block.control_transfer = ControlTransfer::Return { val };
                self.current_block_has_explicit_return = true;
            }

            Statement::FieldWrite { base, field, value } => {
                let expression = Expression::FieldWrite { 
                    base: Box::new(base.clone()), 
                    field_name: field.clone(), 
                    value: Box::new(value.clone()),
                };
                self.gen_expression(&expression);
            }

            /*
            if e: {
                statement1
                statement2
            } else {
                statement1
                statement2
            }
            */
            Statement::If { condition, then_body, else_body } => {
                // here we'll want to make an if label with a condition
                // %condition = expression

                /*
                    CFG:
                    current_block:
                        branch cond -> then, else

                    then:
                        body
                        jump -> merge
                    
                    else:
                        body
                        jump -> merge
                    
                    # new basic block:
                    merge:
                        continue
                        
                */
                let condition = self.gen_expression(condition);
                let then_label = self.gen_unique_label("then");
                let else_label = self.gen_unique_label("else");
                let merge_label = self.gen_unique_label("merge");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: condition, 
                        then_lab: then_label.clone(), 
                        else_lab: else_label.clone(), 
                    },
                    then_label,
                );
                for statement in then_body {
                    self.gen_statement(statement);
                }

                // here we need to check if the then body is returning something
                // because if the then body returns something, we need to handle the return and not
                // just jump blindly
                // we can check this just by checking the current basic block's control transfer, if it is a return
                let then_control_transfer = 
                    if self.current_block_has_explicit_return {
                        self.current_block.control_transfer.clone()
                    } else {
                        ControlTransfer::Jump { target: merge_label.clone() }
                    };
                self.finish_block(then_control_transfer, else_label);

                for statement in else_body {
                    self.gen_statement(statement);
                }

                let else_control_transfer = 
                    if self.current_block_has_explicit_return {
                        self.current_block.control_transfer.clone()
                    } else {
                        ControlTransfer::Jump { target: merge_label.clone() }
                    };
                self.finish_block(else_control_transfer, merge_label);
            }

            Statement::IfOnly { condition, body } => {

                let then_label = self.gen_unique_label("then");
                let merge_label = self.gen_unique_label("merge");
                let condition = self.gen_expression(condition);
                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: condition, 
                        then_lab: then_label.clone(), 
                        else_lab: merge_label.clone(),
                    },
                    then_label.clone(),
                );

                for statement in body {
                    self.gen_statement(statement);
                }

                let then_control_transfer = 
                    if self.current_block_has_explicit_return {
                        self.current_block.control_transfer.clone()
                    } else {
                        ControlTransfer::Jump { target: merge_label.clone() }
                    };
                self.finish_block(then_control_transfer, merge_label);
            }

            Statement::While { condition, body } => {
                todo!("implement while");
            }
        }
    }

    fn gen_method(&mut self, class: &ast::Class, method: &ast::Method) {
        /*
        This is code from the IR parser.
        It shows thats when parsing a basic block for a mthod, it looks for arguments as
            
        > methodName(this, arg1, arg2...) with locals:

        pub fn parse_opt_block_arg_list(i: &[u8]) -> IResult<&[u8], Vec<&str>> {
            alt((
                |x| tuple((tag(":"),opt(tag("\r")),tag("\n")))(x).map(|(rest,_)| (rest, vec![])),
                |x| tuple((tag("("), multispace0, separated_list0(tuple((multispace0,tag(","),multispace0)),parse_block_arg), multispace0, tag("):"), opt(tag("\r")), tag("\n")))(x).map(|(rest,(_,_,args,_,_,_,_))| (rest,args))
            ))(i)
        }
        pub fn parse_basic_block(i: &[u8]) -> IResult<&[u8], BasicBlock> {
            let (i,_) = multispace0(i)?;
            tuple((
                identifier, parse_opt_block_arg_list, parse_ir_statements, parse_control
            ))(i).map(|(rest,(name,formals,prims,ctrl))| (rest,BasicBlock { name: name, instrs: prims, next: ctrl, formals: formals}))
        }
        */
        let method_label = format!("{}{}", method.name, class.name);

        let mut args = vec!["this".to_string()];
        for arg in &method.args {
            args.push(arg.clone());
        }

        self.current_block = BasicBlock { 
            label: method_label, 
            args, 
            primitives: vec![], 
            control_transfer: ControlTransfer::Return { val: Value::Constant(0) },
        };

        for statement in &method.body {
            self.gen_statement(statement);
        }

        // need to make sure there is a return
        if !matches!(self.current_block.control_transfer, ControlTransfer::Return { .. }) {
            self.current_block.control_transfer = ControlTransfer::Return { 
                val: Value::Constant(0),
            }
        }

        self.blocks.push(self.current_block.clone());
    }

    pub fn gen_program(&mut self, program: &ast::Program) -> ir::Program {
        self.gen_class_metadata(program);

        for class in &program.classes {
            for method in &class.methods {
                self.gen_method(class, method);
            }
        }

        // After generating methods, we need to reset the current basic vlock back to main
        self.current_block = BasicBlock { 
            label: "main".to_string(), 
            args: vec![], 
            primitives: vec![], 
            control_transfer: ControlTransfer::Return { 
                val: Value::Constant(0)
             } 
        };

        for statement in &program.main_body {
            self.gen_statement(statement);
        }

        if !matches!(self.current_block.control_transfer, ControlTransfer::Return { .. }) {
            self.current_block.control_transfer = ControlTransfer::Return {
                val: Value::Constant(0),
            };
        }

        self.blocks.push(self.current_block.clone());


        ir::Program {
            globals: self.globals.clone(),
            blocks: self.blocks.clone(),
        }
    }
}