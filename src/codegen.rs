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

/// Holds codegen state while walking the AST.
pub struct Codegen {
    /// Accumulated C output
    output: String,
    /// Current indentation depth (number of levels, not spaces)
    indent_level: usize,
    /// Counter for unique temporaries: _t0, _t1, _t2, ...
    temp_counter: usize,
    /// Counter for unique string buffers: _s0, _s1, ...
    string_counter: usize,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            temp_counter: 0,
            string_counter: 0,
        }
    }

    /// Main entry point: walk the whole program, return generated C source.
    pub fn generate(mut self, program: &Program) -> String {
        self.emit_prelude();
        for unit in &program.units {
            self.emit_program_unit(unit);
        }
        self.emit_main_epilogue();
        self.output
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Allocate a fresh temp name `_tN` and bump the counter.
    fn fresh_temp(&mut self) -> String {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    /// Allocate a fresh string buffer name `_sN`.
    fn fresh_string_buf(&mut self) -> String {
        let name = format!("_s{}", self.string_counter);
        self.string_counter += 1;
        name
    }

    /// Write a line at the current indent level.
    fn emit_line(&mut self, line: &str) {
        for _ in 0..self.indent_level {
            self.output.push_str("  ");
        }
        self.output.push_str(line);
        self.output.push('\n');
    }

    /// Convert a Rapira identifier (Cyrillic) to a C-safe name.
    /// Idea: transliterate each Cyrillic char, e.g. А→A, Б→B, В→V, ...
    /// For now, a simple approach: use a mapping table or percent-encode.
    fn mangle_name(&self, rapira_name: &str) -> String {
        // TODO: implement Cyrillic → Latin transliteration
        // For MVP, could just use the Cyrillic bytes hex-encoded,
        // but a readable transliteration (ПРОСТОЕ → PROSTOE) is much nicer.
        // See: GOST 7.79-2000 or ISO 9 for standard transliteration tables.
        format!("_local_{}", rapira_name)
    }

    /// Same as mangle_name but for function definitions (RAP_FUNC_ prefix).
    fn mangle_func_name(&self, rapira_name: &str) -> String {
        format!("RAP_FUNC_{}", rapira_name)
    }

    // ── Prelude / Epilogue ───────────────────────────────────────────────

    fn emit_prelude(&mut self) {
        self.emit_line("#include \"../runtime.h\"");
        self.emit_line("#include <math.h>");
        self.emit_line("#include <stdio.h>");
        self.emit_line("#include <stdlib.h>");
        self.emit_line("");
    }

    /// Wrap top-level statements in `int main(void) { ... }`.
    fn emit_main_epilogue(&mut self) {
        // TODO: collect top-level statements, wrap them in main()
        // See prime.c for the pattern:
        //   struct RAP_CallFrame _main_frame = {NULL, NULL, 0};
        //   ... callable setup ...
        //   ... top-level statements ...
        //   return 0;
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

    /// Emit a function definition as a C function.
    ///
    /// Pattern (from prime.c):
    /// ```c
    /// RAP_Object *RAP_FUNC_NAME(struct RAP_CallFrame *_frame,
    ///                            RAP_Object **_args, unsigned int _argc) {
    ///   RAP_Object *_local_PARAM = _args[0];
    ///   // ... body ...
    /// }
    /// ```
    ///
    /// Then in main(), register it as a callable:
    /// ```c
    /// RAP_Parameter *_p0 = RAP_create_parameter(RAP_PARAMETER_MODE_IN, "N");
    /// RAP_Object *_local_NAME = RAP_create_callable_obj(&_main_frame, &RAP_FUNC_NAME, &_p0, 1);
    /// ```
    fn emit_function_def(&mut self, _func_def: &FunctionDefinition) {
        // TODO: two things happen here:
        //
        // 1. Emit the C function at file scope (always top-level in C):
        //      RAP_Object *RAP_FUNC_NAME(struct RAP_CallFrame *_frame,
        //                                 RAP_Object **_args, unsigned int _argc) { ... }
        //
        // 2. At the *current* scope position (wherever функ appeared),
        //    emit the callable registration — the enclosing scope "owns" it:
        //      RAP_Parameter *_p0 = RAP_create_parameter(RAP_PARAMETER_MODE_IN, "N");
        //      RAP_Object *_local_NAME = RAP_create_callable_obj(&_frame, &RAP_FUNC_NAME, &_p0, 1);
        //
        //    So in top-level code this lands in main(), but if функ appeared
        //    inside another function, it would land in that function's body.
        //
        // Practical approach: buffer the C function body separately (e.g. into
        // a Vec<String> of "forward" definitions), then flush them before main().
        // The registration line is emitted inline as a normal statement.
    }

    fn emit_procedure_def(&mut self, _proc_def: &ProcedureDefinition) {
        // TODO: same pattern as function but returns void (or RAP_Object* NULL)
        // Procedures use RAP_PARAMETER_MODE_IN / RAP_PARAMETER_MODE_OUT
    }

    // ── Statements ───────────────────────────────────────────────────────

    fn emit_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Empty => {}

            Statement::Assignment { target, value } => {
                // Idea: emit the expression (gets a temp name back),
                // then assign to the target's mangled name.
                // For simple LValue::Name: `_local_X = _tN;`
                // For subscript: `RAP_set_tuple_item(_local_X, idx_temp, _tN);`
                let _value_temp = self.emit_expression(value);
                self.emit_lvalue_assignment(target, &_value_temp);
            }

            Statement::ProcedureCall { procedure, arguments } => {
                // TODO: emit arguments, then call
                // `RAP_call_callable_obj(_local_PROC, args_array, count);`
                let _ = (procedure, arguments);
            }

            Statement::Conditional { condition, then_body, else_body } => {
                // Pattern:
                //   RAP_Object *_tN = <condition>;
                //   if (_tN->logical_val) { ... }
                //   [else { ... }]
                let _cond_temp = self.emit_expression(condition);
                let _ = (then_body, else_body);
            }

            Statement::Selection(_sel) => {
                // TODO: ValueMatch → switch-like chain of if/else
                //       ConditionList → chain of if/else if
            }

            Statement::Loop(loop_stmt) => {
                self.emit_loop(loop_stmt);
            }

            Statement::Output { no_newline, values } => {
                // For each value: emit expression, stringify, printf
                // Pattern:
                //   RAP_Object *_tN = <expr>;
                //   char *_sM = RAP_stringify_object(_tN);
                //   printf("%s\n", _sM);  // or "%s" if no_newline
                let _ = (no_newline, values);
            }

            Statement::Input { text_mode, variables } => {
                // TODO: needs runtime support for scanf/readline
                let _ = (text_mode, variables);
            }

            Statement::ExitLoop => {
                self.emit_line("break;");
            }

            Statement::ReturnFromProcedure => {
                self.emit_line("return RAP_create_int_obj(0); // void return");
            }

            Statement::ReturnFromFunction(expr) => {
                let _result_temp = self.emit_expression(expr);
                // self.emit_line(&format!("return {};", result_temp));
            }
        }
    }

    fn emit_lvalue_assignment(&mut self, _target: &LValue, _value_temp: &str) {
        // TODO: match on LValue variant
        // Name → `_local_X = value_temp;`
        // Subscript → `RAP_set_tuple_item(...)`
        // Slice → runtime support needed
    }

    // ── Loops ────────────────────────────────────────────────────────────

    fn emit_loop(&mut self, _loop_stmt: &LoopStatement) {
        // Idea: match on loop_stmt.header
        //
        // LoopHeader::For { variable, from, to, step }:
        //   RAP_Object *_from = <from or 1>;
        //   RAP_Object *_to = <to>;
        //   for (int64_t _iter_X = RAP_get_int_val(_from);
        //        _iter_X <= (int64_t)RAP_get_float_val(_to);
        //        _iter_X += step) {
        //     RAP_Object *_local_X = RAP_create_int_obj(_iter_X);
        //     ... body ...
        //   }
        //
        // LoopHeader::Repeat(n):
        //   RAP_Object *_tN = <n>;
        //   for (int64_t _rep = 0; _rep < RAP_get_int_val(_tN); _rep++) { ... }
        //
        // LoopHeader::Infinite:
        //   while (1) { ... }
        //
        // while_condition (пока): add `if (!cond->logical_val) break;` at loop top
        // post_condition (по):   add `if (cond->logical_val) break;` at loop bottom
    }

    // ── Expressions ──────────────────────────────────────────────────────

    /// Emit code for an expression. Returns the temp variable name holding the result.
    ///
    /// This is the core pattern: every expression becomes one or more lines
    /// that end with `RAP_Object *_tN = ...;`, and we return `"_tN"`.
    fn emit_expression(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(lit) => self.emit_literal(lit),
            Expr::Name(name) => {
                // Just reference the local: no temp needed, return mangled name directly
                self.mangle_name(name)
            }
            Expr::BinaryOp { operator, left, right } => {
                let left_temp = self.emit_expression(left);
                let right_temp = self.emit_expression(right);
                self.emit_binary_op(operator, &left_temp, &right_temp)
            }
            Expr::UnaryOp { operator, operand } => {
                let operand_temp = self.emit_expression(operand);
                self.emit_unary_op(operator, &operand_temp)
            }
            Expr::FunctionCall { function, arguments } => {
                self.emit_function_call(function, arguments)
            }
            Expr::TupleConstruct(items) => {
                self.emit_tuple_construct(items)
            }
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
                // TODO: runtime slice support
                let result = self.fresh_temp();
                self.emit_line(&format!("RAP_Object *{} = NULL; // TODO: slice", result));
                result
            }
        }
    }

    fn emit_literal(&mut self, lit: &Literal) -> String {
        let temp = self.fresh_temp();
        let rhs = match lit {
            Literal::Null => "NULL /* пусто */".to_string(),
            Literal::Boolean(b) => format!("RAP_create_logical_obj({})", b),
            Literal::Integer(n) => format!("RAP_create_int_obj({})", n),
            Literal::Real(f) => format!("RAP_create_float_obj({})", f),
            Literal::Text(s) => format!("RAP_create_text_obj(\"{}\")", s), // TODO: escape
        };
        self.emit_line(&format!("RAP_Object *{} = {};", temp, rhs));
        temp
    }

    fn emit_binary_op(
        &mut self,
        op: &BinaryOperator,
        left: &str,
        right: &str,
    ) -> String {
        let temp = self.fresh_temp();
        // Idea: map each BinaryOperator to a runtime call or C expression.
        //
        // For integer ops we already have runtime helpers:
        //   Add → RAP_create_int_obj(RAP_get_int_val(l) + RAP_get_int_val(r))
        //   Less → RAP_integer_less_than(l, r)
        //   Equal → RAP_integer_equal(l, r)
        //   Remainder → RAP_integer_modulo(l, r)
        //   etc.
        //
        // Challenge: Rapira is dynamically typed — the same `+` could mean
        // int add, float add, or string concat depending on runtime tags.
        // Options:
        //   1. Emit type-checking dispatch: if (l->tag == INT && r->tag == INT) ...
        //   2. Create runtime polymorphic helpers: RAP_add(l, r)
        //   3. For Phase 1, assume types match and emit the int version (simplest)
        //
        // For now, option 3 (fill in polymorphic dispatch later):
        let _rhs = match op {
            BinaryOperator::Add => format!("RAP_create_int_obj(RAP_get_int_val({}) + RAP_get_int_val({}))", left, right),
            BinaryOperator::Less => format!("RAP_integer_less_than({}, {})", left, right),
            BinaryOperator::Equal => format!("RAP_integer_equal({}, {})", left, right),
            BinaryOperator::Remainder => format!("RAP_integer_modulo({}, {})", left, right),
            // TODO: Subtract, Multiply, Divide, IntegerDivide, Power,
            //       Greater, GreaterOrEqual, LessOrEqual, NotEqual,
            //       And, Or
            _ => format!("NULL /* TODO: {:?} */", op),
        };
        self.emit_line(&format!("RAP_Object *{} = {};", temp, _rhs));
        temp
    }

    fn emit_unary_op(&mut self, _op: &UnaryOperator, _operand: &str) -> String {
        let temp = self.fresh_temp();
        // TODO: Negate → RAP_create_int_obj(-RAP_get_int_val(operand))
        //       Not → RAP_create_logical_obj(!operand->logical_val)
        //       Length → runtime support (#)
        //       Plus → no-op, return operand
        self.emit_line(&format!("RAP_Object *{} = NULL; // TODO: unary {:?}", temp, _op));
        temp
    }

    fn emit_function_call(
        &mut self,
        function: &Expr,
        arguments: &[Box<Expr>],
    ) -> String {
        // Idea:
        // 1. Emit each argument expression → get temp names
        // 2. Build a C array: RAP_Object *_args[] = {_t0, _t1, ...};
        //    Or for single-arg: just pass &_tN
        // 3. Emit: RAP_Object *_tM = RAP_call_callable_obj(func_temp, args, count);
        //
        // Special case: built-in functions (корень, длин, etc.)
        // could be detected by name and emitted as direct C calls.
        // E.g. корень(X) → RAP_create_float_obj(sqrt((double)RAP_get_int_val(X)))
        let _ = (function, arguments);
        let temp = self.fresh_temp();
        self.emit_line(&format!("RAP_Object *{} = NULL; // TODO: function call", temp));
        temp
    }

    fn emit_tuple_construct(&mut self, _items: &[Box<Expr>]) -> String {
        // Idea: emit each item, collect temps, then:
        //   RAP_Object *_items[] = {_t0, _t1, ...};
        //   RAP_Object *_tN = RAP_create_tuple_obj(count, _items);
        let temp = self.fresh_temp();
        self.emit_line(&format!("RAP_Object *{} = NULL; // TODO: tuple", temp));
        temp
    }
}
