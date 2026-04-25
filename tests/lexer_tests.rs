use rapira26::lexer::{Lexer, Token};

/// Collect all tokens from source, panicking on lexer errors.
fn tokenize(source: &str) -> Vec<Token> {
    Lexer::new(source)
        .map(|result| result.expect("lexer error"))
        .map(|(_start, token, _end)| token)
        .collect()
}

// ── Keywords ────────────────────────────────────────────────────────────────

#[test]
fn lex_control_flow_keywords() {
    let tokens = tokenize("если то иначе");
    assert_eq!(tokens, vec![Token::KwЕсли, Token::KwТо, Token::KwИначе]);
}

#[test]
fn lex_loop_keywords() {
    let tokens = tokenize("для от до шаг пока повтор цикл по выход");
    assert_eq!(
        tokens,
        vec![
            Token::KwДля,
            Token::KwОт,
            Token::KwДо,
            Token::KwШаг,
            Token::KwПока,
            Token::KwПовтор,
            Token::KwЦикл,
            Token::KwПо,
            Token::KwВыход,
        ]
    );
}

#[test]
fn lex_definition_keywords() {
    let tokens = tokenize("проц функ возврат чужие свои");
    assert_eq!(
        tokens,
        vec![
            Token::KwПроц,
            Token::KwФунк,
            Token::KwВозврат,
            Token::KwЧужие,
            Token::KwСвои,
        ]
    );
}

#[test]
fn lex_io_keywords() {
    let tokens = tokenize("вывод ввод бпс текста вызов");
    assert_eq!(
        tokens,
        vec![
            Token::KwВывод,
            Token::KwВвод,
            Token::KwБпс,
            Token::KwТекста,
            Token::KwВызов,
        ]
    );
}

#[test]
fn lex_logical_keywords() {
    let tokens = tokenize("и или не");
    assert_eq!(tokens, vec![Token::KwИ, Token::KwИли, Token::KwНе]);
}

#[test]
fn lex_constant_keywords() {
    let tokens = tokenize("пусто да нет пс пи pi");
    assert_eq!(
        tokens,
        vec![
            Token::KwПусто,
            Token::KwДа,
            Token::KwНет,
            Token::KwПс,
            Token::KwПи,
            Token::KwPi,
        ]
    );
}

#[test]
fn lex_выбор_при() {
    let tokens = tokenize("выбор при");
    assert_eq!(tokens, vec![Token::KwВыбор, Token::KwПри]);
}

// ── Identifiers ─────────────────────────────────────────────────────────────

#[test]
fn lex_cyrillic_identifier() {
    let tokens = tokenize("ПРИВЕТ");
    assert_eq!(tokens, vec![Token::Ident("ПРИВЕТ".to_string())]);
}

#[test]
fn lex_latin_identifier() {
    let tokens = tokenize("hello");
    // "hello" is not a keyword, so it's an identifier
    assert_eq!(tokens, vec![Token::Ident("hello".to_string())]);
}

#[test]
fn lex_identifier_with_digits_and_underscore() {
    let tokens = tokenize("X1 ЧИСЛО_2 _foo");
    assert_eq!(
        tokens,
        vec![
            Token::Ident("X1".to_string()),
            Token::Ident("ЧИСЛО_2".to_string()),
            Token::Ident("_foo".to_string()),
        ]
    );
}

#[test]
fn lex_keyword_is_case_sensitive() {
    // Uppercase versions of keywords should be identifiers
    let tokens = tokenize("ЕСЛИ");
    assert_eq!(tokens, vec![Token::Ident("ЕСЛИ".to_string())]);
}

// ── Integer literals ────────────────────────────────────────────────────────

#[test]
fn lex_integer_zero() {
    let tokens = tokenize("0");
    assert_eq!(tokens, vec![Token::Integer(0)]);
}

#[test]
fn lex_integer_simple() {
    let tokens = tokenize("42 125 999");
    assert_eq!(
        tokens,
        vec![Token::Integer(42), Token::Integer(125), Token::Integer(999)]
    );
}

// ── Real literals ───────────────────────────────────────────────────────────

#[test]
fn lex_real_decimal() {
    let tokens = tokenize("3.14");
    assert_eq!(tokens, vec![Token::Real(3.14)]);
}

#[test]
fn lex_real_with_latin_exponent() {
    let tokens = tokenize("1.5e3");
    assert_eq!(tokens, vec![Token::Real(1500.0)]);
}

#[test]
fn lex_real_with_cyrillic_exponent() {
    // Cyrillic 'е' (U+0435) as exponent marker
    let tokens = tokenize("1.9е-8");
    assert_eq!(tokens, vec![Token::Real(1.9e-8)]);
}

#[test]
fn lex_real_exponent_no_decimal() {
    let tokens = tokenize("2e5");
    assert_eq!(tokens, vec![Token::Real(2e5)]);
}

#[test]
fn lex_real_with_positive_exponent() {
    let tokens = tokenize("1.0e+3");
    assert_eq!(tokens, vec![Token::Real(1.0e3)]);
}

// ── Text literals ───────────────────────────────────────────────────────────

#[test]
fn lex_text_simple() {
    let tokens = tokenize("\"hello\"");
    assert_eq!(tokens, vec![Token::Text("hello".to_string())]);
}

#[test]
fn lex_text_empty() {
    let tokens = tokenize("\"\"");
    assert_eq!(tokens, vec![Token::Text("".to_string())]);
}

#[test]
fn lex_text_cyrillic() {
    let tokens = tokenize("\"Привет мир\"");
    assert_eq!(tokens, vec![Token::Text("Привет мир".to_string())]);
}

#[test]
fn lex_text_with_escaped_quote() {
    // Spec §2.2.2: """" (four quotes) represents a single " in the text
    let tokens = tokenize("\"он сказал: \"\"\"\"привет\"\"\"\" ей\"");
    assert_eq!(
        tokens,
        vec![Token::Text("он сказал: \"привет\" ей".to_string())]
    );
}

// ── Operators ───────────────────────────────────────────────────────────────

#[test]
fn lex_arithmetic_operators() {
    let tokens = tokenize("+ - * / // /% **");
    assert_eq!(
        tokens,
        vec![
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::SlashSlash,
            Token::SlashPercent,
            Token::StarStar,
        ]
    );
}

#[test]
fn lex_comparison_operators() {
    let tokens = tokenize("= /= < > <= >=");
    assert_eq!(
        tokens,
        vec![
            Token::Equal,
            Token::NotEqual,
            Token::Less,
            Token::Greater,
            Token::LessOrEqual,
            Token::GreaterOrEqual,
        ]
    );
}

#[test]
fn lex_assignment() {
    let tokens = tokenize(":=");
    assert_eq!(tokens, vec![Token::Assign]);
}

#[test]
fn lex_hash() {
    let tokens = tokenize("#");
    assert_eq!(tokens, vec![Token::Hash]);
}

#[test]
fn lex_input_output_spec() {
    let tokens = tokenize("вых");
    assert_eq!(tokens, vec![Token::KwВых]);
}

// ── Punctuation ─────────────────────────────────────────────────────────────

#[test]
fn lex_punctuation() {
    let tokens = tokenize("( ) [ ] , :");
    assert_eq!(
        tokens,
        vec![
            Token::LParen,
            Token::RParen,
            Token::LBracket,
            Token::RBracket,
            Token::Comma,
            Token::Colon,
        ]
    );
}

// ── Whitespace and comments ─────────────────────────────────────────────────

#[test]
fn lex_skips_whitespace() {
    // Leading spaces on the first line produce an Indent; trailing spaces are inline whitespace
    let tokens = tokenize("  42   да  ");
    assert_eq!(
        tokens,
        vec![
            Token::Indent,
            Token::Integer(42),
            Token::KwДа,
            Token::Dedent
        ]
    );
}

#[test]
fn lex_newlines_produce_tokens() {
    // Newlines are now significant — they produce Newline tokens
    let tokens = tokenize("42\n;\nда");
    assert_eq!(
        tokens,
        vec![
            Token::Integer(42),
            Token::Newline,
            Token::Newline,
            Token::KwДа
        ]
    );
}

#[test]
fn lex_comments_produce_newline() {
    // A comment ending with newline produces a Newline token
    let tokens = tokenize("42 \\ this is a comment\nда");
    assert_eq!(
        tokens,
        vec![Token::Integer(42), Token::Newline, Token::KwДа]
    );
}

#[test]
fn lex_comment_at_end_of_input() {
    let tokens = tokenize("42 \\ trailing comment");
    assert_eq!(tokens, vec![Token::Integer(42)]);
}

#[test]
fn lex_empty_input() {
    let tokens = tokenize("");
    assert!(tokens.is_empty());
}

#[test]
fn lex_only_whitespace_and_comments() {
    // Blank lines and comment-only lines produce no content tokens
    let tokens = tokenize("  \n \\ comment\n  ");
    assert!(tokens.is_empty());
}

// ── Spans ───────────────────────────────────────────────────────────────────

#[test]
fn lex_spans_are_correct_for_ascii() {
    let triples: Vec<_> = Lexer::new("42 + X").map(|r| r.unwrap()).collect();
    // Lexer end position is the byte offset of the NEXT token's start (or source len)
    assert_eq!(triples[0].0, 0); // "42" starts at 0
    assert_eq!(triples[0].1, Token::Integer(42));
    assert_eq!(triples[1].1, Token::Plus);
    assert_eq!(triples[2].1, Token::Ident("X".to_string()));
}

// ── Compound expressions ────────────────────────────────────────────────────

#[test]
fn lex_assignment_statement() {
    let tokens = tokenize("X := 5");
    assert_eq!(
        tokens,
        vec![
            Token::Ident("X".to_string()),
            Token::Assign,
            Token::Integer(5),
        ]
    );
}

#[test]
fn lex_output_statement() {
    let tokens = tokenize("вывод: \"hello\", 42");
    assert_eq!(
        tokens,
        vec![
            Token::KwВывод,
            Token::Colon,
            Token::Text("hello".to_string()),
            Token::Comma,
            Token::Integer(42),
        ]
    );
}

#[test]
fn lex_procedure_definition_header() {
    let tokens = tokenize("проц РАМКА (N)");
    assert_eq!(
        tokens,
        vec![
            Token::KwПроц,
            Token::Ident("РАМКА".to_string()),
            Token::LParen,
            Token::Ident("N".to_string()),
            Token::RParen,
        ]
    );
}

#[test]
fn lex_function_call() {
    let tokens = tokenize("КВАДРАТ(7)");
    assert_eq!(
        tokens,
        vec![
            Token::Ident("КВАДРАТ".to_string()),
            Token::LParen,
            Token::Integer(7),
            Token::RParen,
        ]
    );
}

#[test]
fn lex_tuple_literal() {
    let tokens = tokenize("( 1, 2, 3 )");
    assert_eq!(
        tokens,
        vec![
            Token::LParen,
            Token::Integer(1),
            Token::Comma,
            Token::Integer(2),
            Token::Comma,
            Token::Integer(3),
            Token::RParen,
        ]
    );
}

#[test]
fn lex_complex_expression() {
    let tokens = tokenize("N /% M = 0");
    assert_eq!(
        tokens,
        vec![
            Token::Ident("N".to_string()),
            Token::SlashPercent,
            Token::Ident("M".to_string()),
            Token::Equal,
            Token::Integer(0),
        ]
    );
}

#[test]
fn lex_type_definition() {
    let program = tokenize("тип Сезон\n Зима\n Весна\n Лето\n Осень\n");
    assert_eq!(
        program,
        vec![
            Token::KwТип,
            Token::Ident("Сезон".to_string()),
            Token::Newline,
            Token::Indent,
            Token::Ident("Зима".to_string()),
            Token::Newline,
            Token::Ident("Весна".to_string()),
            Token::Newline,
            Token::Ident("Лето".to_string()),
            Token::Newline,
            Token::Ident("Осень".to_string()),
            Token::Newline,
            Token::Dedent,
        ]
    );
}

#[test]
fn lex_complex_type_definition() {
    let program = tokenize("тип ШкольныйЧел\n Ученик(имя, класс)\n Учитель(имя)\n Никто");
    assert_eq!(
        program,
        vec![
            Token::KwТип,
            Token::Ident("ШкольныйЧел".to_string()),
            Token::Newline,
            Token::Indent,
            Token::Ident("Ученик".to_string()),
            Token::LParen,
            Token::Ident("имя".to_string()),
            Token::Comma,
            Token::Ident("класс".to_string()),
            Token::RParen,
            Token::Newline,
            Token::Ident("Учитель".to_string()),
            Token::LParen,
            Token::Ident("имя".to_string()),
            Token::RParen,
            Token::Newline,
            Token::Ident("Никто".to_string()),
            Token::Dedent,
        ]
    );
}

#[test]
fn lex_imports() {
    let program = tokenize("подкл \"мод\" (функция, ПРОЦЕДУРА)");
    assert_eq!(
        program,
        vec![
            Token::KwПодкл,
            Token::Text("мод".to_string()),
            Token::LParen,
            Token::Ident("функция".to_string()),
            Token::Comma,
            Token::Ident("ПРОЦЕДУРА".to_string()),
            Token::RParen,
        ]
    );
}

// ── Error cases ─────────────────────────────────────────────────────────────
#[test]
fn lex_unterminated_text_is_error() {
    let results: Vec<_> = Lexer::new("\"hello").collect();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

#[test]
fn lex_unexpected_character_is_error() {
    let results: Vec<_> = Lexer::new("@").collect();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}
