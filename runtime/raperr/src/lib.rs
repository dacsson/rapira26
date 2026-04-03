//! Pretty printing API for errors in runtime

use annotate_snippets::renderer::DecorStyle;
use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

use std::ffi::{CStr, c_char};

/// Pretty error output
fn generate_runtime_error(
    source: &str,
    path: &str,
    position_start: usize,
    position_end: usize,
    msg: &str,
) -> String {
    let report = &[Level::ERROR
        .with_name(Some("Упс, ошибка"))
        .primary_title("ошибка в рантайме")
        .element(
            Snippet::source(source).path(path).annotation(
                AnnotationKind::Primary
                    .span(position_start..position_end)
                    .label(format!("{msg}")),
            ),
        )];

    let renderer = Renderer::styled().decor_style(DecorStyle::Unicode);
    return renderer.render(report);
}

#[unsafe(no_mangle)]
pub extern "C" fn runtime_error_description(
    source: *const c_char,
    path: *const c_char,
    position_start: usize,
    position_end: usize,
    msg: *const c_char,
) {
    let desc = generate_runtime_error(
        unsafe { CStr::from_ptr(source) }
            .to_str()
            .expect("Получил строку инвалид из рантайма, что делать?"),
        unsafe { CStr::from_ptr(path) }
            .to_str()
            .expect("Получил строку инвалид из рантайма, что делать?"),
        position_start,
        position_end,
        unsafe { CStr::from_ptr(msg) }
            .to_str()
            .expect("Получил строку инвалид из рантайма, что делать?"),
    );
    println!("{}", desc);
}
