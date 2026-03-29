use std::collections::HashSet;

/// Top-level program: an ordered sequence of definitions and statements
/// (called "единица_общения_с_системой" in the spec §2.1).
#[derive(Debug, Clone)]
pub struct Program {
    pub units: Vec<ProgramUnit>,
}

#[derive(Debug, Clone)]
pub enum ProgramUnit {
    Statement(Statement),
    ProcedureDefinition(ProcedureDefinition),
    FunctionDefinition(FunctionDefinition),
}

/// проц NAME (params) ;; [name_decls] body конец
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    pub name: Option<String>, // spec allows anonymous procedures as values
    pub parameters: Vec<ProcParameter>,
    pub name_declarations: NameDeclarations,
    pub body: Vec<Statement>,
    // variables that need to be saved in the frame, so other procedures can access them via `чужие`
    pub variables_need_saving: HashSet<String>,
}

/// функ NAME (params) ;; [name_decls] body конец
#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: Option<String>,
    pub parameters: Vec<String>, // functions only have input parameters (spec §1.5)
    pub name_declarations: NameDeclarations,
    pub body: Vec<Statement>,
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
    Assignment { target: LValue, value: Box<Expr> },

    /// вызов expr(args)  or  name(args)
    ProcedureCall {
        procedure: Box<Expr>,
        arguments: Vec<CallArgument>,
    },

    /// если condition то body [иначе body] все
    Conditional {
        condition: Box<Expr>,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },

    /// выбор — two forms (spec §2.2.4.2 item 3)
    Selection(SelectionStatement),

    /// All loop variants unified under one node (spec §2.1 grammar)
    Loop(LoopStatement),

    /// вывод [бпс] [: expr, ...]
    Output {
        no_newline: bool,
        values: Vec<Box<Expr>>,
    },

    /// ввод [текста] : var, ...
    Input {
        text_mode: bool,
        variables: Vec<LValue>,
    },

    /// выход  — break out of the innermost loop
    ExitLoop,

    /// возврат  — return from procedure (no value)
    ReturnFromProcedure,

    /// возврат expr  — return value from function
    ReturnFromFunction(Box<Expr>),
}

/// Left-hand side of an assignment (spec §2.1 "переменная")
#[derive(Debug, Clone)]
pub enum LValue {
    /// Plain name:  X
    Name(String),
    /// Subscript:   X[i]
    Subscript {
        collection: Box<Expr>,
        index: Box<Expr>,
    },
    /// Slice:       X[a:b]  X[a:]  X[:b]  X[:]
    Slice {
        collection: Box<Expr>,
        from: Option<Box<Expr>>,
        to: Option<Box<Expr>>,
    },
}

/// Argument in a procedure call (spec §2.1 "факт_парам_проц")
#[derive(Debug, Clone)]
pub enum CallArgument {
    /// =>expr  or just  expr  — input argument
    Input(Box<Expr>),
    /// <=variable  — in-out (return) argument
    InOut(LValue),
}

#[derive(Debug, Clone)]
pub enum SelectionStatement {
    /// выбор expr при v1,v2: body ... [иначе body] все
    ValueMatch {
        expression: Box<Expr>,
        cases: Vec<ValueMatchCase>,
        else_body: Option<Vec<Statement>>,
    },
    /// выбор при cond: body ... [иначе body] все
    ConditionList {
        cases: Vec<ConditionCase>,
        else_body: Option<Vec<Statement>>,
    },
}

#[derive(Debug, Clone)]
pub struct ValueMatchCase {
    pub values: Vec<Box<Expr>>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct ConditionCase {
    pub condition: Box<Expr>,
    pub body: Vec<Statement>,
}

/// All loop forms share optional pre/post conditions (spec §2.1 "цикл").
///
///   [для i [от a] [до b] [шаг c]] | [повтор n]
///   [пока f]
///   цикл body кц [по g]
#[derive(Debug, Clone)]
pub struct LoopStatement {
    pub header: LoopHeader,
    pub while_condition: Option<Box<Expr>>, // пока f
    pub body: Vec<Statement>,
    pub post_condition: Option<Box<Expr>>, // кц по g
}

#[derive(Debug, Clone)]
pub enum LoopHeader {
    /// Plain цикл (infinite loop, or with пока/кц по)
    Infinite,
    /// повтор N
    Repeat(Box<Expr>),
    /// для i [от a] [до b] [шаг c]
    For {
        variable: String,
        from: Option<Box<Expr>>,
        to: Option<Box<Expr>>,
        step: Option<Box<Expr>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Name(String),

    /// k[a]
    Subscript {
        collection: Box<Expr>,
        index: Box<Expr>,
    },

    /// k[a:b]  k[a:]  k[:b]  k[:]
    Slice {
        collection: Box<Expr>,
        from: Option<Box<Expr>>,
        to: Option<Box<Expr>>,
    },

    /// <* expr, ... *>
    TupleConstruct(Vec<Box<Expr>>),

    /// f(expr, ...)  — function call (only input args in expression position)
    FunctionCall {
        function: Box<Expr>,
        arguments: Vec<Box<Expr>>,
    },

    BinaryOp {
        operator: BinaryOperator,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        operator: UnaryOperator,
        operand: Box<Expr>,
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Negate, // -
    Plus,   // +
    Not,    // не
    Length, // #  (spec level 2, above **)
}
