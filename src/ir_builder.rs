use crate::expression::Expression;
use crate::statement::Statement;
use crate::ast;
use crate::ir::{self, BasicBlock, Function, Primitive, Value, ControlTransfer, GlobalArray};
use crate::token::Operator;
use std::collections::HashMap;

pub struct IRBuilder {
    temp_counter: usize,
    block_counter: usize,

    current_block: BasicBlock,
    current_function_blocks: Vec<BasicBlock>,
    
    functions: Vec<Function>,
    globals: Vec<GlobalArray>,

    class_metadata_map: HashMap<String, ClassMetadata>,
    global_field_ids: HashMap<String, usize>,
    global_method_ids: HashMap<String, usize>,

    current_block_has_explicit_return: bool,

    type_environment: HashMap<String, ast::Type>,
    classes: Vec<ast::Class>,

    var_types: HashMap<String, ast::Type>,
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

/*
This ir BUIlder builds the following tree:

Program
|
|__ globals: Vec<GlobalArray>
|
|__ functions: Vec<Function>
    |
    |___ Function "main"
    |   |
    |   |__ name: "main"
    |   |__ args: []
    |   \__ blocks: Vec<BascBlock>
    |       \__ BasicBlock "entry"
    |       \__ other basic blocks
    |
    |
    |___ Function "funcA"
        |__ name ..
        \__ args, etc..
        
this is loosely inspired how rustc handles body and contains functions
for easy ssa gen
https://github.com/rust-lang/rust/blob/main/compiler/rustc_middle/src/mir/mod.rs

by grouping blocks into functions, we can process each function independently to convert to ssa

i tried without grouping, and it made it super hard to convert to ssa
*/
impl IRBuilder {

    pub fn new() -> Self {
        IRBuilder { 
            temp_counter: 0, 
            block_counter: 0, 
            current_block: BasicBlock {
                label: "entry".to_string(),
                primitives: vec![],
                control_transfer: ControlTransfer::Return {
                    val: Value::Constant(0),
                },
            },
            current_function_blocks: vec![],
            functions: vec![],
            globals: vec![],
            class_metadata_map: HashMap::new(),
            global_field_ids: HashMap::new(),
            global_method_ids: HashMap::new(),
            current_block_has_explicit_return: false,
            type_environment: HashMap::new(),
            classes: vec![],
            var_types: HashMap::new(),
        }
    }

    fn evaluate_type(&self, expr: &Expression) -> ast::Type {
        match expr {
            Expression::Variable(name) => self.type_environment.get(name).unwrap().clone(),

            Expression::ThisExpr => self.type_environment.get("this").unwrap().clone(),

            Expression::ClassRef(name) => ast::Type::ClassType(name.clone()),

            Expression::Null(name) => ast::Type::ClassType(name.clone()),

            // constants & binoips are always ints
            Expression::Constant(_) => ast::Type::Int,

            Expression::Binop { .. } => ast::Type::Int,

            // we can recursively eval the type in a field read by evaluate the base and then 
            // find the type
            Expression::FieldRead { base, field_name } => {
                if let ast::Type::ClassType(class_name) = self.evaluate_type(base) {
                    let class = self.classes.iter().find(|c| c.name == class_name).unwrap();
                    class.fields.iter()
                        .find(|(n, _)| n == field_name)
                        .map(|(_, t)| t.clone())
                        .unwrap()
                } else { panic!("field read on int") }
            }

            Expression::FieldWrite { base, .. } => {
                self.evaluate_type(base)
            }
            
            // method call is like field read
            Expression::MethodCall { base, method_name, .. } => {
                if let ast::Type::ClassType(class_name) = self.evaluate_type(base) {
                    let class = self.classes.iter().find(|c| c.name == class_name).unwrap();
                    let method = class.methods.iter().find(|m| m.name == *method_name).unwrap();
                    method.return_type.clone()
                } else { panic!("method call on int") }
            }
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

        // first pass: need to assign globally unique ids to every field and method
        //              across each class. this is because at runtime, any object
        //              could be passed where another is expected due to polymorphism)
        //              so each class's field/vtable arrays must be the same size
        //              and use the same indicies for the same name

        /* 
        C;ass A layout:  [vtable_ptr, field_map_ptr, x_value]
                          slot 0      slot 1         slot 2

        Class B layout:  [vtable_ptr, field_map_ptr, x_value]  
                           slot 0      slot 1         slot 2

        fieldsA: { 2 } = "field with global ID 0 is at slot 2 in an A object"
        fieldsB: { 2 } = "field with global ID 0 is at slot 2 in a B object"
         */
        let mut next_field_id = 0;
        let mut next_method_id = 0;

        // THIS IS WHAT ALOWS THE POLYMORPHISM
        for class in &program.classes {
            for (field, _) in &class.fields {
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

        // second pass : build each class's vtable and field offset arrays globally
        //               and then store than metadata for codegen
        for class in &program.classes {
            // field_name -> slot offset within object
            let mut field_map = HashMap::new();
            for (i, (field, _)) in class.fields.iter().enumerate() {
                field_map.insert(field.clone(), 1 + i);
            }

            // method name -> index within the class's method list
            let mut vtable_map = HashMap::new();
            for (i, method) in class.methods.iter().enumerate() {
                vtable_map.insert(method.name.clone(), i);
            }

            // size = total_methods across all classes
            // "0" means the class doesnt implement that method
            // for methods this class does implement, stor the ir func name
            // which the urntime will evaluate as a function pointer
            let mut vtable_vals: Vec<String> = vec!["0".to_string(); total_methods];
            for method in &class.methods {
                let global_id = *self.global_method_ids.get(&method.name).unwrap();
                vtable_vals[global_id] = format!("{}{}", method.name, class.name);
            }

            self.globals.push(GlobalArray { 
                name: format!("vtbl{}", class.name), 
                vals: vtable_vals,
            });

            // size = total_fields across all classes
            // same as vtable array, 0 means that it doesnt have the field
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
        self.current_function_blocks.push(self.current_block.clone());

        self.current_block = BasicBlock {
            label: next_label,
            primitives: vec![],
            control_transfer: ControlTransfer::Return {
                val: Value::Constant(0),
            },
        };
        self.current_block_has_explicit_return = false;
    }

    // in this refactor, we will be grouping basic blocks into functions
    // this will allow ssa gen to walk functions instead of the entire program
    // finish function has the same logic as finish basic block
    // we just teack the basic blocks, and when we reach the final one for the func
    // push function w/ its basic blocks to the builder
    fn finish_function(&mut self, name: String, args: Vec<String>) {
        if !matches!(self.current_block.control_transfer, ControlTransfer::Return { .. }) {
            self.current_block.control_transfer = ControlTransfer::Return { val: Value::Constant(0) }
        }
        self.current_function_blocks.push(self.current_block.clone());


        self.functions.push(Function {
            name,
            args,
            // we can just transfer the ownership 
            blocks: std::mem::take(&mut self.current_function_blocks),
        });

        self.current_block = BasicBlock {
            label: "entry".to_string(),
            primitives: vec![],
            control_transfer: ControlTransfer::Return { val: Value::Constant(0) }
        };
        self.current_block_has_explicit_return = false;
    }

    // need to return value for generation of nested expressions and statements
    fn gen_expression(&mut self, expression: &Expression) -> Value {
        match expression {

            // if its a contant, tag the leftmost bit with 1
            Expression::Constant(n) => {
                Value::Constant(*n)
            }

            Expression::Variable(name) => {
                Value::Variable(name.clone())
            }

            // we no longer need to do type checking so just do raw math
            Expression::Binop { lhs, op, rhs } => {
                let left = self.gen_expression(lhs);
                let right = self.gen_expression(rhs);

                if *op == Operator::Equals {
                    let result = self.gen_unique_variable("result");
                    self.var_types.insert(result.clone(), ast::Type::Int);

                    self.push_instruction(Primitive::BinOp {
                        dest: result.clone(),
                        lhs: left,
                        op: "==".to_string(),
                        rhs: right,
                    });
                    return Value::Variable(result);
                }

                if *op == Operator::NotEquals {
                    let eq_result = self.gen_unique_variable("eqResult");
                    self.var_types.insert(eq_result.clone(), ast::Type::Int);
                    self.push_instruction(Primitive::BinOp {
                        dest: eq_result.clone(),
                        lhs: left,
                        op: "==".to_string(),
                        rhs: right,
                    });
                    let result = self.gen_unique_variable("result");
                    self.var_types.insert(result.clone(), ast::Type::Int);

                    self.push_instruction(Primitive::BinOp {
                        dest: result.clone(),
                        lhs: Value::Variable(eq_result),
                        op: "^".to_string(),
                        rhs: Value::Constant(1),
                    });
                    return Value::Variable(result);
                }

                let result = self.gen_unique_variable("result");
                self.var_types.insert(result.clone(), ast::Type::Int);
                self.push_instruction(Primitive::BinOp {
                    dest: result.clone(),
                    lhs: left,
                    op: op.to_string(),
                    rhs: right,
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

                let alloc_size = 1 + metadata.field_count as i64;
                let obj_addr = self.gen_unique_variable("objAddr");
                self.var_types.insert(obj_addr.clone(), ast::Type::ClassType(class_name.clone()));

                self.push_instruction(Primitive::Alloc { 
                    dest: obj_addr.clone(), 
                    size: alloc_size, 
                });

                // no longer need to store field map
                self.push_instruction(Primitive::Store {
                    addr: Value::Variable(obj_addr.clone()),
                    val: Value::Global(format!("vtbl{}", class_name)),
                });

                Value::Variable(obj_addr)
            }

            // we can now do direct acces field reads, dont need to do all the crazy stuff we were doing befre
            /*
                ffld Read: &x.f  (where x is of type A, and f is at slot 1)

                BEFORE (untyped, with field map indirection):
                    %tag = %x & 1
                    if %tag then badptr else ok
                    ok:
                    %fieldMapAddr = %x + 8
                    %fieldMap = load(%fieldMapAddr)
                    %offset = getelt(%fieldMap, 0)
                    if %offset then load else badfield
                    load:
                    %result = getelt(%x, %offset)
                    jump final
                    badptr: fail NotAPointer
                    badfield: fail NoSuchField
                    final: ...

                AFTER (typed, direct slot access):
                    if %x then fieldOk else badptr
                    fieldOk:
                    %result = getelt(%x, 1)
                    jump final
                    badptr: fail NotAPointer
                    final: ...

                this is good optimization after type checijgn
            */
            Expression::FieldRead { base, field_name } => {
                let field_type = self.evaluate_type(expression);
                let base_val = self.gen_expression(base);
                
                let class_name = match self.evaluate_type(base) {
                    ast::Type::ClassType(n) => n,
                    _ => panic!("field read on non-class"),
                };

                let metadata = self.class_metadata_map.get(&class_name).unwrap();
                let slot = *metadata.field_map.get(field_name).unwrap();

                let bad_ptr = self.gen_unique_label("badptr");
                let ok_label = self.gen_unique_label("fieldOk");
                let final_label = self.gen_unique_label("final");

                self.finish_block(
                    ControlTransfer::Branch {
                        cond: base_val.clone(),
                        then_lab: ok_label.clone(),
                        else_lab: bad_ptr.clone(),
                    },
                    ok_label.clone(),
                );

                let result = self.gen_unique_variable("result");
                self.var_types.insert(result.clone(), field_type.clone());

                self.push_instruction(Primitive::GetElt {
                    dest: result.clone(),
                    arr: base_val,
                    idx: Value::Constant(slot as i64),
                });

                self.finish_block(
                    ControlTransfer::Jump { target: final_label.clone() },
                    bad_ptr,
                );
                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    final_label,
                );

                Value::Variable(result)
            }

            // similar opt here much shorter code no untagging, etc.
            Expression::FieldWrite { base, field_name, value } => {
                let base_val = self.gen_expression(base);
                let val = self.gen_expression(value);

                let class_name = match self.evaluate_type(base) {
                    ast::Type::ClassType(n) => n,
                    _ => panic!("field write on non-class"),
                };
                let metadata = self.class_metadata_map.get(&class_name).unwrap();
                let slot = *metadata.field_map.get(field_name).unwrap();

                let bad_ptr = self.gen_unique_label("badptr");
                let ok_label = self.gen_unique_label("fieldOk");
                let final_label = self.gen_unique_label("final");

                self.finish_block(
                    ControlTransfer::Branch {
                        cond: base_val.clone(),
                        then_lab: ok_label.clone(),
                        else_lab: bad_ptr.clone(),
                    },
                    ok_label.clone(),
                );

                self.push_instruction(Primitive::SetElt {
                    arr: base_val,
                    idx: Value::Constant(slot as i64),
                    val,
                });

                self.finish_block(
                    ControlTransfer::Jump { target: final_label.clone() },
                    bad_ptr,
                );
                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    final_label,
                );

                Value::Constant(0)
            }

            /*
            BEFORE: %tag = %x & 1 --> if %tag --> load vtable --> getelt --> if %methodPtr --> call
            AFTER:  if %x then ok else badptr --> load vtable --> getelt --> call (no method check)
            */
            Expression::MethodCall { base, method_name, args } => {
                let return_type = self.evaluate_type(expression);
                let base = self.gen_expression(base);

                let badptr = self.gen_unique_label("badptr");
                let ok_label = self.gen_unique_label("methodOk");
                let final_label = self.gen_unique_label("final");

                self.finish_block(
                    ControlTransfer::Branch {
                        cond: base.clone(),
                        then_lab: ok_label.clone(),
                        else_lab: badptr.clone(),
                    },
                    ok_label.clone(),
                );

                // load vtable
                let vtable = self.gen_unique_variable("vtable");
                self.push_instruction(Primitive::Load {
                    dest: vtable.clone(),
                    addr: base.clone(),
                });

                // lookup method - type checker guarantees it exists, no need to check
                let global_method_id = *self.global_method_ids.get(method_name)
                    .expect(&format!("Method {} not found", method_name));
                let method_ptr = self.gen_unique_variable("methodPtr");
                self.push_instruction(Primitive::GetElt {
                    dest: method_ptr.clone(),
                    arr: Value::Variable(vtable),
                    idx: Value::Constant(global_method_id as i64),
                });

                // can now call directly :D
                let result = self.gen_unique_variable("callResult");
                self.var_types.insert(result.clone(), return_type.clone());
                let arguments: Vec<Value> = args
                    .iter()
                    .map(|a| self.gen_expression(a))
                    .collect();

                self.push_instruction(Primitive::Call {
                    dest: result.clone(),
                    func: Value::Variable(method_ptr),
                    receiver: base,
                    args: arguments,
                });

                self.finish_block(
                    ControlTransfer::Jump { target: final_label.clone() },
                    badptr,
                );
                self.finish_block(
                    ControlTransfer::Fail { message: "NotAPointer".to_string() },
                    final_label,
                );

                Value::Variable(result)
            }

            Expression::Null(_) => {
                Value::Constant(0)
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

            // no more tagging needed
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

                let cond_var = match &condition {
                    Value::Variable(v) => v.clone(),
                    other => {
                        let tmp = self.gen_unique_variable("cond");
                        self.push_instruction(Primitive::Assign {
                            dest: tmp.clone(),
                            value: other.clone(),
                        });
                        tmp
                    }
                };

                let then_label = self.gen_unique_label("then");
                let else_label = self.gen_unique_label("else");
                let merge_label = self.gen_unique_label("merge");

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(cond_var),
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

                let cond_var = match &condition {
                    Value::Variable(v) => v.clone(),
                    other => {
                        let tmp = self.gen_unique_variable("cond");
                        self.push_instruction(Primitive::Assign {
                            dest: tmp.clone(),
                            value: other.clone(),
                        });
                        tmp
                    }
                };

                self.finish_block(
                    ControlTransfer::Branch { 
                        cond: Value::Variable(cond_var),
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
                let cond_label = self.gen_unique_label("condLabel");
                let body_label = self.gen_unique_label("whileBody");
                let merge_label = self.gen_unique_label("whileMerge");

                self.finish_block(
                    ControlTransfer::Jump { 
                        target: cond_label.clone() 
                    },
                    cond_label.clone(),
                );
                
                let condition = self.gen_expression(condition);

                let cond_var = match &condition {
                    Value::Variable(v) => v.clone(),
                    other => {
                        let tmp = self.gen_unique_variable("cond");
                        self.push_instruction(Primitive::Assign {
                            dest: tmp.clone(),
                            value: other.clone(),
                        });
                        tmp
                    }
                };

                self.finish_block(
                    ControlTransfer::Branch {
                        cond: Value::Variable(cond_var),
                        then_lab: body_label.clone(),
                        else_lab: merge_label.clone(),
                    },
                    body_label
                );

                for statement in body {
                    self.gen_statement(statement);
                }

                let while_control_transfer = 
                    if self.current_block_has_explicit_return {
                        self.current_block.control_transfer.clone()
                    } else {
                        ControlTransfer::Jump { target: cond_label }
                    };

                self.finish_block(
                    while_control_transfer,
                    merge_label,
                );
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
        self.type_environment.clear();
        self.type_environment.insert("this".to_string(), ast::Type::ClassType(class.name.clone()));
        for (arg, typ) in &method.args {
            self.type_environment.insert(arg.clone(), typ.clone());
        }
        for (local, typ) in &method.locals {
            self.type_environment.insert(local.clone(), typ.clone());
        }

        let function_name = format!("{}{}", method.name, class.name);

        let mut args = vec!["this".to_string()];
        for (arg, _) in &method.args {
            args.push(arg.clone());
        }

        // just build the basic blocks and push the function at the end of the statement evaluation
        self.current_block = BasicBlock { 
            label: function_name.clone(),
            primitives: vec![], 
            control_transfer: ControlTransfer::Return { val: Value::Constant(0) },
        };
        self.current_function_blocks = vec![];
        self.current_block_has_explicit_return = false;

        // initialize the locals to tagged 0s
        for (local, _) in &method.locals {
            self.push_instruction(Primitive::Assign {
                dest: local.clone(),
                value: Value::Constant(0),
            });
        }

        for statement in &method.body {
            self.gen_statement(statement);
        }

        self.finish_function(function_name, args);
    }

    pub fn gen_program(&mut self, program: &ast::Program) -> ir::Program {
        self.classes = program.classes.clone();
        self.gen_class_metadata(program);

        for class in &program.classes {
            for method in &class.methods {
                self.gen_method(class, method);
            }
        }

        self.type_environment.clear();
        for (local, typ) in &program.main_locals {
            self.type_environment.insert(local.clone(), typ.clone());
        }

        // generating main block
        self.current_block = BasicBlock {
            label: "main".to_string(),
            primitives: vec![],
            control_transfer: ControlTransfer::Return { val: Value::Constant(0) },
        };
        self.current_function_blocks = vec![];
        self.current_block_has_explicit_return = false;

        // must initialize main locals, just make them tagged 0
        for (local, _) in &program.main_locals {
            self.push_instruction(Primitive::Assign {
                dest: local.clone(),
                value: Value::Constant(0),
            });
        }

        for statement in &program.main_body {
            self.gen_statement(statement);
        }

        self.finish_function("main".to_string(), vec![]);

        ir::Program {
            globals: self.globals.clone(),
            functions: self.functions.clone(),
            var_types: self.var_types.clone(),
        }
    }
}