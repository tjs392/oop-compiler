use crate::expression::Expression;
use crate::statement::Statement;
use crate::ast;
use crate::ir::{self, BasicBlock, Primitive, Value, ControlTransfer, GlobalArray};

pub struct CodeGenerator {
    temp_counter: usize,

    block_counter: usize,

    current_block: BasicBlock,

    blocks: Vec<BasicBlock>,

    globals: Vec<GlobalArray>,
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
        }
    }

    fn gen_unique_temp_variable(&mut self) -> String {
        let name = format!("{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn gen_unique_label(&mut self) -> String {
        let label = format!("{}", self.block_counter);
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

                let result = self.gen_unique_temp_variable();

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

            Expression::ClassRef(_) => {
                todo!("handle class reference")
            }

            Expression::FieldRead { base, field_name } => {
                let base = self.gen_expression(base);
                todo!("handle field reads")
            }

            Expression::MethodCall { base, method_name, args } => {
                let base = self.gen_expression(base);
                todo!("handle field reads")
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
                todo!("implement field write");
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