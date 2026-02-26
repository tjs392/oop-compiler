use std::collections::HashMap;
use crate::ast::{Program, Class, Type};
use crate::expression::Expression;
use crate::statement::Statement;
use crate::token::Operator;

pub struct TypeChecker {
    // class name -> class def for field/method lookup for checking type compatibility
    classes: HashMap<String, Class>,
}

impl TypeChecker {
    pub fn new(program: &Program) -> Self {
        let mut classes = HashMap::new();
        for class  in &program.classes {
            classes.insert(class.name.clone(), class.clone());
        }
        TypeChecker { classes }
    }

    fn validate_type(&self, typ: &Type) {
        if let Type::ClassType(name) = typ {
            if !self.classes.contains_key(name) {
                panic!("Unknown class {}", name);
            }
        }
    }

    pub fn check_program(&self, program: &Program) {
        // check all type exist
        for class in &program.classes {
            for (_, typ) in &class.fields {
                self.validate_type(typ);
            }

            // check the return types, arguments, and locals types for each method
            for method in &class.methods {
                self.validate_type(&method.return_type);
                for (_, typ) in &method.args {
                    self.validate_type(typ);
                }
                
                for (_, typ) in &method.locals {
                    self.validate_type(typ);
                }
            }
        }

        // check main locals types exist
        for (_, typ) in &program.main_locals {
            self.validate_type(typ);
        }

        for class in &program.classes {
            for method in &class.methods {
                let mut env = HashMap::new();
                env.insert("this".to_string(), Type::ClassType(class.name.clone()));
                for (name, typ) in &method.args {
                    env.insert(name.clone(), typ.clone());
                }

                for (name, typ) in &method.locals {
                    env.insert(name.clone(), typ.clone());
                }

                for statement in &method.body {
                    self.check_statement(statement, &env, &method.return_type);
                }
            }
        }

        let mut env = HashMap::new();
        for (name, typ) in &program.main_locals {
            env.insert(name.clone(), typ.clone());
        }

        for statement in &program.main_body {
            self.check_statement(statement, &env, &Type::Int);
        }
    }

    fn eval_type(&self, expr: &Expression, env: &HashMap<String, Type>) -> Type {
        match expr {
            Expression::Constant(_) => Type::Int,

            Expression::Variable(name) => {
                env.get(name).unwrap_or_else(|| panic!("Undefined variable {}", name)).clone()
            }

            Expression::ThisExpr => {
                env.get("this").expect("this used outside of method").clone()
            }

            Expression::Null(class_name) => {
                let typ = Type::ClassType(class_name.clone());
                self.validate_type(&typ);
                typ
            }

            Expression::Binop { op, lhs, rhs } => {
                let ltyp = self.eval_type(lhs, env);
                let rtyp = self.eval_type(rhs, env);
                match op {
                    Operator::Equals | Operator::NotEquals => {
                        match (&ltyp, &rtyp) {
                            (Type::Int, Type::Int) => Type::Int,
                            (Type::ClassType(a), Type::ClassType(b)) if a == b => Type::Int,
                            _ => panic!("Equality operands must have matching types"),
                        }
                    }
                    _ => {
                        if ltyp != Type::Int || rtyp != Type::Int {
                            panic!("Binary op requires ints");
                        }
                        Type::Int
                    }
                }
            }

            Expression::ClassRef(name) => {
                if !self.classes.contains_key(name) {
                    panic!("Unknown class of {}", name);
                }
                Type::ClassType(name.clone())
            }

            Expression::FieldRead { base, field_name } => {
                let base_type = self.eval_type(base, env);
                match &base_type {
                    Type::ClassType(class_name) => {
                        let class = self.classes.get(class_name)
                            .unwrap_or_else(|| panic!("Unknown class {}", class_name));
                        for (fname, ftyp) in &class.fields {
                            if fname == field_name {
                                return ftyp.clone();
                            }
                        }
                        panic!("Class {} has no field {}", class_name, field_name);
                    }
                    Type::Int => panic!("Cant read field of int"),
                }
            }

            Expression::MethodCall { base, method_name, args } => {
                let base_type = self.eval_type(base, env);
                match &base_type {
                    Type::ClassType(class_name) => {
                        let class = self.classes.get(class_name).unwrap();
                        let method = class.methods.iter()
                            .find(|m| m.name == *method_name)
                            .unwrap_or_else(|| panic!("the claslass {} has no method {}", class_name, method_name));

                        if args.len() != method.args.len() {
                            panic!("Incorrect number of args for {}.{}", class_name, method_name);
                        }
                        for (arg_expr, (_, expected_type)) in args.iter().zip(method.args.iter()) {
                            let actual = self.eval_type(arg_expr, env);
                            if actual != *expected_type {
                                panic!("Arg type mismatch in {}.{}", class_name, method_name);
                            }
                        }

                        method.return_type.clone()
                    }
                    Type::Int => panic!("Cannot call method on int"),
                }
            }

            Expression::FieldWrite { base, field_name, value } => {
                // this is the same as field read but also check value type
                let base_type = self.eval_type(base, env);
                match &base_type {
                    Type::ClassType(class_name) => {
                        let class = self.classes.get(class_name).unwrap();
                        let field_type = class.fields.iter()
                            .find(|(n, _)| n == field_name)
                            .map(|(_, t)| t)
                            .unwrap_or_else(|| panic!("No field exists: {}", field_name));
                        let val_type = self.eval_type(value, env);
                        if val_type != *field_type {
                            panic!("Field write type mismatch");
                        }
                        val_type
                    }
                    Type::Int => panic!("Cannot write field of int"),
                }
            }
        }
    }

    fn check_statement(&self, statement: &Statement, env: &HashMap<String, Type>, return_type: &Type) {
        match statement {
            /*
            print is well-typed if its argument is an int
            if, ifonly, and while are well-typed if their bodies/branches are well-typed, and the condition has type int.
            A variable assignment is well-typed if the type of the expression matches the type of the variable.
            Field updates are similar: find the type of the field being updated, and check that the expression being stored there has the same type 
            */
            
            Statement::Print(expr) => {
                if self.eval_type(expr, env) != Type::Int {
                    panic!("print requires int");
                }
            }
            
            
            Statement::Return(expr) => {
                let t = self.eval_type(expr, env);
                if t != *return_type {
                    panic!("Return type mismatch");
                }
            }

            // just match the expression and var type
            Statement::Assignment { variable, expression } => {
                let var_type = env.get(variable).unwrap_or_else(|| panic!("Undefined: {}", variable));
                let expr_type = self.eval_type(expression, env);
                if expr_type != *var_type {
                    panic!("Assignment type mismatch for {}", variable);
                }
            }

            // straight forward
            Statement::FieldWrite { base, field, value } => {
                let expr = Expression::FieldWrite {
                    base: Box::new(base.clone()),
                    field_name: field.clone(),
                    value: Box::new(value.clone()),
                };
                self.eval_type(&expr, env);
            }

            Statement::Discard(expr) => {
                self.eval_type(expr, env);
            }

            Statement::If { condition, then_body, else_body } => {
                if self.eval_type(condition, env) != Type::Int {
                    panic!("'If' condition must be int");
                }
                for s in then_body { self.check_statement(s, env, return_type); }
                for s in else_body { self.check_statement(s, env, return_type); }
            }

            Statement::IfOnly { condition, body } => {
                if self.eval_type(condition, env) != Type::Int {
                    panic!("'IfOnly' condition must be int");
                }
                for s in body { self.check_statement(s, env, return_type); }
            }

            Statement::While { condition, body } => {
                if self.eval_type(condition, env) != Type::Int {
                    panic!("'While' condition must be a int");
                }
                for s in body { self.check_statement(s, env, return_type); }
            }
        }
    }
}

