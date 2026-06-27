use std::collections::{HashMap, HashSet};

/// Определение нового типа
///
/// Пример:
/// ```text
/// тип МойТип
///     Вариант1
///     Вариант2(поле1, поле2)
/// ```
#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub variants: HashMap<String, Vec<String>>,
}

/// Определение новой процедуры
///
/// Процедуры могут принимать "выходные" параметры и не возвращают значения
///
/// Пример:
/// ```text
/// проц моя_процедура(параметр1, вых параметр2)
///     параметр2 := параметр1 + 1
///     возврат параметр2
/// ```
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    pub name: Option<String>, // spec allows anonymous procedures as values
    pub parameters: Vec<ProcParameter>,
    pub name_declarations: NameDeclarations,
    pub body: Vec<Spannable<Statement>>,
    // variables that need to be saved in the frame, so other procedures can access them via `чужие`
    pub variables_need_saving: HashSet<String>,
}

/// Определение новой функции
///
/// Функции обязаны возвращать значение
///
/// Пример:
/// ```text
/// функ моя_функция(параметр1)
///     возврат параметр1 + 1
/// ```
#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: Option<String>,
    pub parameters: Vec<String>, // functions only have input parameters (spec §1.5)
    pub name_declarations: NameDeclarations,
    pub body: Vec<Spannable<Statement>>,
    // variables that need to be saved in the frame, so other procedures can access them via `чужие`
    pub variables_need_saving: HashSet<String>,
}

/// Параметр процедуры
///
/// Выходные параметры передаются по ссылке
#[derive(Debug, Clone)]
pub enum ProcParameter {
    Input(String),
    InOut(String),
}

/// В теле функции или процедуры можно объявить чужие и свои имена,
/// где свои - локальные имена, необязательно объявлять их своими,
/// а чужие - имена, объявленные в другом месте, переданные неявно вызывающей стороной
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

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Пустое предписание (например `;` или пустая строка)
    Empty,

    /// variable := expression
    Assignment {
        target: Spannable<LValue>,
        value: Box<Spannable<Expr>>,
    },

    /// Вызов процедуры, в двух формах:
    /// ```text
    /// вызов моя_процедура(аргументы) \ с ключевым словом `вызов`
    /// моя_процедура(аргументы)
    /// ```
    ProcedureCall {
        procedure: Box<Spannable<Expr>>,
        arguments: Vec<CallArgument>,
    },

    /// Пример:
    /// ```text
    /// если число < 0 то
    ///     вывод: "Отрицательное число"
    /// иначе
    ///     вывод: "Положительное число"
    /// ```
    Conditional {
        condition: Box<Spannable<Expr>>,
        then_body: Vec<Spannable<Statement>>,
        else_body: Option<Vec<Spannable<Statement>>>,
    },

    /// Выбор при (pattern matching)
    ///
    /// см. [`SelectionStatement`]
    Selection(SelectionStatement),

    /// Циклы
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

    /// выход из цикла
    ExitLoop,

    /// возврат из процедуры
    ReturnFromProcedure,

    /// возврат из функции со значением
    ReturnFromFunction(Box<Spannable<Expr>>),

    /// Импортирование функций, типов или процедур из модуля
    /// подкл "<name>" (<definition_name>,...)
    Import {
        name: String,
        definitions: Vec<String>,
    },
}

/// Левая часть присваивания
#[derive(Debug, Clone, PartialEq)]
pub enum LValue {
    /// Просто имя:  X
    Name(String),
    /// Индексация:  X[i]
    Subscript {
        collection: Box<Spannable<Expr>>,
        index: Box<Spannable<Expr>>,
    },
    /// Отрезок:     X[a:b]  X[a:]  X[:b]  X[:]
    Slice {
        collection: Box<Spannable<Expr>>,
        from: Option<Box<Spannable<Expr>>>,
        to: Option<Box<Spannable<Expr>>>,
    },
    /// Поле типа:   X.field
    Field {
        left: Box<Spannable<Expr>>,
        field: String,
    },
}

/// Тип аргумента в вызове процедуры
#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    /// Стандартный входной аргумент, передаётся по значению
    Input(Box<Spannable<Expr>>),
    /// Выходной аргумент, передаётся по ссылке
    InOut(Spannable<LValue>),
}

/// Сравнение с образцом/pattern matching
///
/// Пример:
/// ```text
/// тип Сезон
///     Хороший
///     Плохой
///
/// сезон := Хороший
/// выбор сезон при
///     Хороший: вывод: "Хороший сезон"
///     Плохой: вывод: "Плохой сезон"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionStatement {
    ValueMatch {
        expression: Box<Spannable<Expr>>,
        cases: Vec<Spannable<ValueMatchCase>>,
        else_body: Option<Vec<Spannable<Statement>>>,
    },
}

/// Паттерн/образец для `выбор при`
#[derive(Debug, Clone, PartialEq)]
pub struct ValueMatchCase {
    pub values: Vec<Box<Spannable<Expr>>>,
    pub body: Vec<Spannable<Statement>>,
}

/// Формы цикла:
///
///   [для i [от a] [до b] [шаг c]] | [повтор n]
///   [пока f]
///   цикл body кц [по g]
#[derive(Debug, Clone, PartialEq)]
pub struct LoopStatement {
    pub header: LoopHeader,
    pub while_condition: Option<Box<Spannable<Expr>>>, // пока f
    pub body: Vec<Spannable<Statement>>,
    pub post_condition: Option<Box<Spannable<Expr>>>, // кц по g
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoopHeader {
    /// Просто цикл (без условия)
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

    /// Индексация
    /// k[a]
    Subscript {
        collection: Box<Spannable<Expr>>,
        index: Box<Spannable<Expr>>,
    },

    /// Отрезок (проекция кортежа)
    /// k[a:b]  k[a:]  k[:b]  k[:]
    Slice {
        collection: Box<Spannable<Expr>>,
        from: Option<Box<Spannable<Expr>>>,
        to: Option<Box<Spannable<Expr>>>,
    },

    /// Безтиповый кортеж
    /// (expr, ...)
    TupleConstruct(Vec<Box<Spannable<Expr>>>),

    /// Вызов функции
    /// f(expr, ...)
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

/// Binary operators in precedence order:
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
    Length, // #
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
