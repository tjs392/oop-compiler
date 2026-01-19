use crate::statement::Statement;

/*
This is for top level structures
 */
 
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub args: Vec<String>,
    pub locals: Vec<String>,
    pub body: Vec<Statement>
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub fields: Vec<String>,
    pub methods: Vec<Method>
}

#[derive(Debug, Clone)]
pub struct Program {
    pub classes: Vec<Class>,
    pub main_locals: Vec<String>,
    pub main_body: Vec<Statement>
}



