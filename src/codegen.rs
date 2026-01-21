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
        for class in &program.classes {
            for field in &class.fields {
                if !self.global_field_ids.contains_key(field) {
                    self.global_field_ids.insert(field.clone(), next_field_id);
                    next_field_id += 1;
                }
            }
        }

        let total_fields = self.global_field_ids.len();

        for class in &program.classes {
            let mut field_map = HashMap::new();
            for (i, field) in class.fields.iter().enumerate() {
                field_map.insert(field.clone(), 2 + i);
            }

            let mut vtable_map = HashMap::new();
            for (i, method) in class.methods.iter().enumerate() {
                vtable_map.insert(method.name.clone(), i);
            }

            let vtable_vals: Vec<String> = class.methods
                .iter()
                .map(|meth| format!("{}{}", meth.name, class.name))
                .collect();

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
                todo!("handle method call")
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
            }

            Statement::FieldWrite { base, field, value } => {
                let expression = Expression::FieldWrite { 
                    base: Box::new(base.clone()), 
                    field_name: field.clone(), 
                    value: Box::new(value.clone()),
                };
                self.gen_expression(&expression);
            }

            Statement::If { condition, then_body, else_body } => {
                todo!("implement if");
            }

            Statement::IfOnly { condition, body } => {
                todo!("implement ifonly");
            }

            Statement::While { condition, body } => {
                todo!("implement while");
            }
        }
    }

    pub fn gen_program(&mut self, program: &ast::Program) -> ir::Program {
        self.gen_class_metadata(program);

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