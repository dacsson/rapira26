use std::collections::{HashMap, HashSet};

/// Top-level program: an ordered sequence of definitions and statements
/// (called "единица_общения_с_системой" in the spec §2.1).
#[derive(Debug, Clone)]
pub struct Program {
    pub units: Vec<ProgramUnit>,
}

#[derive(Debug, Clone)]
pub enum ProgramUnit {
    Statement(Spannable<Statement>),
    ProcedureDefinition(Spannable<ProcedureDefinition>),
    FunctionDefinition(Spannable<FunctionDefinition>),
    TypeDefinition(Spannable<TypeDefinition>),
}

/// тип NAME ;; NAME(params)+
#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub variants: HashMap<String, Vec<String>>,
}

/// проц NAME (params) ;; [name_decls] body
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    pub name: Option<String>, // spec allows anonymous procedures as values
    pub parameters: Vec<ProcParameter>,
    pub name_declarations: NameDeclarations,
    pub body: Vec<Spannable<Statement>>,
    // variables that need to be saved in the frame, so other procedures can access them via `чужие`
    pub variables_need_saving: HashSet<String>,
}

/// функ NAME (params) ;; [name_decls] body
#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: Option<String>,
    pub parameters: Vec<String>, // functions only have input parameters (spec §1.5)
    pub name_declarations: NameDeclarations,
    pub body: Vec<Spannable<Statement>>,
    // variables that need to be saved in the frame, so other procedures can access them via `чужие`
    pub variables_need_saving: HashSet<String>,
}

/// A single parameter in a procedure definition.
/// Input: =>NAME or just NAME.  In-out: <=NAME.
#[derive(Debug, Clone)]
pub enum ProcParameter {
    Input(String),
    InOut(String),
}

/// чужие: and свои: declarations inside a procedure or function body (spec §1.5).
/// Both lists default to empty when omitted.
#[derive(Debug, Clone)]
pub struct NameDeclarations {
    pub foreign_names: Vec<String>, // чужие
    pub own_names: Vec<String>,     // свои
}

impl NameDeclarations {
    pub fn empty() -> Self {
        Self {
            foreign_names: vec![],
            own_names: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    /// Empty statement (bare `;` or blank line)
    Empty,

    /// variable := expression
    Assignment {
        target: Spannable<LValue>,
        value: Box<Spannable<Expr>>,
    },

    /// вызов expr(args)  or  name(args)
    ProcedureCall {
        procedure: Box<Spannable<Expr>>,
        arguments: Vec<CallArgument>,
    },

    /// если condition то body [иначе body] все
    Conditional {
        condition: Box<Spannable<Expr>>,
        then_body: Vec<Spannable<Statement>>,
        else_body: Option<Vec<Spannable<Statement>>>,
    },

    /// выбор — two forms (spec §2.2.4.2 item 3)
    Selection(SelectionStatement),

    /// All loop variants unified under one node (spec §2.1 grammar)
    Loop(LoopStatement),

    /// вывод [бпс] [: expr, ...]
    Output {
        no_newline: bool,
        values: Vec<Box<Spannable<Expr>>>,
    },

    /// ввод [текста] : var, ...
    Input {
        text_mode: bool,
        variables: Vec<Spannable<LValue>>,
    },

    /// выход  — break out of the innermost loop
    ExitLoop,

    /// возврат  — return from procedure (no value)
    ReturnFromProcedure,

    /// возврат expr  — return value from function
    ReturnFromFunction(Box<Spannable<Expr>>),
}

/// Left-hand side of an assignment (spec §2.1 "переменная")
#[derive(Debug, Clone)]
pub enum LValue {
    /// Plain name:  X
    Name(String),
    /// Subscript:   X[i]
    Subscript {
        collection: Box<Spannable<Expr>>,
        index: Box<Spannable<Expr>>,
    },
    /// Slice:       X[a:b]  X[a:]  X[:b]  X[:]
    Slice {
        collection: Box<Spannable<Expr>>,
        from: Option<Box<Spannable<Expr>>>,
        to: Option<Box<Spannable<Expr>>>,
    },
    /// Field:       X.field
    Field {
        left: Box<Spannable<Expr>>,
        field: String,
    },
}

/// Argument in a procedure call (spec §2.1 "факт_парам_проц")
#[derive(Debug, Clone)]
pub enum CallArgument {
    /// =>expr  or just  expr  — input argument
    Input(Box<Spannable<Expr>>),
    /// <=variable  — in-out (return) argument
    InOut(Spannable<LValue>),
}

#[derive(Debug, Clone)]
pub enum SelectionStatement {
    /// выбор expr при v1,v2: body ... [иначе body]
    ValueMatch {
        expression: Box<Spannable<Expr>>,
        cases: Vec<Spannable<ValueMatchCase>>,
        else_body: Option<Vec<Spannable<Statement>>>,
    },
}

#[derive(Debug, Clone)]
pub struct ValueMatchCase {
    pub values: Vec<Box<Spannable<Expr>>>,
    pub body: Vec<Spannable<Statement>>,
}

#[derive(Debug, Clone)]
pub struct ConditionCase {
    pub condition: Box<Spannable<Expr>>,
    pub body: Vec<Spannable<Statement>>,
}

/// All loop forms share optional pre/post conditions (spec §2.1 "цикл").
///
///   [для i [от a] [до b] [шаг c]] | [повтор n]
///   [пока f]
///   цикл body кц [по g]
#[derive(Debug, Clone)]
pub struct LoopStatement {
    pub header: LoopHeader,
    pub while_condition: Option<Box<Spannable<Expr>>>, // пока f
    pub body: Vec<Spannable<Statement>>,
    pub post_condition: Option<Box<Spannable<Expr>>>, // кц по g
}

#[derive(Debug, Clone)]
pub enum LoopHeader {
    /// Plain цикл (infinite loop, or with пока/кц по)
    Infinite,
    /// повтор N
    Repeat(Box<Spannable<Expr>>),
    /// для i [от a] [до b] [шаг c]
    For {
        variable: String,
        from: Option<Box<Spannable<Expr>>>,
        to: Option<Box<Spannable<Expr>>>,
        step: Option<Box<Spannable<Expr>>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Name(String),

    /// k[a]
    Subscript {
        collection: Box<Spannable<Expr>>,
        index: Box<Spannable<Expr>>,
    },

    /// k[a:b]  k[a:]  k[:b]  k[:]
    Slice {
        collection: Box<Spannable<Expr>>,
        from: Option<Box<Spannable<Expr>>>,
        to: Option<Box<Spannable<Expr>>>,
    },

    /// <* expr, ... *>
    TupleConstruct(Vec<Box<Spannable<Expr>>>),

    /// f(expr, ...)  — function call (only input args in expression position)
    FunctionCall {
        function: Box<Spannable<Expr>>,
        arguments: Vec<Box<Spannable<Expr>>>,
    },

    BinaryOp {
        operator: BinaryOperator,
        left: Box<Spannable<Expr>>,
        right: Box<Spannable<Expr>>,
    },

    UnaryOp {
        operator: UnaryOperator,
        operand: Box<Spannable<Expr>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,          // пусто
    Boolean(bool), // да / нет
    Integer(i64),
    Real(f64),
    Text(String),
}

/// Binary operators in precedence order (spec §2.2.3, highest to lowest):
/// ** > * / // /% > + - > > < >= <= > = /= > и > или
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Power,          // **
    Multiply,       // *
    Divide,         // /
    IntegerDivide,  // //
    Remainder,      // /%
    Add,            // +
    Subtract,       // -
    Greater,        // >
    Less,           // <
    GreaterOrEqual, // >=
    LessOrEqual,    // <=
    Equal,          // =
    NotEqual,       // /=
    And,            // и
    Or,             // или
    Dot,            // .
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Negate, // -
    Plus,   // +
    Not,    // не
    Length, // #  (spec level 2, above **)
}

/// A node with span info
#[derive(Debug, Clone, PartialEq)]
pub struct Spannable<T> {
    pub node: T,
    pub position_start: usize,
    pub position_end: usize,
}

impl<T> Spannable<T> {
    pub fn new(node: T, pos: (usize, usize)) -> Self {
        Self {
            node,
            position_start: pos.0,
            position_end: pos.1,
        }
    }
}
