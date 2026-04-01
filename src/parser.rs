//! WARNING: this parser is 90% written by AI
//! Recursive descent parser for the Rapira language (spec Препринт 767).
//!
//! Uses indentation-based block structure: Indent/Dedent tokens from the lexer
//! replace the old block terminators (конец, все, кц).
//!
//! Operator precedence (spec §2.2.3, lowest → highest):
//!   или → и → не → = /= → > < >= <= → + - → * / // /% → ** → unary(- +) → # → postfix
//!
//! The `возврат` ambiguity (bare `возврат` vs `возврат expr`) is resolved greedily:
//! if the next token can start an expression, we parse an expression.

use std::collections::HashSet;

use crate::ast::*;
use crate::lexer::{Lexer, LexerError, Token};

#[derive(Debug)]
pub enum ParseError {
    LexerError(LexerError),
    UnexpectedToken {
        position_start: usize,
        position_end: usize,
        found: Token,
        expected: String,
    },
    UnexpectedEof {
        expected: String,
    },
}

pub struct Parser<'input> {
    lexer: Lexer<'input>,
    current: Option<(usize, Token, usize)>,
}

impl<'input> Parser<'input> {
    pub fn new(mut lexer: Lexer<'input>) -> Self {
        let current = Self::advance_lexer(&mut lexer);
        Self { lexer, current }
    }

    fn advance_lexer(lexer: &mut Lexer<'input>) -> Option<(usize, Token, usize)> {
        loop {
            match lexer.next() {
                None => return None,
                Some(Ok(triple)) => return Some(triple),
                Some(Err(_error)) => continue,
            }
        }
    }

    fn advance(&mut self) -> Option<(usize, Token, usize)> {
        let previous = self.current.take();
        self.current = Self::advance_lexer(&mut self.lexer);
        previous
    }

    fn peek(&self) -> Option<&Token> {
        self.current.as_ref().map(|(_, token, _)| token)
    }

    fn positions(&self) -> (usize, usize) {
        self.current
            .as_ref()
            .map(|(st, _, end)| (*st, *end))
            .unwrap_or((0, 0))
    }

    fn eat(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(usize, Token, usize), ParseError> {
        if self.peek() == Some(expected) {
            Ok(self.advance().unwrap())
        } else {
            match &self.current {
                Some((position_start, found, position_end)) => Err(ParseError::UnexpectedToken {
                    position_start: *position_start,
                    position_end: *position_end,
                    found: found.clone(),
                    expected: format!("{expected}"),
                }),
                None => Err(ParseError::UnexpectedEof {
                    expected: format!("{expected}"),
                }),
            }
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token::Ident(_)) => {
                let (_, token, _) = self.advance().unwrap();
                match token {
                    Token::Ident(name) => Ok(name),
                    _ => unreachable!(),
                }
            }
            Some(_) => {
                let (position_start, found, position_end) = self.current.as_ref().unwrap();
                Err(ParseError::UnexpectedToken {
                    position_start: *position_start,
                    position_end: *position_end,
                    found: found.clone(),
                    expected: "индентификатор".to_string(),
                })
            }
            None => Err(ParseError::UnexpectedEof {
                expected: "identifier".to_string(),
            }),
        }
    }

    fn skip_newlines(&mut self) {
        while self.eat(&Token::Newline) {}
    }

    fn can_start_expression(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Token::Ident(_)
                    | Token::Integer(_)
                    | Token::Real(_)
                    | Token::Text(_)
                    | Token::KwПусто
                    | Token::KwДа
                    | Token::KwНет
                    | Token::KwПс
                    | Token::KwПи
                    | Token::KwPi
                    | Token::LParen
                    | Token::TupleOpen
                    | Token::Minus
                    | Token::Plus
                    | Token::Hash
                    | Token::KwНе
            )
        )
    }

    fn can_start_statement(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Token::Ident(_)
                    | Token::KwВызов
                    | Token::KwЕсли
                    | Token::KwВыбор
                    | Token::KwДля
                    | Token::KwПока
                    | Token::KwПовтор
                    | Token::KwЦикл
                    | Token::KwВывод
                    | Token::KwВвод
                    | Token::KwВыход
                    | Token::KwВозврат
            )
        )
    }

    /// Parse an indented block: Newline Indent stmt* Dedent
    fn parse_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        self.expect(&Token::Newline)?;
        self.expect(&Token::Indent)?;
        let statements = self.parse_statement_list_until(&Token::Dedent)?;
        self.expect(&Token::Dedent)?;
        Ok(statements)
    }

    /// Parse either a block (Newline + Indent...Dedent) or a single statement
    /// on the same line. Used after block openers like `то`, `цикл`, `иначе`.
    fn parse_block_or_single_statement(&mut self) -> Result<Vec<Statement>, ParseError> {
        if self.peek() == Some(&Token::Newline) {
            self.parse_block()
        } else {
            let statement = self.parse_statement()?;
            Ok(vec![statement])
        }
    }

    pub fn parse_program(mut self) -> Result<Program, ParseError> {
        let mut units = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek().is_none() {
                break;
            }
            units.push(self.parse_program_unit()?);
        }
        Ok(Program { units })
    }

    fn parse_program_unit(&mut self) -> Result<ProgramUnit, ParseError> {
        match self.peek() {
            Some(Token::KwПроц) => Ok(ProgramUnit::ProcedureDefinition(
                self.parse_procedure_definition()?,
            )),
            Some(Token::KwФунк) => Ok(ProgramUnit::FunctionDefinition(
                self.parse_function_definition()?,
            )),
            _ => Ok(ProgramUnit::Statement(self.parse_statement()?)),
        }
    }

    fn parse_procedure_definition(&mut self) -> Result<ProcedureDefinition, ParseError> {
        self.expect(&Token::KwПроц)?;

        let name = if matches!(self.peek(), Some(Token::Ident(_))) {
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(&Token::LParen)?;
        let parameters = self.parse_proc_parameter_list()?;
        self.expect(&Token::RParen)?;

        // Body is an indented block
        self.expect(&Token::Newline)?;
        self.expect(&Token::Indent)?;

        let name_declarations = self.parse_name_declarations()?;
        let body = self.parse_statement_list_until(&Token::Dedent)?;
        self.expect(&Token::Dedent)?;

        Ok(ProcedureDefinition {
            name,
            parameters,
            name_declarations,
            body,
            variables_need_saving: HashSet::new(),
        })
    }

    fn parse_function_definition(&mut self) -> Result<FunctionDefinition, ParseError> {
        self.expect(&Token::KwФунк)?;

        let name = if matches!(self.peek(), Some(Token::Ident(_))) {
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(&Token::LParen)?;
        let parameters = self.parse_func_parameter_list()?;
        self.expect(&Token::RParen)?;

        // Body is an indented block
        self.expect(&Token::Newline)?;
        self.expect(&Token::Indent)?;

        let name_declarations = self.parse_name_declarations()?;
        let body = self.parse_statement_list_until(&Token::Dedent)?;
        self.expect(&Token::Dedent)?;

        Ok(FunctionDefinition {
            name,
            parameters,
            name_declarations,
            body,
            variables_need_saving: HashSet::new(),
        })
    }

    fn parse_proc_parameter_list(&mut self) -> Result<Vec<ProcParameter>, ParseError> {
        let mut parameters = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(parameters);
        }

        parameters.push(self.parse_proc_parameter()?);
        while self.eat(&Token::Comma) {
            parameters.push(self.parse_proc_parameter()?);
        }
        Ok(parameters)
    }

    fn parse_proc_parameter(&mut self) -> Result<ProcParameter, ParseError> {
        if self.eat(&Token::LessOrEqual) {
            let name = self.expect_ident()?;
            Ok(ProcParameter::InOut(name))
        } else if self.eat(&Token::InputArrow) {
            let name = self.expect_ident()?;
            Ok(ProcParameter::Input(name))
        } else {
            let name = self.expect_ident()?;
            Ok(ProcParameter::Input(name))
        }
    }

    fn parse_func_parameter_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut parameters = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(parameters);
        }

        parameters.push(self.expect_ident()?);
        while self.eat(&Token::Comma) {
            parameters.push(self.expect_ident()?);
        }
        Ok(parameters)
    }

    fn parse_name_declarations(&mut self) -> Result<NameDeclarations, ParseError> {
        let mut foreign_names = Vec::new();
        let mut own_names = Vec::new();

        // Both чужие and свои can appear in any order
        for _ in 0..2 {
            self.skip_newlines();
            if self.eat(&Token::KwЧужие) {
                self.expect(&Token::Colon)?;
                foreign_names = self.parse_ident_list()?;
            } else if self.eat(&Token::KwСвои) {
                self.expect(&Token::Colon)?;
                own_names = self.parse_ident_list()?;
            }
        }

        Ok(NameDeclarations {
            foreign_names,
            own_names,
        })
    }

    fn parse_ident_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut names = vec![self.expect_ident()?];
        while self.eat(&Token::Comma) {
            names.push(self.expect_ident()?);
        }
        Ok(names)
    }

    /// Parse statements until we see `terminator` token (without consuming it).
    /// Newlines between statements are consumed.
    fn parse_statement_list_until(
        &mut self,
        terminator: &Token,
    ) -> Result<Vec<Statement>, ParseError> {
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(terminator) {
                break;
            }
            if !self.can_start_statement() {
                break;
            }
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(Token::KwВывод) => self.parse_output_statement(),
            Some(Token::KwВвод) => self.parse_input_statement(),
            Some(Token::KwЕсли) => self.parse_conditional(),
            Some(Token::KwВыбор) => self.parse_selection(),
            Some(Token::KwДля)
            | Some(Token::KwПока)
            | Some(Token::KwПовтор)
            | Some(Token::KwЦикл) => self.parse_loop(),
            Some(Token::KwВыход) => {
                self.advance();
                Ok(Statement::ExitLoop)
            }
            Some(Token::KwВозврат) => self.parse_return(),
            Some(Token::KwВызов) => self.parse_procedure_call_with_keyword(),
            Some(Token::Ident(_)) => self.parse_ident_statement(),
            Some(_) => {
                let (position_start, found, position_end) = self.current.as_ref().unwrap();
                Err(ParseError::UnexpectedToken {
                    position_start: *position_start,
                    position_end: *position_end,
                    found: found.clone(),
                    expected: "утверждение (функцию, процедуру, объявление переменной...)"
                        .to_string(),
                })
            }
            None => Err(ParseError::UnexpectedEof {
                expected: "statement".to_string(),
            }),
        }
    }

    /// Parse a statement starting with an identifier: assignment or procedure call.
    fn parse_ident_statement(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;

        match self.peek() {
            // NAME(args) — procedure call by name
            Some(Token::LParen) => {
                self.advance(); // consume (
                let arguments = self.parse_call_argument_list()?;
                self.expect(&Token::RParen)?;
                Ok(Statement::ProcedureCall {
                    procedure: Box::new(Expr::Name(name)),
                    arguments,
                })
            }
            // NAME := expr — simple assignment
            Some(Token::Assign) => {
                self.advance();
                let value = self.parse_expression()?;
                Ok(Statement::Assignment {
                    target: LValue::Name(name),
                    value: Box::new(value),
                })
            }
            // NAME[...] := expr — subscript/slice assignment
            Some(Token::LBracket) => {
                self.advance(); // consume [
                let target = self.parse_lvalue_subscript_or_slice(name)?;
                self.expect(&Token::Assign)?;
                let value = self.parse_expression()?;
                Ok(Statement::Assignment {
                    target,
                    value: Box::new(value),
                })
            }
            other => {
                let expected = "':=', '(', или '['".to_string();
                match other {
                    Some(_) => {
                        let (position_start, found, position_end) = self.current.as_ref().unwrap();
                        Err(ParseError::UnexpectedToken {
                            position_start: *position_start,
                            position_end: *position_end,
                            found: found.clone(),
                            expected,
                        })
                    }
                    None => Err(ParseError::UnexpectedEof { expected }),
                }
            }
        }
    }

    /// After consuming `NAME [`, parse subscript or slice target.
    fn parse_lvalue_subscript_or_slice(&mut self, name: String) -> Result<LValue, ParseError> {
        let collection = Box::new(Expr::Name(name));

        // Check for [:...] (slice with no `from`)
        if self.eat(&Token::Colon) {
            let to = if self.peek() != Some(&Token::RBracket) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&Token::RBracket)?;
            return Ok(LValue::Slice {
                collection,
                from: None,
                to,
            });
        }

        let first_expr = self.parse_expression()?;

        if self.eat(&Token::Colon) {
            // Slice: NAME[from:to]
            let to = if self.peek() != Some(&Token::RBracket) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&Token::RBracket)?;
            Ok(LValue::Slice {
                collection,
                from: Some(Box::new(first_expr)),
                to,
            })
        } else {
            // Subscript: NAME[index]
            self.expect(&Token::RBracket)?;
            Ok(LValue::Subscript {
                collection,
                index: Box::new(first_expr),
            })
        }
    }

    fn parse_output_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwВывод)?;
        let no_newline = self.eat(&Token::KwБпс);
        self.expect(&Token::Colon)?;

        let mut values = Vec::new();
        if self.can_start_expression() {
            values.push(Box::new(self.parse_expression()?));
            while self.eat(&Token::Comma) {
                values.push(Box::new(self.parse_expression()?));
            }
        }

        Ok(Statement::Output { no_newline, values })
    }

    fn parse_input_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwВвод)?;
        let text_mode = self.eat(&Token::KwТекста);
        self.expect(&Token::Colon)?;

        let mut variables = vec![self.parse_lvalue()?];
        while self.eat(&Token::Comma) {
            variables.push(self.parse_lvalue()?);
        }

        Ok(Statement::Input {
            text_mode,
            variables,
        })
    }

    fn parse_lvalue(&mut self) -> Result<LValue, ParseError> {
        let name = self.expect_ident()?;

        if self.eat(&Token::LBracket) {
            self.parse_lvalue_subscript_or_slice(name)
        } else {
            Ok(LValue::Name(name))
        }
    }

    fn parse_conditional(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwЕсли)?;
        let condition = Box::new(self.parse_expression()?);
        self.expect(&Token::KwТо)?;

        // Block or single-line then-body
        let then_body = self.parse_block_or_single_statement()?;

        // Skip newlines before checking for иначе (handles single-line then on separate line from иначе)
        self.skip_newlines();

        // Optional else branch — comes after the Dedent (at same indent as если)
        let else_body = if self.eat(&Token::KwИначе) {
            Some(self.parse_block_or_single_statement()?)
        } else {
            None
        };

        Ok(Statement::Conditional {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_selection(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwВыбор)?;

        // Check if this is form 2 (condition list): выбор followed by Newline+Indent+при
        // vs form 1 (value match): выбор EXPR ...
        if self.peek() == Some(&Token::Newline) {
            // Could be either form — need to look inside the block
            self.expect(&Token::Newline)?;
            self.expect(&Token::Indent)?;
            self.skip_newlines();

            if self.peek() == Some(&Token::KwПри) {
                // Form 2: condition list (no expression before при)
                return self.parse_selection_condition_list_in_block();
            }

            // Shouldn't happen — выбор block should start with при
            return Err(ParseError::UnexpectedToken {
                position_end: self.positions().0,
                position_start: self.positions().1,
                found: self.peek().cloned().unwrap_or(Token::Newline),
                expected: "при".to_string(),
            });
        }

        // Form 1: выбор EXPR — expression before при
        let expression = Box::new(self.parse_expression()?);
        self.expect(&Token::Newline)?;
        self.expect(&Token::Indent)?;
        self.skip_newlines();
        self.parse_selection_value_match_in_block(expression)
    }

    fn parse_selection_value_match_in_block(
        &mut self,
        expression: Box<Expr>,
    ) -> Result<Statement, ParseError> {
        let mut cases = Vec::new();
        while self.peek() == Some(&Token::KwПри) {
            cases.push(self.parse_value_match_case()?);
            self.skip_newlines();
        }

        let else_body = if self.eat(&Token::KwИначе) {
            Some(self.parse_block_or_single_statement()?)
        } else {
            None
        };

        self.skip_newlines();
        self.expect(&Token::Dedent)?;
        Ok(Statement::Selection(SelectionStatement::ValueMatch {
            expression,
            cases,
            else_body,
        }))
    }

    fn parse_value_match_case(&mut self) -> Result<ValueMatchCase, ParseError> {
        self.expect(&Token::KwПри)?;
        let mut values = vec![Box::new(self.parse_expression()?)];
        while self.eat(&Token::Comma) {
            values.push(Box::new(self.parse_expression()?));
        }
        self.expect(&Token::Colon)?;

        let body = self.parse_block_or_single_statement()?;

        Ok(ValueMatchCase { values, body })
    }

    fn parse_selection_condition_list_in_block(&mut self) -> Result<Statement, ParseError> {
        let mut cases = Vec::new();
        while self.peek() == Some(&Token::KwПри) {
            self.advance(); // consume при
            let condition = Box::new(self.parse_expression()?);
            self.expect(&Token::Colon)?;

            let body = self.parse_block_or_single_statement()?;
            cases.push(ConditionCase { condition, body });
            self.skip_newlines();
        }

        let else_body = if self.eat(&Token::KwИначе) {
            Some(self.parse_block_or_single_statement()?)
        } else {
            None
        };

        self.skip_newlines();
        self.expect(&Token::Dedent)?;
        Ok(Statement::Selection(SelectionStatement::ConditionList {
            cases,
            else_body,
        }))
    }

    fn parse_loop(&mut self) -> Result<Statement, ParseError> {
        let header = self.parse_loop_header()?;

        let while_condition = if self.eat(&Token::KwПока) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect(&Token::KwЦикл)?;

        // Block or single-line loop body
        let body = self.parse_block_or_single_statement()?;

        // Skip newlines before checking for post-condition
        self.skip_newlines();

        // Post-condition (at same indent as loop header, after Dedent)
        let post_condition = if self.eat(&Token::KwПо) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        Ok(Statement::Loop(LoopStatement {
            header,
            while_condition,
            body,
            post_condition,
        }))
    }

    fn parse_loop_header(&mut self) -> Result<LoopHeader, ParseError> {
        if self.eat(&Token::KwПовтор) {
            let count = self.parse_expression()?;
            Ok(LoopHeader::Repeat(Box::new(count)))
        } else if self.eat(&Token::KwДля) {
            let variable = self.expect_ident()?;
            let from = if self.eat(&Token::KwОт) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            let to = if self.eat(&Token::KwДо) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            let step = if self.eat(&Token::KwШаг) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            Ok(LoopHeader::For {
                variable,
                from,
                to,
                step,
            })
        } else {
            // цикл or пока — both mean Infinite header
            Ok(LoopHeader::Infinite)
        }
    }

    /// Parse `возврат` — greedy: parse expression if one can follow.
    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwВозврат)?;
        if self.can_start_expression() {
            let value = self.parse_expression()?;
            Ok(Statement::ReturnFromFunction(Box::new(value)))
        } else {
            Ok(Statement::ReturnFromProcedure)
        }
    }

    fn parse_procedure_call_with_keyword(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::KwВызов)?;
        // Parse the callable (name or expression producing a procedure)
        let procedure = Box::new(self.parse_expr_primary()?);
        self.expect(&Token::LParen)?;
        let arguments = self.parse_call_argument_list()?;
        self.expect(&Token::RParen)?;
        Ok(Statement::ProcedureCall {
            procedure,
            arguments,
        })
    }

    fn parse_call_argument_list(&mut self) -> Result<Vec<CallArgument>, ParseError> {
        let mut arguments = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(arguments);
        }

        arguments.push(self.parse_call_argument()?);
        while self.eat(&Token::Comma) {
            arguments.push(self.parse_call_argument()?);
        }
        Ok(arguments)
    }

    fn parse_call_argument(&mut self) -> Result<CallArgument, ParseError> {
        if self.eat(&Token::LessOrEqual) {
            let target = self.parse_lvalue()?;
            Ok(CallArgument::InOut(target))
        } else if self.eat(&Token::InputArrow) {
            let value = self.parse_expression()?;
            Ok(CallArgument::Input(Box::new(value)))
        } else {
            let value = self.parse_expression()?;
            Ok(CallArgument::Input(Box::new(value)))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_or()
    }

    // или (lowest precedence, left-associative)
    fn parse_expr_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_and()?;
        while self.eat(&Token::KwИли) {
            let right = self.parse_expr_and()?;
            left = Expr::BinaryOp {
                operator: BinaryOperator::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // и
    fn parse_expr_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_not()?;
        while self.eat(&Token::KwИ) {
            let right = self.parse_expr_not()?;
            left = Expr::BinaryOp {
                operator: BinaryOperator::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // не (unary prefix, right-associative)
    fn parse_expr_not(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Token::KwНе) {
            let operand = self.parse_expr_not()?;
            Ok(Expr::UnaryOp {
                operator: UnaryOperator::Not,
                operand: Box::new(operand),
            })
        } else {
            self.parse_expr_equality()
        }
    }

    // = /= (non-associative)
    fn parse_expr_equality(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_expr_ordering()?;
        let operator = match self.peek() {
            Some(Token::Equal) => BinaryOperator::Equal,
            Some(Token::NotEqual) => BinaryOperator::NotEqual,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_expr_ordering()?;
        Ok(Expr::BinaryOp {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    // > < >= <= (non-associative)
    fn parse_expr_ordering(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_expr_add()?;
        let operator = match self.peek() {
            Some(Token::Greater) => BinaryOperator::Greater,
            Some(Token::Less) => BinaryOperator::Less,
            Some(Token::GreaterOrEqual) => BinaryOperator::GreaterOrEqual,
            Some(Token::LessOrEqual) => BinaryOperator::LessOrEqual,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_expr_add()?;
        Ok(Expr::BinaryOp {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    // + - (left-associative)
    fn parse_expr_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_mul()?;
        loop {
            let operator = match self.peek() {
                Some(Token::Plus) => BinaryOperator::Add,
                Some(Token::Minus) => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_mul()?;
            left = Expr::BinaryOp {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // * / // /% (left-associative)
    fn parse_expr_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_unary()?;
        loop {
            let operator = match self.peek() {
                Some(Token::Star) => BinaryOperator::Multiply,
                Some(Token::Slash) => BinaryOperator::Divide,
                Some(Token::SlashSlash) => BinaryOperator::IntegerDivide,
                Some(Token::SlashPercent) => BinaryOperator::Remainder,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_power()?;
            left = Expr::BinaryOp {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // ** (right-associative)
    fn parse_expr_power(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_expr_length()?;
        if self.eat(&Token::StarStar) {
            let exponent = self.parse_expr_power()?; // right-recursive for right-associativity
            Ok(Expr::BinaryOp {
                operator: BinaryOperator::Power,
                left: Box::new(base),
                right: Box::new(exponent),
            })
        } else {
            Ok(base)
        }
    }

    // unary - + (prefix)
    fn parse_expr_unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Token::Minus) {
            let operand = self.parse_expr_unary()?;
            Ok(Expr::UnaryOp {
                operator: UnaryOperator::Negate,
                operand: Box::new(operand),
            })
        } else if self.eat(&Token::Plus) {
            let operand = self.parse_expr_unary()?;
            Ok(Expr::UnaryOp {
                operator: UnaryOperator::Plus,
                operand: Box::new(operand),
            })
        } else {
            self.parse_expr_power()
        }
    }

    // # (length, unary prefix)
    fn parse_expr_length(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Token::Hash) {
            let operand = self.parse_expr_postfix()?;
            Ok(Expr::UnaryOp {
                operator: UnaryOperator::Length,
                operand: Box::new(operand),
            })
        } else {
            self.parse_expr_postfix()
        }
    }

    // Postfix: subscript f[i], slice f[a:b], function call f(args)
    fn parse_expr_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expr_primary()?;

        loop {
            match self.peek() {
                Some(Token::LBracket) => {
                    self.advance(); // consume [
                    expr = self.parse_subscript_or_slice(expr)?;
                }
                Some(Token::LParen) => {
                    self.advance(); // consume (
                    let arguments = self.parse_func_arg_list()?;
                    self.expect(&Token::RParen)?;
                    expr = Expr::FunctionCall {
                        function: Box::new(expr),
                        arguments,
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    /// After consuming `[`, parse subscript or slice.
    fn parse_subscript_or_slice(&mut self, collection: Expr) -> Result<Expr, ParseError> {
        let collection = Box::new(collection);

        // [:...] — slice with no from
        if self.eat(&Token::Colon) {
            let to = if self.peek() != Some(&Token::RBracket) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&Token::RBracket)?;
            return Ok(Expr::Slice {
                collection,
                from: None,
                to,
            });
        }

        let first_expr = self.parse_expression()?;

        if self.eat(&Token::Colon) {
            // [from:to]
            let to = if self.peek() != Some(&Token::RBracket) {
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };
            self.expect(&Token::RBracket)?;
            Ok(Expr::Slice {
                collection,
                from: Some(Box::new(first_expr)),
                to,
            })
        } else {
            // [index]
            self.expect(&Token::RBracket)?;
            Ok(Expr::Subscript {
                collection,
                index: Box::new(first_expr),
            })
        }
    }

    fn parse_func_arg_list(&mut self) -> Result<Vec<Box<Expr>>, ParseError> {
        let mut arguments = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(arguments);
        }

        arguments.push(Box::new(self.parse_expression()?));
        while self.eat(&Token::Comma) {
            arguments.push(Box::new(self.parse_expression()?));
        }
        Ok(arguments)
    }

    // Primary expressions: literals, identifiers, parens, tuples
    fn parse_expr_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Integer(_)) => {
                let (_, token, _) = self.advance().unwrap();
                match token {
                    Token::Integer(value) => Ok(Expr::Literal(Literal::Integer(value))),
                    _ => unreachable!(),
                }
            }
            Some(Token::Real(_)) => {
                let (_, token, _) = self.advance().unwrap();
                match token {
                    Token::Real(value) => Ok(Expr::Literal(Literal::Real(value))),
                    _ => unreachable!(),
                }
            }
            Some(Token::Text(_)) => {
                let (_, token, _) = self.advance().unwrap();
                match token {
                    Token::Text(value) => Ok(Expr::Literal(Literal::Text(value))),
                    _ => unreachable!(),
                }
            }
            Some(Token::KwПусто) => {
                self.advance();
                Ok(Expr::Literal(Literal::Null))
            }
            Some(Token::KwДа) => {
                self.advance();
                Ok(Expr::Literal(Literal::Boolean(true)))
            }
            Some(Token::KwНет) => {
                self.advance();
                Ok(Expr::Literal(Literal::Boolean(false)))
            }
            Some(Token::KwПс) => {
                self.advance();
                Ok(Expr::Literal(Literal::Text("\n".to_string())))
            }
            Some(Token::KwПи) | Some(Token::KwPi) => {
                self.advance();
                Ok(Expr::Literal(Literal::Real(std::f64::consts::PI)))
            }
            Some(Token::Ident(_)) => {
                let name = self.expect_ident()?;
                Ok(Expr::Name(name))
            }
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::TupleOpen) => {
                self.advance();
                let mut elements = Vec::new();
                if self.peek() != Some(&Token::TupleClose) {
                    elements.push(Box::new(self.parse_expression()?));
                    while self.eat(&Token::Comma) {
                        elements.push(Box::new(self.parse_expression()?));
                    }
                }
                self.expect(&Token::TupleClose)?;
                Ok(Expr::TupleConstruct(elements))
            }
            Some(_) => {
                let (position_start, found, position_end) = self.current.as_ref().unwrap();
                Err(ParseError::UnexpectedToken {
                    position_start: *position_start,
                    position_end: *position_end,
                    found: found.clone(),
                    expected: "выражение".to_string(),
                })
            }
            None => Err(ParseError::UnexpectedEof {
                expected: "expression".to_string(),
            }),
        }
    }
}
