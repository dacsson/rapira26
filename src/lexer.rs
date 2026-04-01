//! WARNING: this lexer is 99% written by AI
//! Indentation-aware lexer for the Rapira language (spec Препринт 767).
//!
//! Produces `(byte_start, Token, byte_end)` triples.
//! Emits `Indent`/`Dedent` tokens based on leading whitespace changes
//! (Python-style significant indentation). Newlines separate statements.
//! Inside balanced delimiters `()`, `[]`, `<* *>`, newlines and indentation
//! are suppressed.
//!
//! Tabs in leading indentation are rejected; only spaces are allowed.
//! Comments (`\` to end of line) and blank lines are skipped during
//! indentation processing and never affect the indent level.

use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────────────────
    // Control flow
    KwЕсли,   // если
    KwТо,     // то
    KwИначе,  // иначе
    KwВыбор,  // выбор
    KwПри,    // при
    KwДля,    // для
    KwОт,     // от
    KwДо,     // до
    KwШаг,    // шаг
    KwПока,   // пока
    KwПовтор, // повтор
    KwЦикл,   // цикл
    KwПо,     // по
    KwВыход,  // выход

    // Definitions
    KwПроц,    // проц
    KwФунк,    // функ
    KwВозврат, // возврат
    KwЧужие,   // чужие
    KwСвои,    // свои

    // I/O
    KwВывод,  // вывод
    KwВвод,   // ввод
    KwБпс,    // бпс  (вывод бпс — no newline)
    KwТекста, // текста  (ввод текста)
    KwВызов,  // вызов

    // Boolean operators
    KwИ,   // и
    KwИли, // или
    KwНе,  // не

    // Constant literals (treated as keywords for unambiguous parsing)
    KwПусто, // пусто
    KwДа,    // да
    KwНет,   // нет
    KwПс,    // пс  (newline character constant, spec §2.3)
    KwПи,    // пи  (π)
    KwPi,    // pi  (π, Latin alias)

    // ── Indentation tokens ───────────────────────────────────────────────
    Newline, // end of a logical line
    Indent,  // increase in indentation level
    Dedent,  // decrease in indentation level

    // ── Identifiers & literals ────────────────────────────────────────────
    Ident(String),
    Integer(i64),
    Real(f64),
    Text(String),

    // ── Operators ─────────────────────────────────────────────────────────
    StarStar,       // **
    Star,           // *
    SlashSlash,     // //
    SlashPercent,   // /%
    Slash,          // /
    Plus,           // +
    Minus,          // -
    Hash,           // #
    Assign,         // :=
    Equal,          // =
    NotEqual,       // /=
    LessOrEqual,    // <=
    GreaterOrEqual, // >=
    Less,           // <
    Greater,        // >
    InputArrow,     // =>  (input parameter marker)

    // ── Punctuation ───────────────────────────────────────────────────────
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    Colon,    // :
    Comma,    // ,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Token::KwЕсли => write!(f, "если"),
            Token::KwТо => write!(f, "то"),
            Token::KwИначе => write!(f, "иначе"),
            Token::KwВыбор => write!(f, "выбор"),
            Token::KwПри => write!(f, "при"),
            Token::KwДля => write!(f, "для"),
            Token::KwОт => write!(f, "от"),
            Token::KwДо => write!(f, "до"),
            Token::KwШаг => write!(f, "шаг"),
            Token::KwПока => write!(f, "пока"),
            Token::KwПовтор => write!(f, "повтор"),
            Token::KwЦикл => write!(f, "цикл"),
            Token::KwПо => write!(f, "по"),
            Token::KwВыход => write!(f, "выход"),
            Token::KwПроц => write!(f, "проц"),
            Token::KwФунк => write!(f, "функ"),
            Token::KwВозврат => write!(f, "возврат"),
            Token::KwЧужие => write!(f, "чужие"),
            Token::KwСвои => write!(f, "свои"),
            Token::KwВывод => write!(f, "вывод"),
            Token::KwВвод => write!(f, "ввод"),
            Token::KwБпс => write!(f, "бпс"),
            Token::KwТекста => write!(f, "текста"),
            Token::KwВызов => write!(f, "вызов"),
            Token::KwИ => write!(f, "и"),
            Token::KwИли => write!(f, "или"),
            Token::KwНе => write!(f, "не"),
            Token::KwПусто => write!(f, "пусто"),
            Token::KwДа => write!(f, "да"),
            Token::KwНет => write!(f, "нет"),
            Token::KwПс => write!(f, "пс"),
            Token::KwПи => write!(f, "пи"),
            Token::KwPi => write!(f, "pi"),
            Token::Newline => write!(f, "переход на новую строку"),
            Token::Indent => write!(f, "просто отступ"),
            Token::Dedent => write!(f, "отступ на один уровень меньше предыдущего"),
            Token::Ident(_) => write!(f, "идентификатор"),
            Token::Integer(_) => write!(f, "целое число"),
            Token::Real(_) => write!(f, "вещественное число"),
            Token::Text(_) => write!(f, "текст"),
            Token::StarStar => write!(f, "возведение в степень ( `**` )"),
            Token::Star => write!(f, "умножение ( `*` )"),
            Token::SlashSlash => write!(f, "деление ( `//` )"),
            Token::SlashPercent => write!(f, "модуль ( `/%` )"),
            Token::Slash => write!(f, "деление ( `/` )"),
            Token::Plus => write!(f, "сложение ( `+` )"),
            Token::Minus => write!(f, "минус ( `-` )"),
            Token::Hash => write!(f, "получение длины контейнера ( `#` )"),
            Token::Assign => write!(f, "присваивание ( `:=` )"),
            Token::Equal => write!(f, "проверка на равенствно ( `=` )"),
            Token::NotEqual => write!(f, "неравенство ( `/=` )"),
            Token::LessOrEqual => write!(f, "`<="),
            Token::GreaterOrEqual => write!(f, "`>=`"),
            Token::Less => write!(f, "`<`"),
            Token::Greater => write!(f, "`>`"),
            Token::InputArrow => write!(f, "`=>`"),
            Token::LParen => write!(f, "`(`"),
            Token::RParen => write!(f, "`)`"),
            Token::LBracket => write!(f, "`[`"),
            Token::RBracket => write!(f, "`]`"),
            Token::Colon => write!(f, "`:`"),
            Token::Comma => write!(f, "`,`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub position: usize,
    pub message: String,
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

pub struct Lexer<'input> {
    source: &'input str,
    /// Peekable iterator over (byte_index, char) pairs
    chars: std::iter::Peekable<std::str::CharIndices<'input>>,
    /// Byte offset of the character currently being examined
    current_position: usize,
    /// Stack of indentation levels (in spaces), starts with [0]
    indent_stack: Vec<usize>,
    /// Buffered tokens (multiple Dedents, etc.)
    pending: VecDeque<Result<(usize, Token, usize), LexerError>>,
    /// True when the next content should trigger indentation processing
    at_line_start: bool,
    /// Nesting depth of `()`, `[]`, `<* *>` — suppresses Newline/Indent/Dedent
    paren_depth: usize,
    /// Whether EOF Dedents have already been emitted
    emitted_eof_dedents: bool,
}

impl<'input> Lexer<'input> {
    pub fn new(source: &'input str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            current_position: 0,
            indent_stack: vec![0],
            pending: VecDeque::new(),
            at_line_start: true,
            paren_depth: 0,
            emitted_eof_dedents: false,
        }
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, character)| character)
    }

    fn peek_position(&mut self) -> usize {
        self.chars
            .peek()
            .map(|&(position, _)| position)
            .unwrap_or(self.source.len())
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next();
        if let Some((position, _)) = next {
            self.current_position = position;
        }
        next
    }

    /// Skip characters until end of line (stops before the newline char).
    fn skip_to_eol(&mut self) {
        while let Some(character) = self.peek_char() {
            if character == '\n' || character == '\r' {
                break;
            }
            self.advance();
        }
    }

    /// Consume a newline sequence (\n, \r, or \r\n).
    fn consume_newline(&mut self) {
        match self.peek_char() {
            Some('\r') => {
                self.advance();
                if self.peek_char() == Some('\n') {
                    self.advance();
                }
            }
            Some('\n') => {
                self.advance();
            }
            _ => {}
        }
    }

    // ── Indentation processing ───────────────────────────────────────────

    /// Called at the start of each logical line to measure indentation
    /// and emit Indent/Dedent tokens as needed.
    fn process_line_start(&mut self) {
        self.at_line_start = false;

        // Inside delimiters: indentation is not significant
        if self.paren_depth > 0 {
            loop {
                match self.peek_char() {
                    Some(' ') | Some('\t') | Some('\r') | Some('\n') | Some(';') => {
                        self.advance();
                    }
                    Some('\\') => {
                        self.advance();
                        self.skip_to_eol();
                    }
                    _ => break,
                }
            }
            return;
        }

        // Outside delimiters: measure indentation
        loop {
            let line_start = self.peek_position();
            let mut indent: usize = 0;

            // Count leading spaces
            while self.peek_char() == Some(' ') {
                self.advance();
                indent += 1;
            }

            // Reject tabs in indentation
            if self.peek_char() == Some('\t') {
                let tab_position = self.peek_position();
                self.pending.push_back(Err(LexerError {
                    position: tab_position,
                    message: "tabs are not allowed, use spaces for indentation".to_string(),
                }));
                return;
            }

            // Skip blank lines
            match self.peek_char() {
                Some('\n') | Some('\r') => {
                    self.consume_newline();
                    continue;
                }
                None => return, // EOF — handled by emit_eof_dedents
                _ => {}
            }

            // Skip comment-only lines
            if self.peek_char() == Some('\\') {
                self.advance(); // consume '\'
                self.skip_to_eol();
                self.consume_newline();
                continue;
            }

            // Content line — compare indent with stack top
            let current_indent = *self.indent_stack.last().unwrap();

            if indent > current_indent {
                self.indent_stack.push(indent);
                self.pending
                    .push_back(Ok((line_start, Token::Indent, line_start)));
            } else if indent < current_indent {
                while *self.indent_stack.last().unwrap() > indent {
                    self.indent_stack.pop();
                    self.pending
                        .push_back(Ok((line_start, Token::Dedent, line_start)));
                }
                if *self.indent_stack.last().unwrap() != indent {
                    self.pending.push_back(Err(LexerError {
                        position: line_start,
                        message: "dedent does not match any outer indentation level".to_string(),
                    }));
                }
            }

            break;
        }
    }

    /// Emit remaining Dedent tokens at end of file.
    fn emit_eof_dedents(&mut self) -> Option<Result<(usize, Token, usize), LexerError>> {
        if self.emitted_eof_dedents {
            return None;
        }
        self.emitted_eof_dedents = true;

        let pos = self.source.len();
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.pending.push_back(Ok((pos, Token::Dedent, pos)));
        }

        self.pending.pop_front()
    }

    // ── Token lexing ─────────────────────────────────────────────────────

    fn lex_identifier_or_keyword(&mut self, start: usize) -> Token {
        // Consume all identifier characters: letters (Cyrillic or Latin), digits, underscore
        while let Some(character) = self.peek_char() {
            if character.is_alphabetic() || character.is_ascii_digit() || character == '_' {
                self.advance();
            } else {
                break;
            }
        }

        // Find the end position (byte offset after last char of this token)
        let end = self
            .chars
            .peek()
            .map(|&(position, _)| position)
            .unwrap_or(self.source.len());

        let word = &self.source[start..end];

        match word {
            "если" => Token::KwЕсли,
            "то" => Token::KwТо,
            "иначе" => Token::KwИначе,
            "выбор" => Token::KwВыбор,
            "при" => Token::KwПри,
            "для" => Token::KwДля,
            "от" => Token::KwОт,
            "до" => Token::KwДо,
            "шаг" => Token::KwШаг,
            "пока" => Token::KwПока,
            "повтор" => Token::KwПовтор,
            "цикл" => Token::KwЦикл,
            "по" => Token::KwПо,
            "выход" => Token::KwВыход,
            "проц" => Token::KwПроц,
            "функ" => Token::KwФунк,
            "возврат" => Token::KwВозврат,
            "чужие" => Token::KwЧужие,
            "свои" => Token::KwСвои,
            "вывод" => Token::KwВывод,
            "ввод" => Token::KwВвод,
            "бпс" => Token::KwБпс,
            "текста" => Token::KwТекста,
            "вызов" => Token::KwВызов,
            "и" => Token::KwИ,
            "или" => Token::KwИли,
            "не" => Token::KwНе,
            "пусто" => Token::KwПусто,
            "да" => Token::KwДа,
            "нет" => Token::KwНет,
            "пс" => Token::KwПс,
            "пи" => Token::KwПи,
            "pi" => Token::KwPi,
            other => Token::Ident(other.to_string()),
        }
    }

    fn lex_number(&mut self, start: usize) -> Result<Token, LexerError> {
        // Consume integer digits
        while let Some(character) = self.peek_char() {
            if character.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal point
        let has_decimal = self.peek_char() == Some('.');
        if has_decimal {
            self.advance(); // consume '.'
            while let Some(character) = self.peek_char() {
                if character.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Check for exponent: 'e' or 'е' (Cyrillic е, U+0435) followed by optional sign and digits
        let has_exponent = matches!(self.peek_char(), Some('e') | Some('е'));
        if has_exponent {
            self.advance(); // consume 'e' or 'е'
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.advance();
            }
            while let Some(character) = self.peek_char() {
                if character.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let end = self
            .chars
            .peek()
            .map(|&(position, _)| position)
            .unwrap_or(self.source.len());

        let number_text = &self.source[start..end];

        if has_decimal || has_exponent {
            // Replace Cyrillic 'е' with ASCII 'e' for Rust's f64 parser
            let ascii_number_text = number_text.replace('е', "e");
            ascii_number_text
                .parse::<f64>()
                .map(Token::Real)
                .map_err(|_| LexerError {
                    position: start,
                    message: format!("invalid real literal: {number_text}"),
                })
        } else {
            number_text
                .parse::<i64>()
                .map(Token::Integer)
                .map_err(|_| LexerError {
                    position: start,
                    message: format!("integer literal out of range: {number_text}"),
                })
        }
    }

    fn lex_text_literal(&mut self, start: usize) -> Result<Token, LexerError> {
        // The opening `"` has already been consumed.
        // Rule: `""""` (four consecutive quotes) represents a single `"` in the string.
        // Any other `"` closes the literal. (spec §2.2.2 Текст)
        let mut text_content = String::new();

        loop {
            match self.advance() {
                None => {
                    return Err(LexerError {
                        position: start,
                        message: "unterminated text literal".to_string(),
                    });
                }
                Some((_, '"')) => {
                    // Could be end of string or escaped quote ("""")
                    if self.peek_char() == Some('"') {
                        self.advance(); // second "
                        if self.peek_char() == Some('"') {
                            self.advance(); // third "
                            if self.peek_char() == Some('"') {
                                self.advance(); // fourth "
                                text_content.push('"');
                            } else {
                                text_content.push('"');
                                text_content.push('"');
                                return Ok(Token::Text(text_content));
                            }
                        } else {
                            // Two quotes `""` = end of string
                            return Ok(Token::Text(text_content));
                        }
                    } else {
                        // Single closing quote
                        return Ok(Token::Text(text_content));
                    }
                }
                Some((_, character)) => {
                    text_content.push(character);
                }
            }
        }
    }

    fn next_token(&mut self) -> Option<Result<(usize, Token, usize), LexerError>> {
        // 1. Return pending tokens first
        if let Some(token) = self.pending.pop_front() {
            return Some(token);
        }

        // 2. Handle line start (indentation processing)
        if self.at_line_start {
            self.process_line_start();
            if let Some(token) = self.pending.pop_front() {
                return Some(token);
            }
            // If process_line_start didn't produce tokens and we're at EOF
            if self.peek_char().is_none() {
                return self.emit_eof_dedents();
            }
        }

        // 3. Skip inline whitespace (spaces, tabs within a line, semicolons)
        loop {
            match self.peek_char() {
                Some(' ') | Some('\t') | Some(';') => {
                    self.advance();
                }
                _ => break,
            }
        }

        // 4. Check for newline
        match self.peek_char() {
            Some('\n') | Some('\r') => {
                let (pos, ch) = self.advance().unwrap();
                if ch == '\r' && self.peek_char() == Some('\n') {
                    self.advance();
                }
                if self.paren_depth > 0 {
                    // Inside delimiters: newlines are ignored
                    return self.next_token();
                }
                self.at_line_start = true;
                return Some(Ok((pos, Token::Newline, pos + 1)));
            }
            _ => {}
        }

        // 5. Check for comment
        if self.peek_char() == Some('\\') {
            self.advance(); // consume '\'
            self.skip_to_eol();
            // Handle the newline (or EOF) at end of comment
            match self.peek_char() {
                Some('\n') | Some('\r') => {
                    let (newline_pos, ch) = self.advance().unwrap();
                    if ch == '\r' && self.peek_char() == Some('\n') {
                        self.advance();
                    }
                    if self.paren_depth > 0 {
                        return self.next_token();
                    }
                    self.at_line_start = true;
                    return Some(Ok((newline_pos, Token::Newline, newline_pos + 1)));
                }
                None => {
                    return self.emit_eof_dedents();
                }
                _ => unreachable!(),
            }
        }

        // 6. EOF
        if self.peek_char().is_none() {
            return self.emit_eof_dedents();
        }

        // 7. Lex the next token
        let (token_start, first_char) = self.advance()?;

        let token_result = match first_char {
            // ── Identifiers and keywords ─────────────────────────────────
            character if character.is_alphabetic() || character == '_' => {
                Ok(self.lex_identifier_or_keyword(token_start))
            }

            // ── Number literals ──────────────────────────────────────────
            character if character.is_ascii_digit() => self.lex_number(token_start),

            // ── Text literals ────────────────────────────────────────────
            '"' => self.lex_text_literal(token_start),

            // ── Multi-character operators ─────────────────────────────────
            ':' => {
                if self.peek_char() == Some('=') {
                    self.advance();
                    Ok(Token::Assign)
                } else {
                    Ok(Token::Colon)
                }
            }
            '=' => {
                if self.peek_char() == Some('>') {
                    self.advance();
                    Ok(Token::InputArrow)
                } else {
                    Ok(Token::Equal)
                }
            }
            '/' => match self.peek_char() {
                Some('/') => {
                    self.advance();
                    Ok(Token::SlashSlash)
                }
                Some('%') => {
                    self.advance();
                    Ok(Token::SlashPercent)
                }
                Some('=') => {
                    self.advance();
                    Ok(Token::NotEqual)
                }
                _ => Ok(Token::Slash),
            },
            '*' => {
                if self.peek_char() == Some('*') {
                    self.advance();
                    Ok(Token::StarStar)
                } else {
                    Ok(Token::Star)
                }
            }
            '<' => match self.peek_char() {
                Some('=') => {
                    self.advance();
                    Ok(Token::LessOrEqual)
                }
                _ => Ok(Token::Less),
            },
            '>' => {
                if self.peek_char() == Some('=') {
                    self.advance();
                    Ok(Token::GreaterOrEqual)
                } else {
                    Ok(Token::Greater)
                }
            }
            '+' => Ok(Token::Plus),
            '-' => Ok(Token::Minus),
            '#' => Ok(Token::Hash),

            // ── Punctuation ───────────────────────────────────────────────
            '(' => Ok(Token::LParen),
            ')' => Ok(Token::RParen),
            '[' => Ok(Token::LBracket),
            ']' => Ok(Token::RBracket),
            ',' => Ok(Token::Comma),

            unknown => Err(LexerError {
                position: token_start,
                message: format!("unexpected character: {unknown:?}"),
            }),
        };

        // Track paren depth for delimiter nesting
        if let Ok(ref token) = token_result {
            match token {
                Token::LParen | Token::LBracket => {
                    self.paren_depth += 1;
                }
                Token::RParen | Token::RBracket => {
                    if self.paren_depth > 0 {
                        self.paren_depth -= 1;
                    }
                }
                _ => {}
            }
        }

        let token_end = self
            .chars
            .peek()
            .map(|&(position, _)| position)
            .unwrap_or(self.source.len());

        Some(token_result.map(|token| (token_start, token, token_end)))
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<(usize, Token, usize), LexerError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}
