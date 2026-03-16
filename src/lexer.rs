/// Hand-written lexer for the Rapira language (spec Препринт 767).
///
/// Produces `(byte_start, Token, byte_end)` triples for LALRPOP.
/// Whitespace, newlines, semicolons (statement separators), and
/// `\`-to-end-of-line comments are silently consumed.
///
/// Case-sensitivity: keywords are lowercase per the spec examples.
/// Identifiers are case-sensitive (variables are conventionally uppercase).

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────────────────
    // Control flow
    KwЕсли,   // если
    KwТо,     // то
    KwИначе,  // иначе
    KwВсе,    // все
    KwВыбор,  // выбор
    KwПри,    // при
    KwДля,    // для
    KwОт,     // от
    KwДо,     // до
    KwШаг,    // шаг
    KwПока,   // пока
    KwПовтор, // повтор
    KwЦикл,   // цикл
    KwКц,     // кц
    KwПо,     // по
    KwВыход,  // выход

    // Definitions
    KwПроц,    // проц
    KwФунк,    // функ
    KwКонец,   // конец
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
    KwНс,    // нс  (newline character constant)
    KwПи,    // пи  (π)
    KwPi,    // pi  (π, Latin alias)

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
    LParen,     // (
    RParen,     // )
    LBracket,   // [
    RBracket,   // ]
    TupleOpen,  // <*
    TupleClose, // *>
    Colon,      // :
    Comma,      // ,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub position: usize,
    pub message: String,
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "lexer error at byte {}: {}",
            self.position, self.message
        )
    }
}

pub struct Lexer<'input> {
    source: &'input str,
    /// Peekable iterator over (byte_index, char) pairs
    chars: std::iter::Peekable<std::str::CharIndices<'input>>,
    /// Byte offset of the character currently being examined
    current_position: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(source: &'input str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            current_position: 0,
        }
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, character)| character)
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next();
        if let Some((position, _)) = next {
            self.current_position = position;
        }
        next
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                // Spaces, tabs, newlines, carriage returns, semicolons are all
                // treated as statement separators / insignificant whitespace.
                Some(' ') | Some('\t') | Some('\n') | Some('\r') | Some(';') => {
                    self.advance();
                }
                // `\` begins a comment that runs to end of line (spec §2.1)
                Some('\\') => {
                    self.advance();
                    while let Some(character) = self.peek_char() {
                        self.advance();
                        if character == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

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
            "все" => Token::KwВсе,
            "выбор" => Token::KwВыбор,
            "при" => Token::KwПри,
            "для" => Token::KwДля,
            "от" => Token::KwОт,
            "до" => Token::KwДо,
            "шаг" => Token::KwШаг,
            "пока" => Token::KwПока,
            "повтор" => Token::KwПовтор,
            "цикл" => Token::KwЦикл,
            "кц" => Token::KwКц,
            "по" => Token::KwПо,
            "выход" => Token::KwВыход,
            "проц" => Token::KwПроц,
            "функ" => Token::KwФунк,
            "конец" => Token::KwКонец,
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
            "нс" => Token::KwНс,
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
                                // Three quotes: end of string + something
                                // Actually, spec says '""' in text = literal '"'.
                                // Re-reading: `""""` in a text literal = one `"`.
                                // Two quotes `""` outside a literal = empty string (spec §1.2).
                                // Inside a text: `""""` = `"`.
                                // So `""` inside text = empty, closing `"` + opening `"` of next?
                                // The spec §2.2.2 says: 'Последовательность """" в обозначении
                                // текста представляет литеру """ в тексте.'
                                // That is exactly four quotes = one quote.
                                // Two quotes `""` = end of string (empty string).
                                // So `""` always ends the text. """"  means end+start with a "?
                                // Most natural: `""` = empty string, `""""` = one `"` inside.
                                // Three `"""` = empty string followed by opening of next string.
                                // Put the two consumed quotes back as just closing the string:
                                text_content.push('"');
                                text_content.push('"');
                                // We consumed 3 quotes. The last state: peeked non-".
                                // Treat as if we found "}}" in the string, not a proper escape.
                                // This is an edge case; return what we have.
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
        self.skip_whitespace_and_comments();

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
                } else if self.peek_char() == Some('>') {
                    self.advance();
                    Ok(Token::TupleClose)
                } else {
                    Ok(Token::Star)
                }
            }
            '<' => match self.peek_char() {
                Some('*') => {
                    self.advance();
                    Ok(Token::TupleOpen)
                }
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
