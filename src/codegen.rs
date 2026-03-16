//! AST → C code generator.
//!
//! Walks the AST and emits C code that uses the runtime library (runtime.h).
//!
//! Key conventions (matching runtime/translated/prime.c):
//!   - Every expression emits into a temp: `RAP_Object *_tN = ...;`
//!   - Locals are `_local_NAME` (Cyrillic transliterated to Latin)
//!   - Function defs become `RAP_Object *RAP_FUNC_NAME(struct RAP_CallFrame *_frame, ...)`
//!   - No `free()` calls — future GC will handle memory
//!   - All values are `RAP_Object *`, created via `RAP_create_*_obj()`

use crate::ast::*;

/// Escape a string for embedding in a C string literal.
fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_ascii_control() => {
                // Emit as octal escape for other control chars
                for byte in c.to_string().bytes() {
                    out.push_str(&format!("\\{:03o}", byte));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Holds codegen state while walking the AST.
pub struct Codegen {
    /// Current scope output (main body, or function body while emitting a definition)
    output: String,
    /// File-scope C function definitions, flushed before main()
    forward_decls: String,
    /// Current indentation depth
    indent_level: usize,
    /// Counter for unique temporaries: _t0, _t1, _t2, ...
    temp_counter: usize,
    /// Counter for unique string buffers: _s0, _s1, ...
    string_counter: usize,
    /// Counter for parameter names: _p0, _p1, ...
    param_counter: usize,
    /// Name of the current call frame variable ("_main_frame" at top level, "_frame" inside funcs)
    current_frame: String,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            forward_decls: String::new(),
            indent_level: 1, // inside main()
            temp_counter: 0,
            string_counter: 0,
            param_counter: 0,
            current_frame: "_main_frame".to_string(),
        }
    }

    /// Main entry point: walk the whole program, return generated C source.
    pub fn generate(mut self, program: &Program) -> String {
        for unit in &program.units {
            self.emit_program_unit(unit);
        }

        // Assemble: prelude → forward decls → main() { body }
        let mut result = String::new();
        result.push_str("#include \"runtime.h\"\n");
        result.push_str("#include <math.h>\n");
        result.push_str("#include <stdio.h>\n");
        result.push_str("#include <stdlib.h>\n");
        result.push_str("#include <string.h>\n");
        result.push('\n');
        result.push_str(&self.forward_decls);
        result.push_str("int main(void) {\n");
        result.push_str("  struct RAP_CallFrame _main_frame = {NULL, NULL, 0};\n");
        result.push('\n');
        result.push_str(&self.output);
        result.push_str("  return 0;\n");
        result.push_str("}\n");
        result
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn fresh_temp(&mut self) -> String {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn fresh_string_buf(&mut self) -> String {
        let name = format!("_s{}", self.string_counter);
        self.string_counter += 1;
        name
    }

    fn fresh_param(&mut self) -> String {
        let name = format!("_p{}", self.param_counter);
        self.param_counter += 1;
        name
    }

    fn emit_line(&mut self, line: &str) {
        for _ in 0..self.indent_level {
            self.output.push_str("  ");
        }
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn emit_blank_line(&mut self) {
        self.output.push('\n');
    }

    /// Transliterate Cyrillic identifier to Latin (loosely based on GOST 7.79).
    fn transliterate(rapira_name: &str) -> String {
        let mut result = String::new();
        for ch in rapira_name.chars() {
            let mapped = match ch {
                'А' | 'а' => "A",
                'Б' | 'б' => "B",
                'В' | 'в' => "V",
                'Г' | 'г' => "G",
                'Д' | 'д' => "D",
                'Е' | 'е' => "E",
                'Ё' | 'ё' => "YO",
                'Ж' | 'ж' => "ZH",
                'З' | 'з' => "Z",
                'И' | 'и' => "I",
                'Й' | 'й' => "J",
                'К' | 'к' => "K",
                'Л' | 'л' => "L",
                'М' | 'м' => "M",
                'Н' | 'н' => "N",
                'О' | 'о' => "O",
                'П' | 'п' => "P",
                'Р' | 'р' => "R",
                'С' | 'с' => "S",
                'Т' | 'т' => "T",
                'У' | 'у' => "U",
                'Ф' | 'ф' => "F",
                'Х' | 'х' => "KH",
                'Ц' | 'ц' => "TS",
                'Ч' | 'ч' => "CH",
                'Ш' | 'ш' => "SH",
                'Щ' | 'щ' => "SHCH",
                'Ъ' | 'ъ' => "",
                'Ы' | 'ы' => "Y",
                'Ь' | 'ь' => "",
                'Э' | 'э' => "E",
                'Ю' | 'ю' => "YU",
                'Я' | 'я' => "YA",
                _ if ch.is_ascii_alphanumeric() || ch == '_' => {
                    result.push(ch);
                    continue;
                }
                _ => "_",
            };
            result.push_str(mapped);
        }
        result
    }

    fn mangle_name(&self, rapira_name: &str) -> String {
        format!("_local_{}", Self::transliterate(rapira_name))
    }

    fn mangle_func_name(&self, rapira_name: &str) -> String {
        format!("RAP_FUNC_{}", Self::transliterate(rapira_name))
    }

    fn emit_statement_list(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.emit_statement(stmt);
        }
    }

    // ── Program Units ────────────────────────────────────────────────────

    fn emit_program_unit(&mut self, unit: &ProgramUnit) {
        match unit {
            ProgramUnit::FunctionDefinition(func_def) => self.emit_function_def(func_def),
            ProgramUnit::ProcedureDefinition(proc_def) => self.emit_procedure_def(proc_def),
            ProgramUnit::Statement(stmt) => self.emit_statement(stmt),
        }
    }

    // ── Definitions ──────────────────────────────────────────────────────

    fn emit_function_def(&mut self, func_def: &FunctionDefinition) {
        let name = func_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);

        // 1. Emit C function body into forward_decls.
        //    Save current codegen state, emit function body, then restore.
        let saved_output = std::mem::take(&mut self.output);
        let saved_indent = self.indent_level;
        let saved_temp = self.temp_counter;
        let saved_string = self.string_counter;
        let saved_frame = std::mem::replace(&mut self.current_frame, "_frame".to_string());

        self.indent_level = 1;
        self.temp_counter = 0;
        self.string_counter = 0;

        // Unpack parameters from _args array
        for (i, param_name) in func_def.parameters.iter().enumerate() {
            let mangled = self.mangle_name(param_name);
            self.emit_line(&format!("RAP_Object *{} = _args[{}];", mangled, i));
        }
        if !func_def.parameters.is_empty() {
            self.emit_blank_line();
        }

        self.emit_statement_list(&func_def.body);

        // Wrap in C function signature
        let func_body = std::mem::take(&mut self.output);
        self.forward_decls.push_str(&format!("// функ {}\n", name));
        self.forward_decls.push_str(&format!(
            "RAP_Object *{}(struct RAP_CallFrame *_frame,\n",
            c_func_name
        ));
        // Align continuation to after the opening paren
        let align = format!("RAP_Object *{}(", c_func_name).len();
        self.forward_decls.push_str(&format!(
            "{:>width$}RAP_Object **_args, unsigned int _argc) {{\n",
            "",
            width = align
        ));
        self.forward_decls.push_str(&func_body);
        self.forward_decls.push_str("}\n\n");

        // Restore state
        self.output = saved_output;
        self.indent_level = saved_indent;
        self.temp_counter = saved_temp;
        self.string_counter = saved_string;
        self.current_frame = saved_frame;

        // 2. Emit callable registration at current scope (the enclosing scope "owns" it).
        self.emit_callable_registration(
            name,
            &c_func_name,
            &func_def
                .parameters
                .iter()
                .map(|p| (p.as_str(), "RAP_PARAMETER_MODE_IN"))
                .collect::<Vec<_>>(),
        );
    }

    fn emit_procedure_def(&mut self, proc_def: &ProcedureDefinition) {
        let name = proc_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);

        // Same two-step as emit_function_def
        let saved_output = std::mem::take(&mut self.output);
        let saved_indent = self.indent_level;
        let saved_temp = self.temp_counter;
        let saved_string = self.string_counter;
        let saved_frame = std::mem::replace(&mut self.current_frame, "_frame".to_string());

        self.indent_level = 1;
        self.temp_counter = 0;
        self.string_counter = 0;

        // Unpack parameters
        for (i, param) in proc_def.parameters.iter().enumerate() {
            let param_name = match param {
                ProcParameter::Input(n) | ProcParameter::InOut(n) => n,
            };
            let mangled = self.mangle_name(param_name);
            self.emit_line(&format!("RAP_Object *{} = _args[{}];", mangled, i));
        }
        if !proc_def.parameters.is_empty() {
            self.emit_blank_line();
        }

        self.emit_statement_list(&proc_def.body);

        let func_body = std::mem::take(&mut self.output);
        self.forward_decls.push_str(&format!("// проц {}\n", name));
        self.forward_decls.push_str(&format!(
            "RAP_Object *{}(struct RAP_CallFrame *_frame,\n",
            c_func_name
        ));
        let align = format!("RAP_Object *{}(", c_func_name).len();
        self.forward_decls.push_str(&format!(
            "{:>width$}RAP_Object **_args, unsigned int _argc) {{\n",
            "",
            width = align
        ));
        self.forward_decls.push_str(&func_body);
        self.forward_decls.push_str("}\n\n");

        self.output = saved_output;
        self.indent_level = saved_indent;
        self.temp_counter = saved_temp;
        self.string_counter = saved_string;
        self.current_frame = saved_frame;

        let params: Vec<(&str, &str)> = proc_def
            .parameters
            .iter()
            .map(|p| match p {
                ProcParameter::Input(n) => (n.as_str(), "RAP_PARAMETER_MODE_IN"),
                ProcParameter::InOut(n) => (n.as_str(), "RAP_PARAMETER_MODE_OUT"),
            })
            .collect();
        self.emit_callable_registration(name, &c_func_name, &params);
    }

    /// Emit the RAP_Parameter + RAP_create_callable_obj lines to register
    /// a function/procedure as a callable in the current scope.
    fn emit_callable_registration(
        &mut self,
        rapira_name: &str,
        c_func_name: &str,
        params: &[(&str, &str)], // (rapira_param_name, C_mode_constant)
    ) {
        let param_count = params.len();

        if param_count == 0 {
            self.emit_line(&format!(
                "RAP_Object *{} = RAP_create_callable_obj(&{}, &{}, NULL, 0);",
                self.mangle_name(rapira_name),
                self.current_frame,
                c_func_name
            ));
        } else if param_count == 1 {
            let p = self.fresh_param();
            self.emit_line(&format!(
                "RAP_Parameter *{} = RAP_create_parameter({}, \"{}\");",
                p, params[0].1, params[0].0
            ));
            self.emit_line(&format!("RAP_Object *{} =", self.mangle_name(rapira_name)));
            self.emit_line(&format!(
                "    RAP_create_callable_obj(&{}, &{}, &{}, {});",
                self.current_frame, c_func_name, p, param_count
            ));
        } else {
            let mut param_var_names = Vec::new();
            for (rapira_param_name, mode) in params {
                let p = self.fresh_param();
                self.emit_line(&format!(
                    "RAP_Parameter *{} = RAP_create_parameter({}, \"{}\");",
                    p, mode, rapira_param_name
                ));
                param_var_names.push(p);
            }
            let array_name = format!("_params_{}", Self::transliterate(rapira_name));
            self.emit_line(&format!(
                "RAP_Parameter *{}[] = {{{}}};",
                array_name,
                param_var_names.join(", ")
            ));
            self.emit_line(&format!("RAP_Object *{} =", self.mangle_name(rapira_name)));
            self.emit_line(&format!(
                "    RAP_create_callable_obj(&{}, &{}, {}, {});",
                self.current_frame, c_func_name, array_name, param_count
            ));
        }
        self.emit_blank_line();
    }

    // ── Statements ───────────────────────────────────────────────────────

    fn emit_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Empty => {}

            Statement::Assignment { target, value } => {
                let value_temp = self.emit_expression(value);
                self.emit_lvalue_assignment(target, &value_temp);
            }

            Statement::ProcedureCall {
                procedure,
                arguments,
            } => {
                let proc_temp = self.emit_expression(procedure);
                let arg_temps: Vec<String> = arguments
                    .iter()
                    .map(|arg| match arg {
                        CallArgument::Input(expr) => self.emit_expression(expr),
                        CallArgument::InOut(lvalue) => {
                            // For in-out params, pass the variable itself
                            match lvalue {
                                LValue::Name(n) => self.mangle_name(n),
                                _ => {
                                    // TODO: subscript/slice in-out params
                                    "_NULL_TODO".to_string()
                                }
                            }
                        }
                    })
                    .collect();
                self.emit_call_discard(&proc_temp, &arg_temps);
            }

            Statement::Conditional {
                condition,
                then_body,
                else_body,
            } => {
                let cond_temp = self.emit_expression(condition);
                self.emit_line(&format!("if ({}->logical_val) {{", cond_temp));
                self.indent_level += 1;
                self.emit_statement_list(then_body);
                self.indent_level -= 1;
                if let Some(else_stmts) = else_body {
                    self.emit_line("} else {");
                    self.indent_level += 1;
                    self.emit_statement_list(else_stmts);
                    self.indent_level -= 1;
                }
                self.emit_line("}");
            }

            Statement::Selection(sel) => self.emit_selection(sel),

            Statement::Loop(loop_stmt) => self.emit_loop(loop_stmt),

            Statement::Output { no_newline, values } => {
                for value_expr in values {
                    let val_temp = self.emit_expression(value_expr);
                    let str_buf = self.fresh_string_buf();
                    self.emit_line(&format!(
                        "char *{} = RAP_stringify_object({});",
                        str_buf, val_temp
                    ));
                    self.emit_line(&format!("printf(\"%s\", {});", str_buf));
                }
                if !no_newline {
                    self.emit_line("printf(\"\\n\");");
                }
            }

            Statement::Input { .. } => {
                self.emit_line("// TODO: ввод (input) — needs runtime support");
            }

            Statement::ExitLoop => {
                self.emit_line("break;");
            }

            Statement::ReturnFromProcedure => {
                self.emit_line("return NULL;");
            }

            Statement::ReturnFromFunction(expr) => {
                let result_temp = self.emit_expression(expr);
                self.emit_line(&format!("return {};", result_temp));
            }
        }
    }

    fn emit_lvalue_assignment(&mut self, target: &LValue, value_temp: &str) {
        match target {
            LValue::Name(name) => {
                let mangled = self.mangle_name(name);
                self.emit_line(&format!("{} = {};", mangled, value_temp));
            }
            LValue::Subscript { collection, index } => {
                let coll_temp = self.emit_expression(collection);
                let idx_temp = self.emit_expression(index);
                self.emit_line(&format!(
                    "RAP_set_tuple_item({}, (uint32_t)RAP_get_int_val({}), {});",
                    coll_temp, idx_temp, value_temp
                ));
            }
            LValue::Slice { .. } => {
                self.emit_line("// TODO: slice assignment — needs runtime support");
            }
        }
    }

    /// Emit a call whose return value is discarded (procedure call statement).
    fn emit_call_discard(&mut self, callable_temp: &str, arg_temps: &[String]) {
        let arg_count = arg_temps.len();
        if arg_count == 0 {
            self.emit_line(&format!(
                "RAP_call_callable_obj({}, NULL, 0);",
                callable_temp
            ));
        } else if arg_count == 1 {
            self.emit_line(&format!(
                "RAP_call_callable_obj({}, &{}, 1);",
                callable_temp, arg_temps[0]
            ));
        } else {
            let args_array = format!("_call_args_{}", self.temp_counter);
            self.emit_line(&format!(
                "RAP_Object *{}[] = {{{}}};",
                args_array,
                arg_temps.join(", ")
            ));
            self.emit_line(&format!(
                "RAP_call_callable_obj({}, {}, {});",
                callable_temp, args_array, arg_count
            ));
        }
    }

    // ── Selection ────────────────────────────────────────────────────────

    fn emit_selection(&mut self, sel: &SelectionStatement) {
        match sel {
            SelectionStatement::ValueMatch {
                expression,
                cases,
                else_body,
            } => {
                let sel_temp = self.emit_expression(expression);
                for (i, case) in cases.iter().enumerate() {
                    // Build OR-chain: if (sel == v1 || sel == v2 || ...)
                    let mut conditions = Vec::new();
                    for val_expr in &case.values {
                        let val_temp = self.emit_expression(val_expr);
                        conditions.push(format!(
                            "RAP_integer_equal({}, {})->logical_val",
                            sel_temp, val_temp
                        ));
                    }
                    let keyword = if i == 0 { "if" } else { "} else if" };
                    self.emit_line(&format!("{} ({}) {{", keyword, conditions.join(" || ")));
                    self.indent_level += 1;
                    self.emit_statement_list(&case.body);
                    self.indent_level -= 1;
                }
                if let Some(else_stmts) = else_body {
                    self.emit_line("} else {");
                    self.indent_level += 1;
                    self.emit_statement_list(else_stmts);
                    self.indent_level -= 1;
                }
                self.emit_line("}");
            }

            SelectionStatement::ConditionList { cases, else_body } => {
                for (i, case) in cases.iter().enumerate() {
                    let cond_temp = self.emit_expression(&case.condition);
                    let keyword = if i == 0 { "if" } else { "} else if" };
                    self.emit_line(&format!("{} ({}->logical_val) {{", keyword, cond_temp));
                    self.indent_level += 1;
                    self.emit_statement_list(&case.body);
                    self.indent_level -= 1;
                }
                if let Some(else_stmts) = else_body {
                    self.emit_line("} else {");
                    self.indent_level += 1;
                    self.emit_statement_list(else_stmts);
                    self.indent_level -= 1;
                }
                self.emit_line("}");
            }
        }
    }

    // ── Loops ────────────────────────────────────────────────────────────

    fn emit_loop(&mut self, loop_stmt: &LoopStatement) {
        match &loop_stmt.header {
            LoopHeader::For {
                variable,
                from,
                to,
                step,
            } => {
                // Emit from/to/step expressions before the loop
                let from_temp = if let Some(from_expr) = from {
                    self.emit_expression(from_expr)
                } else {
                    let t = self.fresh_temp();
                    self.emit_line(&format!("RAP_Object *{} = RAP_create_int_obj(1);", t));
                    t
                };

                let to_temp = to.as_ref().map(|to_expr| self.emit_expression(to_expr));

                let step_val = if let Some(step_expr) = step {
                    let t = self.emit_expression(step_expr);
                    format!("RAP_get_int_val({})", t)
                } else {
                    "1".to_string()
                };

                let iter_var = format!("_iter_{}", Self::transliterate(variable));
                let local_var = self.mangle_name(variable);

                // Extract upper limit as int64_t, handling both int and float bounds
                let limit_var = format!("_for_limit_{}", Self::transliterate(variable));
                if let Some(ref to_t) = to_temp {
                    self.emit_line(&format!(
                        "int64_t {} = ({}->tag == RAP_OBJECT_TAG_INT) \
                         ? RAP_get_int_val({}) : (int64_t)RAP_get_float_val({});",
                        limit_var, to_t, to_t, to_t
                    ));
                }

                let condition = if to_temp.is_some() {
                    format!("{} <= {}", iter_var, limit_var)
                } else {
                    // для X от 1 (no upper bound — infinite, controlled by выход)
                    "1".to_string()
                };

                self.emit_line(&format!(
                    "for (int64_t {} = RAP_get_int_val({}); {}; {} += {}) {{",
                    iter_var, from_temp, condition, iter_var, step_val
                ));
                self.indent_level += 1;
                self.emit_line(&format!(
                    "RAP_Object *{} = RAP_create_int_obj({});",
                    local_var, iter_var
                ));
                self.emit_blank_line();
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.indent_level -= 1;
                self.emit_line("}");
            }

            LoopHeader::Repeat(count_expr) => {
                let count_temp = self.emit_expression(count_expr);
                let rep_var = format!("_rep_{}", self.temp_counter);
                self.emit_line(&format!(
                    "for (int64_t {} = 0; {} < RAP_get_int_val({}); {}++) {{",
                    rep_var, rep_var, count_temp, rep_var
                ));
                self.indent_level += 1;
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.indent_level -= 1;
                self.emit_line("}");
            }

            LoopHeader::Infinite => {
                self.emit_line("while (1) {");
                self.indent_level += 1;
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.indent_level -= 1;
                self.emit_line("}");
            }
        }
    }

    /// Emit `пока` (while) pre-condition: `if (!cond) break;` at top of loop body.
    fn emit_while_condition(&mut self, while_cond: &Option<Box<Expr>>) {
        if let Some(cond_expr) = while_cond {
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if (!{}->logical_val) break;", cond_temp));
        }
    }

    /// Emit `по` (until) post-condition: `if (cond) break;` at bottom of loop body.
    fn emit_post_condition(&mut self, post_cond: &Option<Box<Expr>>) {
        if let Some(cond_expr) = post_cond {
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if ({}->logical_val) break;", cond_temp));
        }
    }

    // ── Expressions ──────────────────────────────────────────────────────

    /// Emit code for an expression. Returns the temp variable name holding the result.
    fn emit_expression(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(lit) => self.emit_literal(lit),

            Expr::Name(name) => {
                // No temp needed — just reference the local variable directly
                self.mangle_name(name)
            }

            Expr::BinaryOp {
                operator,
                left,
                right,
            } => {
                let left_temp = self.emit_expression(left);
                let right_temp = self.emit_expression(right);
                self.emit_binary_op(operator, &left_temp, &right_temp)
            }

            Expr::UnaryOp { operator, operand } => {
                let operand_temp = self.emit_expression(operand);
                self.emit_unary_op(operator, &operand_temp)
            }

            Expr::FunctionCall {
                function,
                arguments,
            } => self.emit_function_call(function, arguments),

            Expr::TupleConstruct(items) => self.emit_tuple_construct(items),

            Expr::Subscript { collection, index } => {
                let coll_temp = self.emit_expression(collection);
                let idx_temp = self.emit_expression(index);
                let result = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Object *{} = RAP_get_tuple_item({}, (uint32_t)RAP_get_int_val({}));",
                    result, coll_temp, idx_temp
                ));
                result
            }

            Expr::Slice { .. } => {
                let result = self.fresh_temp();
                self.emit_line(&format!("RAP_Object *{} = NULL; // TODO: slice", result));
                result
            }
        }
    }

    fn emit_literal(&mut self, lit: &Literal) -> String {
        let temp = self.fresh_temp();
        let rhs = match lit {
            Literal::Null => "RAP_create_null_obj()".to_string(),
            Literal::Boolean(b) => format!("RAP_create_logical_obj({})", b),
            Literal::Integer(n) => format!("RAP_create_int_obj({})", n),
            Literal::Real(f) => format!("RAP_create_float_obj({:?})", f),
            Literal::Text(s) => format!("RAP_create_text_obj(\"{}\")", escape_c_string(s)),
        };
        self.emit_line(&format!("RAP_Object *{} = {};", temp, rhs));
        temp
    }

    fn emit_binary_op(&mut self, op: &BinaryOperator, left: &str, right: &str) -> String {
        let temp = self.fresh_temp();
        // Phase 1: integer-only arithmetic. TODO: add runtime polymorphic
        // helpers (RAP_add, RAP_subtract, etc.) that dispatch on tag for
        // mixed int/float/text operations.
        let rhs = match op {
            BinaryOperator::Add => format!(
                "RAP_create_int_obj(RAP_get_int_val({}) + RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::Subtract => format!(
                "RAP_create_int_obj(RAP_get_int_val({}) - RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::Multiply => format!(
                "RAP_create_int_obj(RAP_get_int_val({}) * RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::Divide => format!(
                "RAP_create_float_obj((double)RAP_get_int_val({}) / (double)RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::IntegerDivide => format!(
                "RAP_create_int_obj(RAP_get_int_val({}) / RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::Remainder => {
                format!("RAP_integer_modulo({}, {})", left, right)
            }
            BinaryOperator::Power => format!(
                "RAP_create_float_obj(pow((double)RAP_get_int_val({}), (double)RAP_get_int_val({})))",
                left, right
            ),
            BinaryOperator::Less => {
                format!("RAP_integer_less_than({}, {})", left, right)
            }
            BinaryOperator::Greater => {
                format!("RAP_integer_greater_than({}, {})", left, right)
            }
            BinaryOperator::LessOrEqual => format!(
                "RAP_create_logical_obj(RAP_get_int_val({}) <= RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::GreaterOrEqual => format!(
                "RAP_create_logical_obj(RAP_get_int_val({}) >= RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::Equal => {
                format!("RAP_integer_equal({}, {})", left, right)
            }
            BinaryOperator::NotEqual => format!(
                "RAP_create_logical_obj(RAP_get_int_val({}) != RAP_get_int_val({}))",
                left, right
            ),
            BinaryOperator::And => format!(
                "RAP_create_logical_obj({}->logical_val && {}->logical_val)",
                left, right
            ),
            BinaryOperator::Or => format!(
                "RAP_create_logical_obj({}->logical_val || {}->logical_val)",
                left, right
            ),
        };
        self.emit_line(&format!("RAP_Object *{} = {};", temp, rhs));
        temp
    }

    fn emit_unary_op(&mut self, op: &UnaryOperator, operand: &str) -> String {
        let temp = self.fresh_temp();
        let rhs = match op {
            UnaryOperator::Negate => format!("RAP_create_int_obj(-RAP_get_int_val({}))", operand),
            UnaryOperator::Plus => {
                // No-op: just alias the operand
                return operand.to_string();
            }
            UnaryOperator::Not => format!("RAP_create_logical_obj(!{}->logical_val)", operand),
            UnaryOperator::Length => format!(
                // # operator — works on text (strlen) and tuples (count)
                // TODO: runtime helper RAP_length(obj)
                "RAP_create_int_obj(({0}->tag == RAP_OBJECT_TAG_TEXT) \
                 ? (int64_t)strlen(RAP_get_text_val({0})) \
                 : (int64_t){0}->tuple_val->count)",
                operand
            ),
        };
        self.emit_line(&format!("RAP_Object *{} = {};", temp, rhs));
        temp
    }

    fn emit_function_call(&mut self, function: &Expr, arguments: &[Box<Expr>]) -> String {
        // Check for built-in functions by name
        if let Expr::Name(name) = function {
            if let Some(result) = self.try_emit_builtin(name, arguments) {
                return result;
            }
        }

        // General case: callable dispatch
        let func_temp = self.emit_expression(function);
        let arg_temps: Vec<String> = arguments
            .iter()
            .map(|arg| self.emit_expression(arg))
            .collect();

        let result = self.fresh_temp();
        let arg_count = arg_temps.len();

        if arg_count == 0 {
            self.emit_line(&format!(
                "RAP_Object *{} = RAP_call_callable_obj({}, NULL, 0);",
                result, func_temp
            ));
        } else if arg_count == 1 {
            self.emit_line(&format!(
                "RAP_Object *{} = RAP_call_callable_obj({}, &{}, 1);",
                result, func_temp, arg_temps[0]
            ));
        } else {
            let args_array = format!("_call_args_{}", self.temp_counter);
            self.emit_line(&format!(
                "RAP_Object *{}[] = {{{}}};",
                args_array,
                arg_temps.join(", ")
            ));
            self.emit_line(&format!(
                "RAP_Object *{} = RAP_call_callable_obj({}, {}, {});",
                result, func_temp, args_array, arg_count
            ));
        }
        result
    }

    /// Try to emit a built-in function call. Returns Some(temp_name) if handled.
    fn try_emit_builtin(&mut self, name: &str, arguments: &[Box<Expr>]) -> Option<String> {
        match name {
            "корень" => {
                // sqrt — single argument, returns float
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Object *{t} = RAP_create_float_obj(\
                     sqrt(({a}->tag == RAP_OBJECT_TAG_INT) \
                     ? (double)RAP_get_int_val({a}) : RAP_get_float_val({a})));",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "абс" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Object *{t} = ({a}->tag == RAP_OBJECT_TAG_INT) \
                     ? RAP_create_int_obj(llabs(RAP_get_int_val({a}))) \
                     : RAP_create_float_obj(fabs(RAP_get_float_val({a})));",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "целый" => {
                // Truncate to integer
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Object *{} = RAP_create_int_obj(\
                     (int64_t)RAP_get_float_val({}));",
                    temp, arg
                ));
                Some(temp)
            }
            "длин" => {
                // Length of text or tuple (same as # operator)
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Object *{t} = RAP_create_int_obj(\
                     ({a}->tag == RAP_OBJECT_TAG_TEXT) \
                     ? (int64_t)strlen(RAP_get_text_val({a})) \
                     : (int64_t){a}->tuple_val->count);",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            _ => None,
        }
    }

    fn emit_tuple_construct(&mut self, items: &[Box<Expr>]) -> String {
        let item_temps: Vec<String> = items
            .iter()
            .map(|item| self.emit_expression(item))
            .collect();

        let count = item_temps.len();
        let items_array = format!("_tuple_items_{}", self.temp_counter);
        self.emit_line(&format!(
            "RAP_Object *{}[] = {{{}}};",
            items_array,
            item_temps.join(", ")
        ));
        let result = self.fresh_temp();
        self.emit_line(&format!(
            "RAP_Object *{} = RAP_create_tuple_obj({}, {});",
            result, count, items_array
        ));
        result
    }
}
