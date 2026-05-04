//! AST to C code generator.
//!
//! Walks the AST and emits C code that uses the runtime library (runtime.h).

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
};

use log::debug;

use crate::{
    ast::*,
    codegen::{CodegenTarget, CodegenWarning, ModuleMap, RunError, find_runtime_dir},
    module::Module,
    pretty::pretty_parse_warning,
};

/// A stack of declared variables in scope
type ScopeVariablesStack = Vec<HashSet<String>>;

/// Holds codegen state while walking the AST.
pub struct CGen {
    /// Path to the source file of the module currently being emitted
    current_module_path: String,
    /// User declared types collected from all modules at the start of generate
    /// key: module base name, value: list of type definitions
    declared_types: HashMap<String, Vec<TypeDefinition>>,
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
    /// Variables in current function/procedure that need to be saved in the frame
    /// so other functions can access them via foreign vars
    variables_need_saving: Vec<String>,
    /// Temps created during expression evaluation that own a new value and need dec_ref at statement end.
    statement_temps: Vec<String>,
    /// module_base_name -> (rapira_name -> callable info)
    known_callables: HashMap<String, HashMap<String, CallableInfo>>,
    /// Variables declared as чужие in current function scope (original Rapira names).
    /// Empty at top level and inside functions without чужие.
    foreign_vars: HashSet<String>,
    /// True when emitting inside a function/procedure body (use frame-based access for non-param locals)
    inside_function: bool,
    /// Emit RAP_check_leaks() call and #define RAP_TEST_LEAKS
    check_leaks: bool,
    /// Path of the entry module (the one defining вход()), used by compile() for binary naming
    entry_module_path: String,
    /// Imported functions
    imported_definitions: HashSet<String>,
}

impl CGen {
    pub fn new() -> Self {
        Self {
            current_module_path: String::new(),
            declared_types: HashMap::new(),
            output: String::new(),
            forward_decls: String::new(),
            indent_level: 1, // inside main()
            temp_counter: 0,
            string_counter: 0,
            param_counter: 0,
            current_frame: "&_main_frame".to_string(),
            declared_vars: Vec::new(),
            variables_need_saving: Vec::new(),
            statement_temps: Vec::new(),
            known_callables: HashMap::new(),
            foreign_vars: HashSet::new(),
            inside_function: false,
            check_leaks: false,
            entry_module_path: String::new(),
            imported_definitions: HashSet::new(),
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

    fn is_known_type(&self, var: &str) -> bool {
        self.find_type_def(var).is_some()
    }

    fn find_type_def(&self, variant_name: &str) -> Option<&TypeDefinition> {
        let current = self.current_module_base_name();
        if let Some(typedef) = self
            .declared_types
            .get(&current)
            .and_then(|m| m.iter().find(|t| t.variants.contains_key(variant_name)))
        {
            return Some(typedef);
        }
        None
    }

    fn add_type_def(&mut self, name: String, variants: HashMap<String, Vec<String>>) {
        let current = self.current_module_base_name();
        self.declared_types
            .entry(current)
            .or_default()
            .push(TypeDefinition { name, variants });
    }

    fn current_module_base_name(&self) -> String {
        let path = std::path::Path::new(&self.current_module_path);
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        module_base_name(filename)
    }

    fn find_callable(&self, name: &str) -> Option<&CallableInfo> {
        let current = self.current_module_base_name();
        if let Some(info) = self.known_callables.get(&current).and_then(|m| m.get(name)) {
            return Some(info);
        }
        self.known_callables.values().find_map(|m| m.get(name))
    }

    fn insert_callable(&mut self, name: String, info: CallableInfo) {
        let module = self.current_module_base_name();
        self.known_callables
            .entry(module)
            .or_default()
            .insert(name, info);
    }

    fn reset_for_module(&mut self) {
        self.output = String::new();
        self.forward_decls = String::new();
        self.indent_level = 1;
        self.temp_counter = 0;
        self.string_counter = 0;
        self.param_counter = 0;
        self.current_frame = "&_main_frame".to_string();
        self.declared_vars = Vec::new();
        self.variables_need_saving = Vec::new();
        self.statement_temps = Vec::new();
        self.foreign_vars = HashSet::new();
        self.inside_function = false;
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
    fn emit_epilogue(&mut self, exclude: Option<&str>) {
        let all_vars: Vec<String> = self
            .declared_vars
            .iter()
            .flat_map(|scope| scope.iter().cloned()) // TODO: cloned
            .filter(|s| s != exclude.unwrap_or_default())
            .collect();
        for var in all_vars {
            self.emit_line(&format!("RAP_dec_ref({});", var));
        }
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
                'А' => "A",
                'а' => "a",
                'Б' => "B",
                'б' => "b",
                'В' => "V",
                'в' => "v",
                'Г' => "G",
                'г' => "g",
                'Д' => "D",
                'д' => "d",
                'Е' => "E",
                'е' => "e",
                'Ё' => "YO",
                'ё' => "yo",
                'Ж' => "ZH",
                'ж' => "zh",
                'З' => "Z",
                'з' => "z",
                'И' => "I",
                'и' => "i",
                'Й' => "J",
                'й' => "j",
                'К' => "K",
                'к' => "k",
                'Л' => "L",
                'л' => "l",
                'М' => "M",
                'м' => "m",
                'Н' => "N",
                'н' => "n",
                'О' => "O",
                'о' => "o",
                'П' => "P",
                'п' => "p",
                'Р' => "R",
                'р' => "r",
                'С' => "S",
                'с' => "s",
                'Т' => "T",
                'т' => "t",
                'У' => "U",
                'у' => "u",
                'Ф' => "F",
                'ф' => "f",
                'Х' => "KH",
                'х' => "kh",
                'Ц' => "TS",
                'ц' => "ts",
                'Ч' => "CH",
                'ч' => "ch",
                'Ш' => "SH",
                'ш' => "sh",
                'Щ' => "SHCH",
                'щ' => "shch",
                'Ъ' => "",
                'ъ' => "",
                'Ы' => "Y",
                'ы' => "y",
                'Ь' => "",
                'ь' => "",
                'Э' => "E",
                'э' => "e",
                'Ю' => "YU",
                'ю' => "yu",
                'Я' => "YA",
                'я' => "ya",
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
        format!(
            "RAP_FUNC_{}_{}",
            self.current_module_base_name(),
            Self::transliterate(rapira_name)
        )
    }

    fn mangle_type_name(&self, rapira_name: &str) -> String {
        format!(
            "RAP_TYPE_{}_{}",
            self.current_module_base_name(),
            Self::transliterate(rapira_name)
        )
    }

    fn mangle_type_variant_name(&self, type_name: &str, variant_name: &str) -> String {
        format!(
            "RAP_TYPE_CTR_{}_{}_{}",
            self.current_module_base_name(),
            Self::transliterate(type_name),
            Self::transliterate(variant_name)
        )
    }

    fn emit_statement_list(&mut self, stmts: &[Spannable<Statement>]) {
        for stmt in stmts {
            self.emit_statement(stmt);
        }
    }

    fn emit_type_def(&mut self, type_def_span: &Spannable<TypeDefinition>) {
        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            type_def_span.position_start
        ));
        self.emit_line(&format!(
            "RAP_current_pos_end={};",
            type_def_span.position_end
        ));

        let saved_output = std::mem::take(&mut self.output);
        let saved_indent = self.indent_level;

        self.indent_level = 0;

        // Define a struct for each variant
        let type_def = &type_def_span.node;
        for (variant_name, fields) in &type_def.variants {
            self.emit_line(&format!("typedef struct {{"));
            for field in fields {
                self.emit_line(&format!("    RAP_Value {};", Self::transliterate(field)));
            }
            self.emit_line(&format!(
                "}} {};\n",
                self.mangle_type_variant_name(&type_def.name, variant_name)
            ));
        }

        // Define field names for each variant
        for (variant_name, fields) in &type_def.variants {
            self.emit_line(&format!(
                "const char* {}_field_names[] = {{",
                Self::transliterate(variant_name)
            ));

            for field in fields {
                self.emit_line(&format!("  \"{}\",", field));
            }
            self.emit_line("};");
        }

        // Now create the custom tagged type
        self.emit_line(&format!("\ntypedef struct {{"));
        self.emit_line(&format!("  uint16_t tag;"));
        self.emit_line(&format!("  union {{"));
        for (variant_name, _) in &type_def.variants {
            self.emit_line(&format!(
                "    {} {};",
                self.mangle_type_variant_name(&type_def.name, variant_name),
                Self::transliterate(variant_name)
            ));
        }
        self.emit_line(&format!("  }};"));
        self.emit_line(&format!(
            "}} __attribute__((packed)) {};\n",
            self.mangle_type_name(&type_def.name)
        ));

        self.forward_decls.push_str(&self.output);

        self.output = saved_output;
        self.indent_level = saved_indent;

        self.add_type_def(type_def.name.clone(), type_def.variants.clone());
    }

    fn emit_function_def(&mut self, func_def_span: &Spannable<FunctionDefinition>) {
        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            func_def_span.position_start
        ));
        self.emit_line(&format!(
            "RAP_current_pos_end={};",
            func_def_span.position_end
        ));

        let func_def = &func_def_span.node;
        let name = func_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);
        self.insert_callable(
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
        self.variables_need_saving = func_def.variables_need_saving.clone().into_iter().collect();

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

        // Clear saved variables
        self.variables_need_saving.clear();
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

    fn emit_procedure_def(&mut self, proc_def_span: &Spannable<ProcedureDefinition>) {
        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            proc_def_span.position_start
        ));
        self.emit_line(&format!(
            "RAP_current_pos_end={};",
            proc_def_span.position_end
        ));

        let proc_def = &proc_def_span.node;
        let name = proc_def.name.as_deref().unwrap_or("_anon");
        let c_func_name = self.mangle_func_name(name);
        self.insert_callable(
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

        self.variables_need_saving = proc_def.variables_need_saving.clone().into_iter().collect(); // TODO: no clone

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

        // Clear saved variables
        self.variables_need_saving.clear();
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
        self.track_temp(&temp);
        temp
    }

    fn emit_statement(&mut self, stmt_span: &Spannable<Statement>) {
        let stmt = &stmt_span.node;

        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            stmt_span.position_start
        ));
        self.emit_line(&format!("RAP_current_pos_end={};", stmt_span.position_end));

        // Each statement gets its own temp tracking context.
        // Inner statements (in if/loop bodies) save and restore, so they don't
        // interfere with the outer statement's temps.
        let saved_temps = std::mem::take(&mut self.statement_temps);

        match stmt {
            Statement::Empty => {}

            Statement::Assignment { target, value } => {
                let value_temp = self.emit_expression(&value);

                // Decrement refcount of old value in target (if reassigning a known variable)
                if let Spannable {
                    node: LValue::Name(name),
                    ..
                } = &target
                {
                    let mangled = self.mangle_name(&name);
                    if self.is_var_in_scope(&mangled) {
                        self.emit_line(&format!("RAP_dec_ref({});", mangled));
                    }
                }

                // Increment refcount when assigning from another variable (shared reference)
                if let Spannable {
                    node: Expr::Name(_),
                    ..
                } = value.as_ref()
                {
                    // Do not increment refcount if value is taken from frame OR if its a temp,
                    // since frame_get increments count on itself and temps usually created with refcount=1
                    // TODO: refactor, this is bad
                    if self.output.lines().last().is_some()
                        && !self.output.lines().last().unwrap().contains("frame_get")
                        && !value_temp.contains("_t")
                    {
                        self.emit_line(&format!("RAP_inc_ref({});", value_temp));
                    }
                }

                self.emit_lvalue_assignment(&target, &value_temp);

                // For name assignments, ownership transfers to the C local — don't dec_ref the value.
                // For subscript/slice, the runtime inc_refs internally, so all temps can be freed.
                let exclude = match target {
                    Spannable {
                        node: LValue::Name(_),
                        ..
                    } => Some(value_temp.as_str()),
                    _ => None,
                };
                self.flush_statement_temps(exclude);
            }

            Statement::ProcedureCall {
                procedure: proc,
                arguments,
            } => {
                let procedure = &proc.node;

                // Collect in-out param info: (proc_param_name, caller_var_name)
                // TODO: refactor, wtf is this
                let inout_pairs: Vec<(String, String)> = if let Expr::Name(proc_name) = procedure {
                    if let Some(info) = self.find_callable(proc_name.as_str()) {
                        info.params
                            .iter()
                            .zip(arguments.iter())
                            .filter_map(|((param_name, mode), arg)| {
                                if mode == "RAP_PARAMETER_MODE_OUT" {
                                    if let CallArgument::InOut(Spannable {
                                        node: LValue::Name(caller_name),
                                        ..
                                    }) = arg
                                    {
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

                    if self.variables_need_saving.contains(&caller_name) {
                        // Get variable from current scope frame, if it is saved
                        self.emit_line(&format!(
                            "RAP_Value {} = RAP_frame_get({}, \"{}\");",
                            val_temp, self.current_frame, caller_name
                        ));

                        self.emit_line(&format!(
                            "RAP_frame_set({}, \"{}\", {});",
                            self.current_frame, param_name, val_temp
                        ));

                        // Also incref inout pararms
                        // frame_get already increfs (?)
                        // self.emit_line(&format!("RAP_inc_ref({});", val_temp));
                    } else {
                        // Otherwise just use mangled name, since it is just a local
                        self.emit_line(&format!(
                            "RAP_frame_set({}, \"{}\", {});",
                            self.current_frame,
                            param_name,
                            self.mangle_name(&caller_name)
                        ));

                        // Also incref inout pararms
                        self.emit_line(&format!(
                            "RAP_inc_ref({});",
                            self.mangle_name(&caller_name)
                        ));
                    }
                }

                let proc_temp = self.emit_expression(&proc);
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
                let cond_temp = self.emit_expression(&condition);
                self.emit_line(&format!("if (RAP_BOOL_VALUE({})) {{", cond_temp));

                self.create_scope();

                self.indent_level += 1;
                self.emit_statement_list(&then_body);
                self.drop_scope();
                self.indent_level -= 1;
                if let Some(else_stmts) = else_body {
                    self.emit_line("} else {");
                    self.create_scope();
                    self.indent_level += 1;
                    self.emit_statement_list(&else_stmts);
                    self.drop_scope();
                    self.indent_level -= 1;
                }
                self.emit_line("}");
                self.flush_statement_temps(None);
            }

            Statement::Selection(sel) => {
                self.emit_selection(&sel);
                self.flush_statement_temps(None);
            }

            Statement::Loop(loop_stmt) => {
                self.emit_loop(&loop_stmt);
                // self.flush_statement_temps(None);
                let temps = self.statement_temps.clone();
                for temp in &temps {
                    self.emit_line(&format!("RAP_dec_ref({});", temp));
                }
            }

            Statement::Output { no_newline, values } => {
                for value_expr in values {
                    let val_temp = self.emit_expression(value_expr.as_ref());
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
                    self.emit_lvalue_assignment(&var, &temp);
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
                self.emit_epilogue(None);
                self.emit_line("return RAP_create_null_obj();");
            }

            Statement::ReturnFromFunction(expr) => {
                let result_temp = self.emit_expression(&expr);
                // Remember to protect return value
                let mut temps = saved_temps.clone();
                temps.append(&mut self.statement_temps.clone());
                for temp in &temps {
                    if *temp != result_temp {
                        self.emit_line(&format!("RAP_dec_ref({});", temp));
                    }
                }
                self.emit_epilogue(Some(&result_temp));
                self.emit_line(&format!("return {};", result_temp));
            }

            Statement::Import { name, definitions } => {
                // TODO: save definitions and check
                for def in definitions {
                    self.imported_definitions.insert(def.clone());
                }

                self.emit_line(&format!("RAP_{}_MOD_ENTRY();", name));
            }
        }

        self.statement_temps = saved_temps;
    }

    fn emit_lvalue_assignment(&mut self, target_node: &Spannable<LValue>, value_temp: &str) {
        let target = &target_node.node;

        match target {
            LValue::Name(name) => {
                let mangled = self.mangle_name(name);
                if self.is_var_in_scope(&mangled) {
                    // Parameter updates BOTH C local and frame
                    // self.emit_line(&format!("RAP_inc_ref({});", value_temp)); // handled in caller
                    self.emit_line(&format!("{} = {};", mangled, value_temp));
                    // Variable needs saving in frame
                    if self.variables_need_saving.contains(name) {
                        self.emit_line(&format!(
                            "RAP_frame_set({}, \"{}\", {});",
                            self.current_frame, name, mangled
                        ));
                    }
                } else if self.foreign_vars.contains(name.as_str()) {
                    // чужие - here we walk up the frame chain and try to find the variable
                    self.emit_line(&format!(
                        "RAP_frame_set_foreign({}, \"{}\", {});",
                        self.current_frame, name, value_temp
                    ));
                } else {
                    // Locals saved to frame, to allow access from nested scopes
                    // unless value is never requested as `чужие` in other frames
                    if self.variables_need_saving.contains(name) {
                        self.emit_line(&format!(
                            "RAP_frame_set({}, \"{}\", {});",
                            self.current_frame, name, value_temp
                        ));
                    } else {
                        if self.is_var_in_scope(&mangled) {
                            self.emit_line(&format!("{} = {};", mangled, value_temp));
                        } else {
                            self.emit_line(&format!("RAP_Value {} = {};", mangled, value_temp));
                            self.insert_declared_var(mangled);
                        }
                    }
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
            LValue::Field { left, field } => {
                // Just an identifier
                let left_temp = self.emit_expression(&left);

                self.emit_line(&format!(
                    "RAP_set_variant_field({}, \"{}\", {});",
                    left_temp, field, value_temp
                ));
                self.track_temp(&value_temp);
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

    /// Classify a `при`-case pattern as either a plain value expression or
    /// a constructor pattern `Ctr(x, y, ...)` that binds fields by position.
    /// A constructor pattern is only recognised when `Ctr` is a known type
    /// variant AND every argument is a bare name (no nested expressions).
    fn classify_case_pattern<'a>(
        &self,
        pattern: &'a Spannable<Expr>,
    ) -> Option<(String, Vec<String>)> {
        if let Expr::FunctionCall {
            function,
            arguments,
        } = &pattern.node
        {
            if let Expr::Name(ctr_name) = &function.node {
                if self.is_known_type(ctr_name) {
                    let bindings: Option<Vec<String>> = arguments
                        .iter()
                        .map(|arg| match &arg.node {
                            Expr::Name(n) => Some(n.clone()),
                            _ => None,
                        })
                        .collect();
                    if let Some(names) = bindings {
                        return Some((ctr_name.clone(), names));
                    }
                }
            }
        }
        None
    }

    fn emit_selection(&mut self, sel: &SelectionStatement) {
        match sel {
            SelectionStatement::ValueMatch {
                expression,
                cases,
                else_body: _,
            } => {
                let sel_temp = self.emit_expression(expression);

                // A single-value case whose value is `Ctr(x, y, ...)` with a
                // known type and bare-name args is a binding pattern. Anything
                // else is a plain value-tag match.
                let binding_patterns: Vec<Option<(String, Vec<String>)>> = cases
                    .iter()
                    .map(|case| {
                        if case.node.values.len() == 1 {
                            self.classify_case_pattern(&case.node.values[0])
                        } else {
                            None
                        }
                    })
                    .collect();

                // Pre-evaluate value-pattern temps (outside the if-else chain)
                // so their declarations don't leak into a branch body. Binding
                // patterns produce no runtime temp — their condition is a
                // compile-time tag constant.
                let case_value_temps: Vec<Vec<String>> = cases
                    .iter()
                    .zip(binding_patterns.iter())
                    .map(|(case, bp)| {
                        if bp.is_some() {
                            Vec::new()
                        } else {
                            case.node
                                .values
                                .iter()
                                .map(|val_expr| self.emit_expression(val_expr))
                                .collect()
                        }
                    })
                    .collect();

                for (i, (case, bp)) in cases.iter().zip(binding_patterns.iter()).enumerate() {
                    let keyword = if i == 0 { "if" } else { "} else if" };

                    let conditions: Vec<String> = if let Some((ctr_name, _)) = bp {
                        let type_def = self.find_type_def(ctr_name).unwrap();
                        let tag = type_def
                            .variants
                            .iter()
                            .position(|(k, _)| k == ctr_name)
                            .unwrap();
                        vec![format!("RAP_get_variant_tag({}) == {}", sel_temp, tag)]
                    } else {
                        case_value_temps[i]
                            .iter()
                            .map(|vt| {
                                format!(
                                    "RAP_get_variant_tag({}) == RAP_get_variant_tag({})",
                                    sel_temp, vt
                                )
                            })
                            .collect()
                    };

                    self.emit_line(&format!("{} ({}) {{", keyword, conditions.join(" || ")));
                    self.indent_level += 1;

                    if let Some((ctr_name, binding_names)) = bp {
                        self.create_scope();
                        self.emit_constructor_bindings(&sel_temp, ctr_name, binding_names);
                        self.emit_statement_list(&case.node.body);
                        self.drop_scope();
                    } else {
                        self.emit_statement_list(&case.node.body);
                    }

                    self.indent_level -= 1;
                }
                self.emit_line("}");
            }
        }
    }

    /// Inside a matched `при Ctr(a, b, ...)` branch, extract the variant's
    /// payload and bind each field to a fresh local. Bindings take a reference
    /// (inc_ref) and are registered with the current scope so drop_scope
    /// releases them on exit.
    fn emit_constructor_bindings(
        &mut self,
        selector_temp: &str,
        ctr_name: &str,
        binding_names: &[String],
    ) {
        let type_def = self.find_type_def(ctr_name).unwrap().clone();
        let mangled_type_name = self.mangle_type_name(&type_def.name);
        let variant_tag = Self::transliterate(ctr_name);
        let fields = type_def.variants.get(ctr_name).unwrap().clone();

        let payload_ptr = format!("_payload_{}", self.temp_counter);
        self.temp_counter += 1;
        self.emit_line(&format!(
            "{ty} *{ptr} = ({ty}*)RAP_GET_VARIANT_VAL({sel})->payload;",
            ty = mangled_type_name,
            ptr = payload_ptr,
            sel = selector_temp
        ));

        for (binding, field) in binding_names.iter().zip(fields.iter()) {
            let mangled = self.mangle_name(binding);
            // Shadow any outer binding: always emit a fresh local scoped to
            // the case body, not a reassignment.
            self.emit_line(&format!(
                "RAP_Value {m} = {ptr}->{v}.{f};",
                m = mangled,
                ptr = payload_ptr,
                v = variant_tag,
                f = Self::transliterate(field)
            ));
            self.emit_line(&format!("RAP_inc_ref({});", mangled));
            self.insert_declared_var(mangled);
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
    fn emit_while_condition(&mut self, while_cond: &Option<Box<Spannable<Expr>>>) {
        if let Some(cond_expr) = while_cond {
            self.emit_line(&format!(
                "RAP_current_pos_start={};",
                cond_expr.position_start
            ));
            self.emit_line(&format!("RAP_current_pos_end={};", cond_expr.position_end));

            let saved = std::mem::take(&mut self.statement_temps);
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if (!RAP_BOOL_VALUE({})) {{", cond_temp));
            let temps = self.statement_temps.clone();
            // Decref temps in condition body
            for temp in &temps {
                self.emit_line(&format!("  RAP_dec_ref({});", temp));
            }
            self.emit_line("  break;}");
            // We should also decref if we havent step into branch
            for temp in &temps {
                self.emit_line(&format!("RAP_dec_ref({});", temp));
            }
            // self.flush_statement_temps(None);
            self.statement_temps = saved;
        }
    }

    /// Emit `по` (until) post-condition: `if (cond) break;` at bottom of loop body.
    /// Flushes its own temps since they're inside the C loop block.
    fn emit_post_condition(&mut self, post_cond: &Option<Box<Spannable<Expr>>>) {
        if let Some(cond_expr) = post_cond {
            self.emit_line(&format!(
                "RAP_current_pos_start={};",
                cond_expr.position_start
            ));
            self.emit_line(&format!("RAP_current_pos_end={};", cond_expr.position_end));

            let saved = std::mem::take(&mut self.statement_temps);
            let cond_temp = self.emit_expression(cond_expr);
            self.emit_line(&format!("if (RAP_BOOL_VALUE({})) break;", cond_temp));
            self.flush_statement_temps(None);
            self.statement_temps = saved;
        }
    }

    fn emit_type_constructor_call(
        &mut self,
        name: &str,
        temp: &str,
        arguments: &[Box<Spannable<Expr>>],
    ) {
        let type_def = self.find_type_def(name).unwrap().clone();
        let mangled_type_name = self.mangle_type_name(&type_def.name);
        let ctr_tag = type_def
            .variants
            .iter()
            .position(|(k, _)| k == name)
            .unwrap();
        let ctor_fields = type_def.variants.get(name).unwrap();

        // Evaluate argument expressions BEFORE emitting the struct literal —
        // emit_expression emits its own statements, which would otherwise leak
        // into the middle of the initializer and produce invalid C.
        let arg_temps: Vec<String> = arguments
            .iter()
            .map(|arg| self.emit_expression(arg))
            .collect();

        // The payload owns each field, so take a reference. The caller
        // decrefs the arg temps when the statement ends
        for arg_temp in &arg_temps {
            self.emit_line(&format!("RAP_inc_ref({});", arg_temp));
        }

        let payload_name = format!("{}_{}_payload", temp, Self::transliterate(name));

        self.emit_line(&format!("{} {} = {{", mangled_type_name, payload_name));
        self.emit_line(&format!("  .tag = {},", ctr_tag));
        self.emit_line(&format!("  .{} = {{", Self::transliterate(name)));

        for (i, field) in ctor_fields.iter().enumerate() {
            self.emit_line(&format!(
                "    .{} = {},",
                Self::transliterate(field),
                arg_temps[i]
            ));
        }

        self.emit_line("  },");
        self.emit_line("};");

        self.emit_line(&format!(
            "RAP_Value {} = RAP_create_custom_typed_obj(\"{}\", {}_field_names, {}, &{});",
            temp,
            mangled_type_name,
            Self::transliterate(name),
            ctor_fields.len(),
            payload_name
        ));
    }

    /// Emit code for an expression. Returns the temp variable name holding the result.
    fn emit_expression(&mut self, expr_node: &Spannable<Expr>) -> String {
        let expr = &expr_node.node;

        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            expr_node.position_start
        ));
        self.emit_line(&format!("RAP_current_pos_end={};", expr_node.position_end));

        match expr {
            Expr::Literal(lit) => self.emit_literal(lit),

            Expr::Name(name) => {
                if let Some(info) = self.find_callable(name.as_str()) {
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
                    } else if self.is_known_type(name) {
                        self.emit_type_constructor_call(name, &temp, &[]);
                    } else {
                        // Variable needs saving in frame
                        if self.variables_need_saving.contains(name) {
                            self.emit_line(&format!(
                                "RAP_Value {} = RAP_frame_get({}, \"{}\");",
                                temp, self.current_frame, name
                            ));
                        } else {
                            // Issue warning
                            println!(
                                "{}",
                                pretty_parse_warning(
                                    &std::fs::read_to_string(&self.current_module_path).unwrap(),
                                    &self.current_module_path,
                                    CodegenWarning::UndeclaredVariable(
                                        expr_node.position_start,
                                        name.clone(),
                                        expr_node.position_end
                                    )
                                )
                            );

                            // Undeclared variables are NULL by default
                            let mangled = self.mangle_name(name);
                            if !self.is_var_in_scope(&mangled) {
                                self.emit_line(&format!(
                                    "RAP_Value {} = RAP_create_null_obj();",
                                    mangled
                                ));
                            }

                            return mangled;
                        }
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
                let right_temp = match (operator, &right.node) {
                    // Special case for field access where `right` is
                    // just a string rather than an expression
                    (BinaryOperator::Dot, Expr::Name(name)) => name.clone(),
                    _ => self.emit_expression(right),
                };

                self.emit_binary_op(operator, &left_temp, &right_temp)
            }

            Expr::UnaryOp { operator, operand } => {
                let operand_temp = self.emit_expression(operand);
                self.emit_unary_op(operator, &operand_temp)
            }

            Expr::FunctionCall {
                function,
                arguments,
            } => {
                // Rules:
                // - Function calls start with a lower letter
                // - Type constructors start with a capital letter
                if let Expr::Name(name) = &function.node {
                    if self.is_known_type(name) {
                        let temp = self.fresh_temp();
                        self.emit_type_constructor_call(name, &temp, arguments);
                        temp
                    } else {
                        self.emit_function_call(function, arguments)
                    }
                } else {
                    self.emit_function_call(function, arguments)
                }
            }

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
            BinaryOperator::Dot => format!("RAP_get_variant_field({}, \"{}\")", left, right),
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

    fn emit_function_call(
        &mut self,
        function_node: &Spannable<Expr>,
        arguments: &[Box<Spannable<Expr>>],
    ) -> String {
        let function = &function_node.node;

        self.emit_line(&format!(
            "RAP_current_pos_start={};",
            function_node.position_start
        ));
        self.emit_line(&format!(
            "RAP_current_pos_end={};",
            arguments
                .last()
                .map(|i| i.position_end)
                .unwrap_or(function_node.position_start)
        ));

        // Check for built-in functions by name
        if let Expr::Name(name) = function {
            if let Some(result) = self.try_emit_builtin(&name, arguments) {
                return result;
            }
        }

        // General case: callable dispatch
        let func_temp = self.emit_expression(function_node);
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
    fn try_emit_builtin(
        &mut self,
        name: &str,
        arguments: &[Box<Spannable<Expr>>],
    ) -> Option<String> {
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

    fn emit_tuple_construct(&mut self, items: &[Box<Spannable<Expr>>]) -> String {
        if !items.is_empty() {
            self.emit_line(&format!(
                "RAP_current_pos_start={};",
                items[0].position_start
            ));
            self.emit_line(&format!(
                "RAP_current_pos_end={};",
                items
                    .last()
                    .map(|i| i.position_end)
                    .unwrap_or(items[0].position_start)
            ));
        }

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

impl CGen {
    fn assemble_module_c(
        &self,
        is_main: bool,
        module_base_name: &str,
        extern_decls: &str,
    ) -> String {
        let mut result = String::new();

        if self.check_leaks {
            result.push_str("#define RAP_TEST_LEAKS\n");
        }
        result.push_str("#include \"runtime.h\"\n");
        result.push_str("#include <math.h>\n");
        result.push_str("#include <stdio.h>\n");
        result.push_str("#include <stdlib.h>\n");
        result.push_str("#include <string.h>\n\n");

        if is_main {
            result.push_str(&format!(
                "char* RAP_curret_module_path = \"{}\";\n",
                self.current_module_path
            ));
            result.push_str("size_t RAP_current_pos_start=0;\nsize_t RAP_current_pos_end=0;\n\n");
        } else {
            result.push_str("extern char* RAP_curret_module_path;\n");
            result.push_str(
                "extern size_t RAP_current_pos_start;\nextern size_t RAP_current_pos_end;\n\n",
            );
        }

        if !extern_decls.is_empty() {
            result.push_str(extern_decls);
            result.push('\n');
        }

        result.push_str(&self.forward_decls);

        // MOD_ENTRY function
        result.push_str(&format!(
            "void RAP_{}_MOD_ENTRY(void) {{\n",
            module_base_name
        ));
        result.push_str("  static int _initialized = 0;\n");
        result.push_str("  if (_initialized) return;\n");
        result.push_str("  _initialized = 1;\n");
        result.push_str("  struct RAP_CallFrame _main_frame = {NULL, NULL, 0};\n");
        result.push_str(&format!(
            "  RAP_curret_module_path = \"{}\";\n",
            self.current_module_path
        ));
        result.push('\n');
        result.push_str(&self.output);
        result.push_str("  RAP_free_main_frame(&_main_frame);\n");
        result.push_str("}\n\n");

        if is_main {
            result.push_str("int main(void) {\n");
            result.push_str(&format!("  RAP_{}_MOD_ENTRY();\n", module_base_name));
            result.push_str(&format!(
                "  RAP_FUNC_{}_vkhod(NULL, NULL, 0);\n",
                module_base_name
            ));
            if self.check_leaks {
                result.push_str("  RAP_check_leaks();\n");
            }
            result.push_str("  return 0;\n");
            result.push_str("}\n");
        }

        result
    }
}

impl CodegenTarget for CGen {
    /// Main entry point: generate one C source per module.
    fn generate(&mut self, modules: Vec<Module>) -> ModuleMap {
        let mut result = ModuleMap::new();

        // Find main module (has функ вход())
        let main_module_name = modules
            .iter()
            .find(|m| {
                m.functions
                    .iter()
                    .any(|f| f.node.name.as_deref() == Some("вход"))
            })
            .map(|m| module_base_name(&m.name));

        for module in modules.iter() {
            let base_name = module_base_name(&module.name);
            let is_main = main_module_name.as_ref() == Some(&base_name);

            self.reset_for_module();
            self.current_module_path = module.path.display().to_string();

            if is_main {
                self.entry_module_path = self.current_module_path.clone();
            }

            // Emit definitions
            for type_def in &module.types {
                self.emit_type_def(type_def);
            }
            for func in &module.functions {
                self.emit_function_def(func);
            }
            for proc in &module.procedures {
                self.emit_procedure_def(proc);
            }

            // Collect extern declarations for imported symbols
            let mut extern_decls = String::new();
            for (import_mod_name, imported_names) in &module.imports {
                extern_decls.push_str(&format!(
                    "extern void RAP_{}_MOD_ENTRY(void);\n",
                    import_mod_name
                ));
                debug!("Known callables {:#?}", self.known_callables);
                if let Some(mod_callables) = self.known_callables.get(import_mod_name) {
                    for name in imported_names {
                        if let Some(info) = mod_callables.get(name) {
                            extern_decls.push_str(&format!(
                                "extern RAP_Value {}(struct RAP_CallFrame *_frame,\n{:>width$}RAP_Value *_args, unsigned int _argc);\n",
                                info.c_func_name,
                                "",
                                width = format!("extern RAP_Value {}(", info.c_func_name).len()
                            ));
                        }
                    }
                }
            }

            // Emit toplevel into self.output (becomes MOD_ENTRY body)
            self.create_scope();

            for (import_mod_name, _) in &module.imports {
                self.emit_line(&format!("RAP_{}_MOD_ENTRY();", import_mod_name));
            }

            for stmt in &module.toplevel {
                self.emit_statement(stmt);
            }

            self.drop_scope();

            let code = self.assemble_module_c(is_main, &base_name, &extern_decls);
            result.insert(module.path.display().to_string(), code);
        }

        result
    }

    fn compile(
        &mut self,
        modules_map: ModuleMap,
        current_dir: &PathBuf,
        flags: &[String],
        run: bool,
        dump: bool,
    ) -> Result<(), RunError> {
        let runtime_dir = find_runtime_dir();
        let mut object_files = Vec::new();

        // 1. Compile each module's .c to .o
        for (module_path, module_code) in &modules_map {
            let module_path = PathBuf::from(module_path);
            let file_name = module_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("a");
            let c_path = current_dir.join(PathBuf::from(file_name).with_extension("c"));
            let o_path = current_dir.join(PathBuf::from(file_name).with_extension("o"));

            std::fs::write(&c_path, module_code).unwrap_or_else(|error| {
                eprintln!("error writing {:?}: {error}", c_path);
                std::process::exit(1);
            });

            let status = Command::new("gcc")
                .arg("-c")
                .arg(&c_path)
                .arg("-o")
                .arg(&o_path)
                .arg(format!("-I{}", runtime_dir.display()))
                .arg(format!(
                    "-I{}",
                    runtime_dir.join("raperr/include").display()
                ))
                .args(flags)
                .status()
                .unwrap_or_else(|error| {
                    eprintln!("failed to run gcc: {error}");
                    std::process::exit(1);
                });

            if !status.success() {
                eprintln!("gcc failed with {status}");
                std::process::exit(1);
            }

            if !dump {
                if let Err(error) = std::fs::remove_file(&c_path) {
                    eprintln!("failed to remove {:?}: {error}", c_path);
                }
            }

            object_files.push(o_path);
        }

        // 2. Link all .o files into the final binary
        let entry_path = PathBuf::from(&self.entry_module_path);
        let binary_name = entry_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("a");
        let binary_path = current_dir.join(binary_name);

        let status = Command::new("gcc")
            .args(&object_files)
            .arg("-o")
            .arg(&binary_path)
            .arg(format!("-L{}", runtime_dir.join("lib").display()))
            .arg(format!(
                "-L{}",
                runtime_dir.join("raperr/target/release").display()
            ))
            .arg("-lrapruntime")
            .arg("-lraperr")
            .arg("-lm")
            .status()
            .unwrap_or_else(|error| {
                eprintln!("failed to link: {error}");
                std::process::exit(1);
            });

        if !status.success() {
            eprintln!("linking failed with {status}");
            std::process::exit(1);
        }

        // Clean up .o files
        for o_path in &object_files {
            if let Err(error) = std::fs::remove_file(o_path) {
                eprintln!("failed to remove {:?}: {error}", o_path);
            }
        }

        if run {
            let status = Command::new(&binary_path).status().unwrap_or_else(|error| {
                eprintln!("failed to run {:?}: {error}", binary_path);
                std::process::exit(1);
            });

            if !status.success() {
                eprintln!("{} failed with {status}", binary_path.to_string_lossy());
                std::process::exit(1);
            }

            if let Err(error) = std::fs::remove_file(&binary_path) {
                eprintln!("failed to remove {:?}: {error}", binary_path);
            }
        }

        Ok(())
    }
}

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
#[derive(Debug)]
struct CallableInfo {
    c_func_name: String,
    /// (rapira_param_name, c_mode_constant)
    params: Vec<(String, String)>,
    is_function: bool,
}

fn module_base_name(filename: &str) -> String {
    filename
        .strip_suffix(".рап")
        .or_else(|| filename.strip_suffix(".rap"))
        .unwrap_or(filename)
        .to_string()
}
