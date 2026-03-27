//! AST → C code generator.
//!
//! Walks the AST and emits C code that uses the runtime library (runtime.h).

use std::collections::{HashMap, HashSet};

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
            c if c.is_ascii_whitespace() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Info needed to create a callable object at any call site.
struct CallableInfo {
    c_func_name: String,
    /// (rapira_param_name, c_mode_constant)
    params: Vec<(String, String)>,
    is_function: bool,
}

/// A stack of declared variables in scope
type ScopeVariablesStack = Vec<HashSet<String>>;

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
    /// Stack of scopes — each scope holds the set of C variable names (mangled) declared in it.
    /// push on scope entry (function, loop, if), pop on scope exit.
    declared_vars: ScopeVariablesStack,
    /// Temps created during expression evaluation that own a new value and need dec_ref at statement end.
    statement_temps: Vec<String>,
    /// Rapira name → callable info, so call sites can create callables inline
    known_callables: HashMap<String, CallableInfo>,
    /// Variables declared as чужие in current function scope (original Rapira names).
    /// Empty at top level and inside functions without чужие.
    foreign_vars: HashSet<String>,
    /// True when emitting inside a function/procedure body (use frame-based access for non-param locals)
    inside_function: bool,
    /// Emit RAP_check_leaks() call and #define RAP_TEST_LEAKS
    check_leaks: bool,
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
            current_frame: "&_main_frame".to_string(),
            declared_vars: Vec::new(),
            statement_temps: Vec::new(),
            known_callables: HashMap::new(),
            foreign_vars: HashSet::new(),
            inside_function: false,
            check_leaks: false,
        }
    }

    /// Enable leak checking: emits `#define RAP_TEST_LEAKS` and `RAP_check_leaks()` before exit.
    pub fn with_check_leaks(mut self, enabled: bool) -> Self {
        self.check_leaks = enabled;
        self
    }

    /// Insert a variable into the current (top) scope's declared vars set.
    fn insert_declared_var(&mut self, var: String) {
        if let Some(scope) = self.declared_vars.last_mut() {
            scope.insert(var);
        }
    }

    fn is_var_in_scope(&self, var: &str) -> bool {
        self.declared_vars.iter().any(|scope| scope.contains(var))
    }

    /// Record a temp as needing dec_ref at statement end.
    fn track_temp(&mut self, temp: &str) {
        self.statement_temps.push(temp.to_string());
    }

    /// Emit dec_ref for all tracked temps, then clear the list.
    /// `exclude` is the temp whose ownership was transferred (e.g. assigned to a named var).
    fn flush_statement_temps(&mut self, exclude: Option<&str>) {
        let temps = std::mem::take(&mut self.statement_temps);
        for temp in &temps {
            if temp != exclude.unwrap_or_default() {
                self.emit_line(&format!("RAP_dec_ref({});", temp));
            }
            // if exclude.map_or(true, |ex| ex != temp) {
            //     self.emit_line(&format!("RAP_dec_ref({});", temp));
            // }
        }
    }

    fn create_scope(&mut self) {
        self.declared_vars.push(std::collections::HashSet::new());
    }

    fn drop_scope(&mut self) {
        if self.declared_vars.is_empty() {
            return; // TODO: error
        }
        let vars_in_scope = self.declared_vars.pop().unwrap(); // TODO: error

        // Decrement reference count for all variables in scope
        for var in vars_in_scope {
            self.emit_line(&format!("RAP_dec_ref({});", var));
        }
    }

    /// Emit dec_ref for all variables across all function scopes (used before return).
    /// Covers everything from the current nested scope up to the function scope,
    /// since outer (caller) scopes are saved/restored by emit_function_def.
    fn emit_epilogue(&mut self) {
        let all_vars: Vec<String> = self
            .declared_vars
            .iter()
            .flat_map(|scope| scope.iter().cloned()) // TODO: cloned
            .collect();
        for var in all_vars {
            self.emit_line(&format!("RAP_dec_ref({});", var));
        }
    }

    /// Main entry point: walk the whole program, return generated C source.
    pub fn generate(mut self, program: &Program) -> String {
        self.create_scope(); // main scope
        for unit in &program.units {
            self.emit_program_unit(unit);
        }

        // Assemble: prelude → forward decls → main() { body }
        let mut result = String::new();
        if self.check_leaks {
            result.push_str("#define RAP_TEST_LEAKS\n");
        }
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
        result.push_str("  RAP_free_main_frame(&_main_frame);\n");
        if self.check_leaks {
            // Decref all locals declared in main scope
            let vars_in_scope = self.declared_vars.pop();
            if let Some(vars) = vars_in_scope {
                for var in &vars {
                    result.push_str(&format!("  RAP_dec_ref({});\n", var));
                }
            }
            result.push_str("  RAP_check_leaks();\n");
        }
        result.push_str("  return 0;\n");
        result.push_str("}\n");
        result
    }

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

    /// Transliterate Cyrillic identifier to Latin
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

    fn emit_program_unit(&mut self, unit: &ProgramUnit) {
        match unit {
            ProgramUnit::FunctionDefinition(func_def) => self.emit_function_def(func_def),
            ProgramUnit::ProcedureDefinition(proc_def) => self.emit_procedure_def(proc_def),
            ProgramUnit::Statement(stmt) => self.emit_statement(stmt),
        }
    }

    fn emit_function_def(&mut self, func_def: &FunctionDefinition) {
        let name = func_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);
        self.known_callables.insert(
            name.to_string(),
            CallableInfo {
                c_func_name: c_func_name.clone(),
                params: func_def
                    .parameters
                    .iter()
                    .map(|p| (p.clone(), "RAP_PARAMETER_MODE_IN".to_string()))
                    .collect(),
                is_function: true,
            },
        );

        // 1. Emit C function body into forward_decls.
        //    Save current codegen state, emit function body, then restore.
        let saved_output = std::mem::take(&mut self.output);
        let saved_indent = self.indent_level;
        let saved_temp = self.temp_counter;
        let saved_string = self.string_counter;
        let saved_frame = std::mem::replace(&mut self.current_frame, "_frame".to_string());
        let saved_declared = std::mem::take(&mut self.declared_vars);
        let saved_foreign = std::mem::replace(
            &mut self.foreign_vars,
            func_def
                .name_declarations
                .foreign_names
                .iter()
                .cloned()
                .collect(),
        );
        let saved_inside = std::mem::replace(&mut self.inside_function, true);

        self.indent_level = 1;
        self.temp_counter = 0;
        self.string_counter = 0;

        // Creates empty declared vars set for this function scope
        self.create_scope();

        // Unpack parameters from _args array — mark them as declared.
        // Incref each param so the function owns its reference.
        for (i, param_name) in func_def.parameters.iter().enumerate() {
            let mangled = self.mangle_name(param_name);
            self.insert_declared_var(mangled.clone());
            self.emit_line(&format!("RAP_Value {} = _args[{}];", mangled, i));
            self.emit_line(&format!("RAP_inc_ref({});", mangled));
        }
        if !func_def.parameters.is_empty() {
            self.emit_blank_line();
        }

        self.emit_statement_list(&func_def.body);

        self.drop_scope();

        // Wrap in C function signature
        let func_body = std::mem::take(&mut self.output);
        self.forward_decls.push_str(&format!("// функ {}\n", name));
        self.forward_decls.push_str(&format!(
            "RAP_Value {}(struct RAP_CallFrame *_frame,\n",
            c_func_name
        ));
        let align = format!("RAP_Value {}(", c_func_name).len();
        self.forward_decls.push_str(&format!(
            "{:>width$}RAP_Value *_args, unsigned int _argc) {{\n",
            "",
            width = align
        ));
        self.forward_decls.push_str(&func_body);

        // TODO: no return statment in func

        self.forward_decls.push_str("  return 0;\n}\n\n");

        // Restore state
        self.output = saved_output;
        self.indent_level = saved_indent;
        self.temp_counter = saved_temp;
        self.string_counter = saved_string;
        self.current_frame = saved_frame;
        self.declared_vars = saved_declared;
        self.foreign_vars = saved_foreign;
        self.inside_function = saved_inside;
    }

    fn emit_procedure_def(&mut self, proc_def: &ProcedureDefinition) {
        let name = proc_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);
        self.known_callables.insert(
            name.to_string(),
            CallableInfo {
                c_func_name: c_func_name.clone(),
                params: proc_def
                    .parameters
                    .iter()
                    .map(|p| match p {
                        ProcParameter::Input(n) => (n.clone(), "RAP_PARAMETER_MODE_IN".to_string()),
                        ProcParameter::InOut(n) => {
                            (n.clone(), "RAP_PARAMETER_MODE_OUT".to_string())
                        }
                    })
                    .collect(),
                is_function: false,
            },
        );

        // Same two-step as emit_function_def
        let saved_output = std::mem::take(&mut self.output);
        let saved_indent = self.indent_level;
        let saved_temp = self.temp_counter;
        let saved_string = self.string_counter;
        let saved_frame = std::mem::replace(&mut self.current_frame, "_frame".to_string());
        let saved_declared = std::mem::take(&mut self.declared_vars);
        // In-out params are treated as чужие — they read/write the caller's frame
        // They act exactly the same as foreign variables!
        let mut foreign: std::collections::HashSet<String> = proc_def
            .name_declarations
            .foreign_names
            .iter()
            .cloned()
            .collect();
        for param in &proc_def.parameters {
            if let ProcParameter::InOut(n) = param {
                foreign.insert(n.clone());
            }
        }
        let saved_foreign = std::mem::replace(&mut self.foreign_vars, foreign);
        let saved_inside = std::mem::replace(&mut self.inside_function, true);

        self.indent_level = 1;
        self.temp_counter = 0;
        self.string_counter = 0;

        // Creates empty declared vars set for this procedure scope
        self.create_scope();

        // Unpack only input parameters — in-out params use frame lookup
        let mut arg_index = 0;
        for param in &proc_def.parameters {
            match param {
                ProcParameter::Input(n) => {
                    let mangled = self.mangle_name(n);
                    self.insert_declared_var(mangled.clone());
                    self.emit_line(&format!("RAP_Value {} = _args[{}];", mangled, arg_index));
                    self.emit_line(&format!("RAP_inc_ref({});", mangled));
                    arg_index += 1;
                }
                ProcParameter::InOut(_) => {
                    // Skipped — accessed via frame_get_foreign/frame_set_foreign
                }
            }
        }
        if arg_index > 0 {
            self.emit_blank_line();
        }

        self.emit_statement_list(&proc_def.body);

        self.drop_scope();

        let func_body = std::mem::take(&mut self.output);
        self.forward_decls.push_str(&format!("// проц {}\n", name));
        self.forward_decls.push_str(&format!(
            "RAP_Value {}(struct RAP_CallFrame *_frame,\n",
            c_func_name
        ));
        let align = format!("RAP_Value {}(", c_func_name).len();
        self.forward_decls.push_str(&format!(
            "{:>width$}RAP_Value *_args, unsigned int _argc) {{\n",
            "",
            width = align
        ));
        self.forward_decls.push_str(&func_body);

        self.forward_decls.push_str("  return 0;\n}\n\n");

        self.output = saved_output;
        self.indent_level = saved_indent;
        self.temp_counter = saved_temp;
        self.string_counter = saved_string;
        self.current_frame = saved_frame;
        self.declared_vars = saved_declared;
        self.foreign_vars = saved_foreign;
        self.inside_function = saved_inside;
    }

    /// Emit inline callable creation, returns the temp variable name.
    fn emit_inline_callable(
        &mut self,
        c_func_name: &str,
        params: &[(String, String)],
        is_function: bool,
    ) -> String {
        let temp = self.fresh_temp();
        let param_count = params.len();
        let is_func_str = if is_function { "true" } else { "false" };

        if param_count == 0 {
            self.emit_line(&format!(
                "RAP_Value {} = RAP_create_callable_obj({}, &{}, NULL, 0, {});",
                temp, self.current_frame, c_func_name, is_func_str
            ));
        } else if param_count == 1 {
            let p = self.fresh_param();
            self.emit_line(&format!(
                "RAP_Parameter *{} = RAP_create_parameter({}, \"{}\");",
                p, params[0].1, params[0].0
            ));
            self.emit_line(&format!(
                "RAP_Value {} = RAP_create_callable_obj({}, &{}, &{}, {}, {});",
                temp, self.current_frame, c_func_name, p, param_count, is_func_str
            ));
        } else {
            let mut param_var_names = Vec::new();
            for (param_name, mode) in params {
                let p = self.fresh_param();
                self.emit_line(&format!(
                    "RAP_Parameter *{} = RAP_create_parameter({}, \"{}\");",
                    p, mode, param_name
                ));
                param_var_names.push(p);
            }
            let array_name = format!("_params_{}", self.temp_counter);
            self.emit_line(&format!(
                "RAP_Parameter *{}[] = {{{}}};",
                array_name,
                param_var_names.join(", ")
            ));
            self.emit_line(&format!(
                "RAP_Value {} = RAP_create_callable_obj({}, &{}, {}, {}, {});",
                temp, self.current_frame, c_func_name, array_name, param_count, is_func_str
            ));
        }
        // TODO
        self.track_temp(&temp);
        temp
    }

    fn emit_statement(&mut self, stmt: &Statement) {
        // Each statement gets its own temp tracking context.
        // Inner statements (in if/loop bodies) save and restore, so they don't
        // interfere with the outer statement's temps.
        let saved_temps = std::mem::take(&mut self.statement_temps);

        match stmt {
            Statement::Empty => {}

            Statement::Assignment { target, value } => {
                let value_temp = self.emit_expression(value);

                // Decrement refcount of old value in target (if reassigning a known variable)
                if let LValue::Name(name) = target {
                    let mangled = self.mangle_name(name);
                    if self.is_var_in_scope(&mangled) {
                        self.emit_line(&format!("RAP_dec_ref({});", mangled));
                    }
                }

                // Increment refcount when assigning from another variable (shared reference)
                if let Expr::Name(_) = value.as_ref() {
                    // Do not increment refcount if value is taken from frame,
                    // since frame_get increments count on itself
                    if self.output.lines().last().is_some()
                        && !self.output.lines().last().unwrap().contains("frame_get")
                    {
                        self.emit_line(&format!("RAP_inc_ref({});", value_temp));
                    }
                }

                self.emit_lvalue_assignment(target, &value_temp);

                // For name assignments, ownership transfers to the C local — don't dec_ref the value.
                // For subscript/slice, the runtime inc_refs internally, so all temps can be freed.
                let exclude = match target {
                    LValue::Name(_) => Some(value_temp.as_str()),
                    _ => None,
                };
                self.flush_statement_temps(exclude);
            }

            Statement::ProcedureCall {
                procedure,
                arguments,
            } => {
                // Collect in-out param info: (proc_param_name, caller_var_name)
                let inout_pairs: Vec<(String, String)> = if let Expr::Name(proc_name) =
                    procedure.as_ref()
                {
                    if let Some(info) = self.known_callables.get(proc_name.as_str()) {
                        info.params
                            .iter()
                            .zip(arguments.iter())
                            .filter_map(|((param_name, mode), arg)| {
                                if mode == "RAP_PARAMETER_MODE_OUT" {
                                    if let CallArgument::InOut(LValue::Name(caller_name)) = arg {
                                        Some((param_name.clone(), caller_name.clone()))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                // Before call: copy caller's variables into frame under proc's param names
                for (param_name, caller_name) in &inout_pairs {
                    let val_temp = self.fresh_temp();
                    self.emit_line(&format!(
                        "RAP_Value {} = RAP_frame_get({}, \"{}\");",
                        val_temp, self.current_frame, caller_name
                    ));
                    self.emit_line(&format!(
                        "RAP_frame_set({}, \"{}\", {});",
                        self.current_frame, param_name, val_temp
                    ));
                    // Also incref inout pararms
                    self.emit_line(&format!("RAP_inc_ref({});", val_temp));
                }

                let proc_temp = self.emit_expression(procedure);
                // Only pass input args; in-out params use frame lookup
                let arg_temps: Vec<String> = arguments
                    .iter()
                    .filter_map(|arg| match arg {
                        CallArgument::Input(expr) => Some(self.emit_expression(expr)),
                        CallArgument::InOut(_) => None,
                    })
                    .collect();
                self.emit_call_discard(&proc_temp, &arg_temps);

                // After call: copy back proc's param names to caller's variable names
                for (param_name, caller_name) in &inout_pairs {
                    let val_temp = self.fresh_temp();
                    self.emit_line(&format!(
                        "RAP_Value {} = RAP_frame_get({}, \"{}\");",
                        val_temp, self.current_frame, param_name
                    ));
                    self.emit_line(&format!(
                        "RAP_frame_set({}, \"{}\", {});",
                        self.current_frame, caller_name, val_temp
                    ));
                }

                self.flush_statement_temps(None);
            }

            Statement::Conditional {
                condition,
                then_body,
                else_body,
            } => {
                let cond_temp = self.emit_expression(condition);
                self.emit_line(&format!("if (RAP_BOOL_VALUE({})) {{", cond_temp));

                self.create_scope();

                self.indent_level += 1;
                self.emit_statement_list(then_body);
                self.drop_scope();
                self.indent_level -= 1;
                if let Some(else_stmts) = else_body {
                    self.emit_line("} else {");
                    self.create_scope();
                    self.indent_level += 1;
                    self.emit_statement_list(else_stmts);
                    self.drop_scope();
                    self.indent_level -= 1;
                }
                self.emit_line("}");
                self.flush_statement_temps(None);
            }

            Statement::Selection(sel) => {
                self.emit_selection(sel);
                self.flush_statement_temps(None);
            }

            Statement::Loop(loop_stmt) => {
                self.emit_loop(loop_stmt);
                // self.flush_statement_temps(None);
                let temps = self.statement_temps.clone();
                for temp in &temps {
                    self.emit_line(&format!("RAP_dec_ref({});", temp));
                }
            }

            Statement::Output { no_newline, values } => {
                for value_expr in values {
                    let val_temp = self.emit_expression(value_expr);
                    let str_buf = self.fresh_string_buf();
                    self.emit_line(&format!(
                        "char *{} = RAP_stringify_object({});",
                        str_buf, val_temp
                    ));
                    self.emit_line(&format!("printf(\"%s\", {});", str_buf));
                    self.emit_line(&format!("free({});", str_buf));
                }
                if !no_newline {
                    self.emit_line("printf(\"\\n\");");
                }
                self.flush_statement_temps(None);
            }

            Statement::Input {
                text_mode,
                variables,
            } => {
                for var in variables {
                    let temp = self.fresh_temp();
                    if *text_mode {
                        self.emit_line(&format!("RAP_Value {} = RAP_input_text();", temp));
                    } else {
                        self.emit_line(&format!("RAP_Value {} = RAP_input_value();", temp));
                    }
                    // Input creates a new value — track it
                    // self.track_temp(&temp);
                    self.emit_lvalue_assignment(var, &temp);
                }
                // Input value ownership transfers to the variable
                // but flush any other intermediate temps
                self.flush_statement_temps(None);
            }

            Statement::ExitLoop => {
                self.emit_line("break;");
                // let temps = saved_temps.clone();
                // for temp in &temps {
                //     self.emit_line(&format!("RAP_dec_ref({});", temp));
                // }
                // self.emit_line("break;");
            }

            Statement::ReturnFromProcedure => {
                // self.flush_statement_temps(None);
                let mut temps = saved_temps.clone();
                temps.append(&mut self.statement_temps.clone());
                for temp in &temps {
                    self.emit_line(&format!("RAP_dec_ref({});", temp));
                }
                self.emit_epilogue();
                self.emit_line("return RAP_create_null_obj();");
            }

            Statement::ReturnFromFunction(expr) => {
                let result_temp = self.emit_expression(expr);
                // Protect return value: inc_ref before epilogue dec_refs all locals
                // self.emit_line(&format!("RAP_inc_ref({});", result_temp));
                // self.flush_statement_temps(None);
                let mut temps = saved_temps.clone();
                temps.append(&mut self.statement_temps.clone());
                for temp in &temps {
                    if *temp != result_temp {
                        self.emit_line(&format!("RAP_dec_ref({});", temp));
                    }
                }
                self.emit_epilogue();
                self.emit_line(&format!("return {};", result_temp));
            }
        }

        self.statement_temps = saved_temps;
    }

    fn emit_lvalue_assignment(&mut self, target: &LValue, value_temp: &str) {
        match target {
            LValue::Name(name) => {
                let mangled = self.mangle_name(name);
                if self.is_var_in_scope(&mangled) {
                    // Parameter updates BOTH C local and frame
                    self.emit_line(&format!("RAP_inc_ref({});", value_temp));
                    self.emit_line(&format!("{} = {};", mangled, value_temp));
                    self.emit_line(&format!(
                        "RAP_frame_set({}, \"{}\", {});",
                        self.current_frame, name, mangled
                    ));
                } else if self.foreign_vars.contains(name.as_str()) {
                    // чужие - here we walk up the frame chain and try to find the variable
                    self.emit_line(&format!(
                        "RAP_frame_set_foreign({}, \"{}\", {});",
                        self.current_frame, name, value_temp
                    ));
                    // Inc ref new value
                    // self.emit_line(&format!("RAP_inc_ref({});", value_temp));
                } else {
                    // Locals saved to frame, to allow access from nested scopes
                    // self.emit_line(&format!("RAP_Value {} = {};", mangled, value_temp));
                    self.emit_line(&format!(
                        "RAP_frame_set({}, \"{}\", {});",
                        self.current_frame, name, value_temp
                    ));
                    // Inc ref new value
                    // self.emit_line(&format!("RAP_nc_ref({});", value_temp));

                    // self.insert_declared_var(mangled.clone());
                }
            }
            LValue::Subscript { collection, index } => {
                let coll_temp = self.emit_expression(collection);
                let idx_temp = self.emit_expression(index);
                self.emit_line(&format!(
                    "RAP_set_tuple_item({}, (uint32_t)RAP_SMI_VALUE({}), {});",
                    coll_temp, idx_temp, value_temp
                ));
            }
            LValue::Slice {
                collection,
                from,
                to,
            } => {
                let coll_temp = self.emit_expression(collection);
                let from_val = if let Some(f) = from {
                    format!("RAP_SMI_VALUE({})", self.emit_expression(f))
                } else {
                    "0".to_string()
                };
                let to_val = if let Some(t) = to {
                    format!("RAP_SMI_VALUE({})", self.emit_expression(t))
                } else {
                    format!("RAP_SMI_VALUE(RAP_length({c}))", c = coll_temp)
                };
                let slice_temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {} = RAP_create_slice({}, {}, {});",
                    slice_temp, coll_temp, from_val, to_val
                ));
                self.emit_line(&format!(
                    "RAP_slice_assign({}, {});",
                    slice_temp, value_temp
                ));
                self.track_temp(&slice_temp);

                // Decrement replacement if its not a temp (i.e. a local value), since its
                // incremented in lvalue assignment BUT we dont consume the container
                if !self.statement_temps.contains(&value_temp.to_string()) {
                    self.emit_line(&format!("RAP_dec_ref({});", value_temp));
                }
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
                "RAP_Value {}[] = {{{}}};",
                args_array,
                arg_temps.join(", ")
            ));
            self.emit_line(&format!(
                "RAP_call_callable_obj({}, {}, {});",
                callable_temp, args_array, arg_count
            ));
        }
    }

    fn emit_selection(&mut self, sel: &SelectionStatement) {
        match sel {
            SelectionStatement::ValueMatch {
                expression,
                cases,
                else_body,
            } => {
                // Pre-evaluate selector and all case values before the if-else chain,
                // so temps are declared in the enclosing scope (not inside a branch).
                let sel_temp = self.emit_expression(expression);
                let case_value_temps: Vec<Vec<String>> = cases
                    .iter()
                    .map(|case| {
                        case.values
                            .iter()
                            .map(|val_expr| self.emit_expression(val_expr))
                            .collect()
                    })
                    .collect();

                for (i, (case, val_temps)) in cases.iter().zip(case_value_temps.iter()).enumerate()
                {
                    let conditions: Vec<String> = val_temps
                        .iter()
                        .map(|vt| format!("RAP_BOOL_VALUE(RAP_equal({}, {}))", sel_temp, vt))
                        .collect();
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
                // Pre-evaluate all conditions before the if-else chain.
                let cond_temps: Vec<String> = cases
                    .iter()
                    .map(|case| self.emit_expression(&case.condition))
                    .collect();

                for (i, (case, cond_temp)) in cases.iter().zip(cond_temps.iter()).enumerate() {
                    let keyword = if i == 0 { "if" } else { "} else if" };
                    self.emit_line(&format!("{} (RAP_BOOL_VALUE({})) {{", keyword, cond_temp));
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

    fn emit_loop(&mut self, loop_stmt: &LoopStatement) {
        match &loop_stmt.header {
            LoopHeader::For {
                variable,
                from,
                to,
                step,
            } => {
                self.create_scope();

                // Emit from/to/step expressions before the loop
                let from_temp = if let Some(from_expr) = from {
                    self.emit_expression(from_expr)
                } else {
                    let t = self.fresh_temp();
                    self.emit_line(&format!("RAP_Value {} = RAP_create_int_obj(1);", t));
                    t
                };

                let to_temp = to.as_ref().map(|to_expr| self.emit_expression(to_expr));

                let loop_id = self.temp_counter;
                self.temp_counter += 1;
                let iter_var = format!("_iter_{}_{}", Self::transliterate(variable), loop_id);
                let local_var = self.mangle_name(variable);

                let step_val = if let Some(step_expr) = step {
                    let t = self.emit_expression(step_expr);
                    format!("RAP_SMI_VALUE({})", t)
                } else {
                    "1".to_string()
                };

                // Store step in a variable so we can check direction at runtime
                let step_var = format!("_step_{}", loop_id);
                self.emit_line(&format!("int64_t {} = {};", step_var, step_val));

                // Extract upper limit as int64_t, handling both int and float bounds
                let limit_var = format!("_for_limit_{}", loop_id);
                if let Some(ref to_t) = to_temp {
                    self.emit_line(&format!(
                        "int64_t {} = RAP_IS_SMI({}) \
                         ? RAP_SMI_VALUE({}) : (int64_t)RAP_GET_FLOAT_VAL({});",
                        limit_var, to_t, to_t, to_t
                    ));
                }

                // step > 0 → iter <= limit; step < 0 → iter >= limit
                let condition = if to_temp.is_some() {
                    format!(
                        "({s} > 0 ? {i} <= {l} : {i} >= {l})",
                        s = step_var,
                        i = iter_var,
                        l = limit_var
                    )
                } else {
                    "1".to_string()
                };

                self.emit_line(&format!(
                    "for (int64_t {} = RAP_SMI_VALUE({}); {}; {} += {}) {{",
                    iter_var, from_temp, condition, iter_var, step_var
                ));
                self.indent_level += 1;
                self.insert_declared_var(local_var.clone());
                self.emit_line(&format!(
                    "RAP_Value {} = RAP_create_int_obj({});",
                    local_var, iter_var
                ));
                self.emit_blank_line();
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.drop_scope();
                self.indent_level -= 1;
                self.emit_line("}");
            }

            LoopHeader::Repeat(count_expr) => {
                self.create_scope();

                let count_temp = self.emit_expression(count_expr);
                let rep_var = format!("_rep_{}", self.temp_counter);
                self.emit_line(&format!(
                    "for (int64_t {} = 0; {} < RAP_SMI_VALUE({}); {}++) {{",
                    rep_var, rep_var, count_temp, rep_var
                ));
                self.indent_level += 1;
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.drop_scope();
                self.indent_level -= 1;
                self.emit_line("}");
            }

            LoopHeader::Infinite => {
                self.create_scope();
                self.emit_line("while (1) {");
                self.indent_level += 1;
                self.emit_while_condition(&loop_stmt.while_condition);
                self.emit_statement_list(&loop_stmt.body);
                self.emit_post_condition(&loop_stmt.post_condition);
                self.drop_scope();
                self.indent_level -= 1;
                self.emit_line("}");
            }
        }
    }

    /// Emit `пока` (while) pre-condition: `if (!cond) break;` at top of loop body.
    /// Flushes its own temps since they're inside the C loop block.
    fn emit_while_condition(&mut self, while_cond: &Option<Box<Expr>>) {
        if let Some(cond_expr) = while_cond {
            let saved = std::mem::take(&mut self.statement_temps);
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if (!RAP_BOOL_VALUE({})) {{", cond_temp));
            let temps = self.statement_temps.clone();
            for temp in &temps {
                self.emit_line(&format!("  RAP_dec_ref({});", temp));
            }
            self.emit_line("  break;}");
            // self.flush_statement_temps(None);
            self.statement_temps = saved;
        }
    }

    /// Emit `по` (until) post-condition: `if (cond) break;` at bottom of loop body.
    /// Flushes its own temps since they're inside the C loop block.
    fn emit_post_condition(&mut self, post_cond: &Option<Box<Expr>>) {
        if let Some(cond_expr) = post_cond {
            let saved = std::mem::take(&mut self.statement_temps);
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if (RAP_BOOL_VALUE({})) break;", cond_temp));
            self.flush_statement_temps(None);
            self.statement_temps = saved;
        }
    }

    /// Emit code for an expression. Returns the temp variable name holding the result.
    fn emit_expression(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(lit) => self.emit_literal(lit),

            Expr::Name(name) => {
                if let Some(info) = self.known_callables.get(name.as_str()) {
                    // TODO: we can detect name collisions here
                    // Known function/procedure - callable objects created, they are treated as objects too
                    let c_func_name = info.c_func_name.clone();
                    let params = info.params.clone();
                    let is_function = info.is_function;
                    self.emit_inline_callable(&c_func_name, &params, is_function)
                } else if self.is_var_in_scope(&self.mangle_name(name)) {
                    // Parameter is local
                    self.mangle_name(name)
                } else {
                    // Variable lookup
                    let temp = self.fresh_temp();
                    if self.foreign_vars.contains(name.as_str()) {
                        self.emit_line(&format!(
                            "RAP_Value {} = RAP_frame_get_foreign({}, \"{}\");",
                            temp, self.current_frame, name
                        ));
                    } else {
                        self.emit_line(&format!(
                            "RAP_Value {} = RAP_frame_get({}, \"{}\");",
                            temp, self.current_frame, name
                        ));
                    }
                    self.track_temp(&temp);
                    temp
                }
            }

            Expr::BinaryOp {
                operator,
                left,
                right,
            } => {
                let left_temp = self.emit_expression(left);
                let right_temp = self.emit_expression(right);

                // self.track_temp(&left_temp);
                // self.track_temp(&right_temp);

                // Both temps must be cleared, they can be heap allocated temporaries and
                // we should decrement their refcount at the scope end
                // TODO: this is ugly, need to rethink how to handle frame-based locals
                // if !left_temp.contains("_local") {
                //     self.insert_declared_var(left_temp.clone());
                // }
                // if !right_temp.contains("_local_") {
                //     self.insert_declared_var(right_temp.clone());
                // }

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
                    "RAP_Value {} = RAP_get_tuple_item({}, (uint32_t)RAP_SMI_VALUE({}));",
                    result, coll_temp, idx_temp
                ));
                self.track_temp(&result);
                result
            }

            Expr::Slice {
                collection,
                from,
                to,
            } => {
                let coll_temp = self.emit_expression(collection);
                let from_val = if let Some(f) = from {
                    format!("RAP_SMI_VALUE({})", self.emit_expression(f))
                } else {
                    "0".to_string()
                };
                let to_val = if let Some(t) = to {
                    format!("RAP_SMI_VALUE({})", self.emit_expression(t))
                } else {
                    format!("RAP_SMI_VALUE(RAP_length({c}))", c = coll_temp)
                };
                let result = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {} = RAP_create_slice({}, {}, {});",
                    result, coll_temp, from_val, to_val
                ));
                self.track_temp(&result);
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
        self.emit_line(&format!("RAP_Value {} = {};", temp, rhs));
        self.track_temp(&temp);
        temp
    }

    fn emit_binary_op(&mut self, op: &BinaryOperator, left: &str, right: &str) -> String {
        let temp = self.fresh_temp();
        // self.track_temp(&left);
        // self.track_temp(&right);

        let rhs = match op {
            BinaryOperator::Add => format!("RAP_add({}, {})", left, right),
            BinaryOperator::Subtract => format!("RAP_subtract({}, {})", left, right),
            BinaryOperator::Multiply => format!("RAP_multiply({}, {})", left, right),
            BinaryOperator::Divide => format!("RAP_divide({}, {})", left, right),
            BinaryOperator::IntegerDivide => format!("RAP_integer_divide({}, {})", left, right),
            BinaryOperator::Remainder => {
                format!("RAP_integer_modulo({}, {})", left, right)
            }
            BinaryOperator::Power => format!("RAP_power({}, {})", left, right),
            BinaryOperator::Less => {
                format!("RAP_less_than({}, {})", left, right)
            }
            BinaryOperator::Greater => {
                format!("RAP_greater_than({}, {})", left, right)
            }
            BinaryOperator::LessOrEqual => format!("RAP_less_or_equal({}, {})", left, right),
            BinaryOperator::GreaterOrEqual => {
                format!("RAP_greater_or_equal({}, {})", left, right)
            }
            BinaryOperator::Equal => {
                format!("RAP_equal({}, {})", left, right)
            }
            BinaryOperator::NotEqual => format!("RAP_not_equal({}, {})", left, right),
            BinaryOperator::And => format!(
                "RAP_create_logical_obj(RAP_BOOL_VALUE({}) && RAP_BOOL_VALUE({}))",
                left, right
            ),
            BinaryOperator::Or => format!(
                "RAP_create_logical_obj(RAP_BOOL_VALUE({}) || RAP_BOOL_VALUE({}))",
                left, right
            ),
        };
        self.emit_line(&format!("RAP_Value {} = {};", temp, rhs));
        self.track_temp(&temp);
        temp
    }

    fn emit_unary_op(&mut self, op: &UnaryOperator, operand: &str) -> String {
        let temp = self.fresh_temp();
        let rhs = match op {
            UnaryOperator::Negate => format!("RAP_negate({})", operand),
            UnaryOperator::Plus => {
                // No-op: just alias the operand
                return operand.to_string();
            }
            UnaryOperator::Not => format!("RAP_create_logical_obj(!RAP_BOOL_VALUE({}))", operand),
            UnaryOperator::Length => format!("RAP_length({})", operand),
        };
        self.emit_line(&format!("RAP_Value {} = {};", temp, rhs));
        self.track_temp(&temp);
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
                "RAP_Value {} = RAP_call_callable_obj({}, NULL, 0);",
                result, func_temp
            ));
        } else if arg_count == 1 {
            self.emit_line(&format!(
                "RAP_Value {} = RAP_call_callable_obj({}, &{}, 1);",
                result, func_temp, arg_temps[0]
            ));
        } else {
            let args_array = format!("_call_args_{}", self.temp_counter);
            self.emit_line(&format!(
                "RAP_Value {}[] = {{{}}};",
                args_array,
                arg_temps.join(", ")
            ));
            self.emit_line(&format!(
                "RAP_Value {} = RAP_call_callable_obj({}, {}, {});",
                result, func_temp, args_array, arg_count
            ));
        }
        self.track_temp(&result);
        result
    }

    /// Try to emit a built-in function call. Returns Some(temp_name) if handled.
    fn try_emit_builtin(&mut self, name: &str, arguments: &[Box<Expr>]) -> Option<String> {
        let result = match name {
            "корень" | "sqrt" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_float_obj(\
                     sqrt(RAP_IS_SMI({a}) \
                     ? (double)RAP_SMI_VALUE({a}) : RAP_GET_FLOAT_VAL({a})));",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "абс" | "abs" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_IS_SMI({a}) \
                     ? RAP_create_int_obj(llabs(RAP_SMI_VALUE({a}))) \
                     : RAP_create_float_obj(fabs(RAP_GET_FLOAT_VAL({a})));",
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
                    "RAP_Value {} = RAP_create_int_obj(\
                     (int64_t)RAP_GET_FLOAT_VAL({}));",
                    temp, arg
                ));
                Some(temp)
            }
            "длин" => {
                // Length of text or tuple (same as # operator)
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_length({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "sign" | "знак" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_sign({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "целч" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_floor({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "окрч" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_round({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "дсч" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_random({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "цсч" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_random_int({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "индекс" => {
                let needle = self.emit_expression(&arguments[0]);
                let haystack = self.emit_expression(&arguments[1]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_index_of({n}, {h});",
                    t = temp,
                    n = needle,
                    h = haystack
                ));
                Some(temp)
            }
            "тип_пуст" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_NULL);",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_лог" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_BOOL({a}));",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            "тип_цел" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_SMI({a}) || (RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_INT));",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_вещ" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_FLOAT({a}) || (RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_FLOAT));",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_текст" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_TEXT);",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_корт" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_TUPLE);",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_проц" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_CALLABLE && !RAP_PTR_VALUE({a})->callable_val->is_function);",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            "тип_функ" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_create_logical_obj(RAP_IS_PTR({a}) && RAP_PTR_VALUE({a})->tag == RAP_OBJECT_TAG_CALLABLE && RAP_PTR_VALUE({a})->callable_val->is_function);",
                    t = temp, a = arg
                ));
                Some(temp)
            }
            // Gets the current refcount of an object
            "кол_ссылок" => {
                let arg = self.emit_expression(&arguments[0]);
                let temp = self.fresh_temp();
                self.emit_line(&format!(
                    "RAP_Value {t} = RAP_get_objects_refcount({a});",
                    t = temp,
                    a = arg
                ));
                Some(temp)
            }
            _ => None,
        };
        if let Some(ref t) = result {
            self.track_temp(t);
        }
        result
    }

    fn emit_tuple_construct(&mut self, items: &[Box<Expr>]) -> String {
        let item_temps: Vec<String> = items
            .iter()
            .map(|item| self.emit_expression(item))
            .collect();

        let count = item_temps.len();
        let items_array = format!("_tuple_items_{}", self.temp_counter);
        self.emit_line(&format!(
            "RAP_Value {}[] = {{{}}};",
            items_array,
            item_temps.join(", ")
        ));
        let result = self.fresh_temp();
        self.emit_line(&format!(
            "RAP_Value {} = RAP_create_tuple_obj({}, {});",
            result, count, items_array
        ));
        self.track_temp(&result);
        result
    }
}
