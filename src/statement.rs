use crate::expression::Expression;

#[derive(Debug, Clone)]
pub enum Statement {
    
    // x = e for any variable x and expression e
    Assignment {
        variable: String,
        expression: Expression
    },

    // _ = e for any e. This is used if you don’t care about the expression’s result, 
    // but are just running it for side effects (e.g., printing from a method called in e)
    Discard(Expression),

    // !e.f = e for field update
    FieldWrite {
        base: Expression,
        field: String,
        value: Expression
    },

    // if e: { <newline> <one or more statements> } else { <newline> <one or more statements> }
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>
    },

    // ifonly e: { <newline> <one or more statements> }
    IfOnly {
        condition: Expression,
        body: Vec<Statement>
    },

    // while e: { <newline> <one or more statements> }
    While {
        condition: Expression,
        body: Vec<Statement>
    },

    // return e
    Return(Expression),

    // print(e)
    Print(Expression)

}