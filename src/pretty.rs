//! Pretty printing for errors

use crate::lexer::*;
use crate::parser::*;

use annotate_snippets::renderer::DecorStyle;
use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

/// Pretty error output
pub fn pretty_parse_error(source: &str, path: &str, err: ParseError) -> String {
    match err {
        ParseError::LexerError(LexerError { position, message }) => {
            let report = &[Level::ERROR
                .with_name(Some("Упс, ошибка"))
                .primary_title("ошибка при лексинге")
                .element(
                    Snippet::source(source).path(path).annotation(
                        AnnotationKind::Primary
                            .span(position..position)
                            .label(format!("{message}")),
                    ),
                )];

            let renderer = Renderer::styled().decor_style(DecorStyle::Unicode);
            return renderer.render(report);
        }
        ParseError::UnexpectedEof { expected } => {
            let report = &[Level::ERROR
                .with_name(Some("Упс, ошибка"))
                .primary_title("ошибка при парсинге")
                .element(
                    Snippet::source(source).path(path).annotation(
                        AnnotationKind::Primary
                            .span(0..source.len())
                            .label(format!("файл подошёл к концу... а я ждал {expected}")),
                    ),
                )];

            let renderer = Renderer::styled().decor_style(DecorStyle::Unicode);
            return renderer.render(report);
        }
        ParseError::UnexpectedToken {
            position_start,
            position_end,
            found,
            expected,
        } => {
            let report = &[Level::ERROR
                .with_name(Some("Упс, ошибка"))
                .primary_title("ошибка при парсинге")
                .element(
                    Snippet::source(source).path(path).annotation(
                        AnnotationKind::Primary
                            .span(position_start..position_end)
                            .label(format!("ожидал {expected}... но нашёл {found} !?")),
                    ),
                )];

            let renderer = Renderer::styled().decor_style(DecorStyle::Unicode);
            return renderer.render(report);
        }
    }
}
