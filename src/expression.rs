use crate::token::Operator;

#[derive(Debug, Clone)]
pub enum Expression {
    ThisExpr,
    Constant(i64),
    Binop {
        // here, Box is a heap pointer with size 8 bytes
        // Need this here for recursie types to avoid infinite size at compile time
        lhs: Box<Expression>,
        op: Operator,
        rhs: Box<Expression>,
    },
    MethodCall {
        base: Box<Expression>,
        method_name: String,
        args: Vec<Expression>,
    },
    FieldRead {
        base: Box<Expression>,
        field_name: String,
    },
    FieldWrite {
        base: Box<Expression>,
        field_name: String,
        value: Box<Expression>,
    },
    ClassRef(String),
    Variable(String),
}