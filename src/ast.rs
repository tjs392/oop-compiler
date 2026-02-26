use crate::statement::Statement;

/*
This is for top level structures
 */
 
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub args: Vec<(String, Type)>,
    pub locals: Vec<(String, Type)>,
    pub body: Vec<Statement>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub methods: Vec<Method>
}

#[derive(Debug, Clone)]
pub struct Program {
    pub classes: Vec<Class>,
    pub main_locals: Vec<(String, Type)>,
    pub main_body: Vec<Statement>
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    ClassType(String),
}



