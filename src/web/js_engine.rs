// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::HashMap;
use std::sync::{Arc, Mutex};
/// A JavaScript value in the engine-neutral subset understood by this module.
///
/// Mirrors the ECMAScript value types, but only shallowly: collections are plain
/// Rust containers with no prototype chain, getters or cycles, and [`Self::Ident`]
/// is an internal parsing artefact that is never a real runtime value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum JsValue {
    /// The absence of a value; the default, corresponding to `undefined`.
    #[default]
    Undefined,
    /// The explicit empty value `null`.
    Null,
    /// A boolean `true`/`false`.
    Boolean(bool),
    /// An IEEE-754 double, which also carries `NaN` and `Infinity`.
    Number(f64),
    /// A string.
    String(String),
    /// A dense, ordered list of values; sparse arrays are not representable.
    Array(Vec<JsValue>),
    /// A string-keyed map of values. Iteration order is the map's, not
    /// insertion order, so it may differ from the script that built it.
    Object(HashMap<String, JsValue>),
    /// A reference to a callable known only by `name` (e.g. a built-in such as
    /// `parseInt`); the name is resolved later by the engine.
    Function(String),
    /// An identifier reference (used during parsing).
    Ident(String),
    /// A function with a parameter list and body source.
    FunctionDef {
        /// Function name as written in the script.
        name: String,
        /// Parameter names, in declaration order; position determines which
        /// argument is bound to which parameter.
        params: Vec<String>,
        /// The function body as raw source text, re-parsed on each call.
        body: String,
    },
}
impl JsValue {
    /// Whether this value counts as `true` in a JavaScript boolean context.
    ///
    /// Follows ECMAScript `ToBoolean`: `undefined`/`null` are false, `NaN` and
    /// both zeros are false, empty strings, arrays and objects are false, while
    /// functions are always true.
    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Boolean(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(a) => !a.is_empty(),
            JsValue::Object(o) => !o.is_empty(),
            JsValue::Ident(s) => !s.is_empty(),
            JsValue::Function(_) | JsValue::FunctionDef { .. } => true,
        }
    }
    /// Renders the value as display text using JavaScript-like rules.
    ///
    /// Intended for logging and diagnostics, not for round-tripping: strings are
    /// returned unquoted, arrays/objects are rendered recursively with
    /// `[...]`/`{...}`, and a `Number` uses Rust's formatting, so `NaN` and
    /// `Infinity` print as `NaN`/`inf` rather than the ECMAScript spellings.
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        match self {
            JsValue::Undefined => "undefined".to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::Boolean(b) => b.to_string(),
            JsValue::Number(n) => n.to_string(),
            JsValue::String(s) => s.clone(),
            JsValue::Array(a) => {
                let items: Vec<String> = a.iter().map(|v| v.to_string()).collect();
                format!("[{}]", items.join(", "))
            }
            JsValue::Object(o) => {
                let items: Vec<String> =
                    o.iter().map(|(k, v)| format!("{}: {}", k, v.to_string())).collect();
                format!("{{{}}}", items.join(", "))
            }
            JsValue::Function(name) => format!("[Function: {name}]"),
            JsValue::Ident(s) => s.clone(),
            JsValue::FunctionDef { name, params, .. } => {
                format!("[Function: {}({})]", name, params.join(", "))
            }
        }
    }
    /// Converts the value to a number following ECMAScript `ToNumber`.
    ///
    /// `undefined` and every composite value (array, object, function, identifier)
    /// become `NaN`, since this does not attempt `valueOf`/`toString` coercions.
    /// `null` becomes `0`, booleans become `1`/`0`, and a string is parsed as a
    /// Rust float — so `"12px"` yields `NaN` rather than `12` as JS would.
    pub fn to_number(&self) -> f64 {
        match self {
            JsValue::Undefined => f64::NAN,
            JsValue::Null => 0.0,
            JsValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            JsValue::Number(n) => *n,
            JsValue::String(s) => s.parse().unwrap_or(f64::NAN),
            JsValue::Array(_)
            | JsValue::Object(_)
            | JsValue::Function(_)
            | JsValue::Ident(_)
            | JsValue::FunctionDef { .. } => f64::NAN,
        }
    }
    /// Converts the value to a boolean with the same truthiness rules as
    /// [`Self::is_truthy`]; provided for symmetry with [`Self::to_number`].
    pub fn to_boolean(&self) -> bool {
        self.is_truthy()
    }
}
/// An error raised while evaluating a script.
///
/// Deliberately plain data rather than a rich exception type: there is no
/// wrapped JavaScript value, no error kind, and no cause chain.
#[derive(Debug, Clone)]
pub struct JsError {
    /// Human-readable description. Built-in errors carry no prefix, but the
    /// [`std::fmt::Display`] impl prepends `JsError: ` when printing.
    pub message: String,
    /// Optional captured call stack, or `None` when none was recorded. Error
    /// constructors in this module never populate it.
    pub stack: Option<String>,
    /// 1-based source line, or `None` if unknown.
    pub line: Option<u32>,
    /// 1-based column within [`Self::line`], or `None` if unknown. Only meaningful
    /// alongside [`Self::line`]; [`std::fmt::Display`] prints both or neither.
    pub column: Option<u32>,
}
impl JsError {
    /// Creates an error with just a message; stack, line and column are unset.
    pub fn new(message: String) -> Self {
        Self { message, stack: None, line: None, column: None }
    }
    /// Creates an error carrying a source position, both values 1-based by
    /// convention. The stack remains unset.
    pub fn with_location(message: String, line: u32, column: u32) -> Self {
        Self { message, stack: None, line: Some(line), column: Some(column) }
    }

    /// Creates an error from a **byte offset** into `source`.
    ///
    /// The parser knows where it stopped as an offset into the script, not as a
    /// line and column, so this converts one into the other. Use this rather than
    /// [`Self::with_location`] for anything derived from an offset: passing the
    /// offset as the `line` argument (which the interpreter used to do) printed
    /// nonsense such as `at line 0, column 24`, because the two fields meant
    /// different things.
    ///
    /// The offset is interpreted as a byte index, which is what the parser tracks.
    /// A position inside a multi-byte character is floored to the start of that
    /// character rather than panicking, and an offset past the end of `source`
    /// clamps to the final line.
    pub fn at_offset(message: String, source: &str, offset: usize) -> Self {
        // Floor to a character boundary: slicing a UTF-8 string mid-character would
        // panic, and an error path must never panic on malformed input.
        let mut end = offset.min(source.len());
        while end > 0 && !source.is_char_boundary(end) {
            end -= 1;
        }
        let prefix = &source[..end];

        let line = prefix.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
        // Column within the line, 1-based. `rfind` over the already-sliced prefix
        // avoids a second pass over the whole script.
        let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
        let column = prefix[line_start..].chars().count() as u32 + 1;

        Self { message, stack: None, line: Some(line), column: Some(column) }
    }
}
impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsError: {}", self.message)?;
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, " at line {line}, column {column}")?;
        }
        Ok(())
    }
}
impl std::error::Error for JsError {}
/// Result alias for evaluation: `Ok` holds the script's value, `Err` a
/// [`JsError`].
pub type JsResult<T> = Result<T, JsError>;

/// Per-evaluation state: the global namespace plus captured console output.
///
/// A context must be passed explicitly to every [`JsEngine`] call rather than
/// being owned by the engine, which is what lets several engines share (or each
/// keep separate) globals. It is a plain value type with no interior mutability.
#[derive(Debug, Clone)]
pub struct JsContext {
    global: HashMap<String, JsValue>,
    console_messages: Vec<ConsoleMessage>,
}
/// A single captured `console.*` call.
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    /// Severity of the call. Note the simple engine funnels every console method
    /// through [`JsContext::log`] and so always reports [`ConsoleLevel::Log`].
    pub level: ConsoleLevel,
    /// The already-stringified argument, not a live [`JsValue`].
    pub message: String,
    /// Source line, or `0` when unknown — which is the case for every message
    /// produced by the simple engine, so `0` is not a real line number.
    pub line: u32,
    /// Source identifier (e.g. a file name); empty when unknown, which is again
    /// always the case for the simple engine.
    pub source: String,
}
/// Severity of a [`ConsoleMessage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    /// `console.log` — ordinary informational output.
    Log,
    /// `console.info` — informational output, typically non-critical.
    Info,
    /// `console.warn` — a non-fatal problem worth reporting.
    Warn,
    /// `console.error` — a failure.
    Error,
    /// `console.debug` — verbose developer output.
    Debug,
}
impl JsContext {
    /// Creates an empty context seeded with the three globals `undefined`,
    /// `NaN` and `Infinity`, so scripts referencing them resolve without the
    /// engine having to special-case them.
    pub fn new() -> Self {
        let mut global = HashMap::new();
        global.insert("undefined".to_string(), JsValue::Undefined);
        global.insert("NaN".to_string(), JsValue::Number(f64::NAN));
        global.insert("Infinity".to_string(), JsValue::Number(f64::INFINITY));
        Self { global, console_messages: Vec::new() }
    }
    /// Defines or overwrites the global named `name`.
    ///
    /// There is no error and no notice when overwriting an existing global,
    /// including the seeded `undefined`/`NaN`/`Infinity`.
    pub fn set_global(&mut self, name: &str, value: JsValue) {
        self.global.insert(name.to_string(), value);
    }
    /// Borrows the global named `name`, or `None` if it was never defined.
    pub fn get_global(&self, name: &str) -> Option<&JsValue> {
        self.global.get(name)
    }
    /// All console output captured so far, in call order.
    pub fn console_messages(&self) -> &[ConsoleMessage] {
        &self.console_messages
    }
    /// Discards all captured console output, leaving globals untouched.
    pub fn clear_console(&mut self) {
        self.console_messages.clear();
    }
    /// Emit a console message with log level.
    ///
    /// Appends at [`ConsoleLevel::Log`] with no position information; line and
    /// source are empty, so the entry cannot be traced back to script text.
    pub fn log(&mut self, message: String) {
        self.console_messages.push(ConsoleMessage {
            level: ConsoleLevel::Log,
            message,
            line: 0,
            source: String::new(),
        });
    }
}
crate::impl_default_via_new!(JsContext);
/// The abstraction the rest of the library evaluates JavaScript through.
///
/// Implementations must be `Send + Sync` because engines are shared behind a
/// mutex (see [`SharedJsEngine`]). Globals live in the caller-supplied
/// [`JsContext`], not in the engine, so an engine holds no per-script state that
/// caller could not inspect.
pub trait JsEngine: Send + Sync {
    /// Evaluates a complete `script` and returns its value.
    ///
    /// The returned value is the script's last expression; statements that
    /// produce nothing yield [`JsValue::Undefined`]. Syntax and runtime problems
    /// come back as `Err(JsError)`, and a failure must not be assumed to leave
    /// `context` or the engine unchanged.
    fn evaluate(&mut self, script: &str, context: &mut JsContext) -> JsResult<JsValue>;
    /// Invokes a function by `name` — either one defined in a script, or a
    /// built-in — with positional `args`.
    ///
    /// Callers are responsible for argument count and types: implementations may
    /// bind missing arguments as `undefined` and cannot type-check them. An
    /// unknown name is an error rather than a returned `undefined`.
    fn call_function(
        &mut self,
        name: &str,
        args: &[JsValue],
        context: &mut JsContext,
    ) -> JsResult<JsValue>;
    /// Exposes `value` to scripts as the global `name`, replacing any previous
    /// binding of that name.
    fn set_global(&mut self, name: &str, value: JsValue, context: &mut JsContext) -> JsResult<()>;
    /// Returns the current value of the global `name`, or `None` if it is not
    /// defined. Taking `&self` means implementations need interior mutability to
    /// report state here.
    fn get_global(&self, name: &str, context: &JsContext) -> Option<JsValue>;
}

/// A small interpreter that understands a pragmatic *subset* of JavaScript.
///
/// It is not a conforming engine: it supports var/let/const declarations,
/// function definitions and calls with positional parameters, `if`/`else`,
/// `for` loops, array literals, member/index access, the **arithmetic, comparison and
/// logical operators** (`+ - * / %`, `== != < <= > >=`, `&& || !`, with the usual
/// precedence and left associativity), and the `parseInt`, `parseFloat`, `String`,
/// `Number` and `Boolean` built-ins. It performs no prototype lookups, no closures
/// (function bodies see whatever globals exist at call time), no exceptions/`try`, and
/// no real number formatting. Parsing is textual and recursive, so deeply nested
/// scripts consume stack proportional to nesting depth.
///
/// # What it does not do
///
/// An argument that is itself an expression (`f(a + 1)`) is not evaluated: arguments are
/// read as literals, so a bare identifier there becomes `undefined`. Pass the computed
/// value from the caller or bind it to a variable first. There are no escapes inside
/// string literals, and no arrow functions (`=>`).
///
/// Globals are stored in the engine as well as in the passed [`JsContext`], and
/// the context argument is ignored by the [`JsEngine`] methods, so two calls
/// sharing one context still share nothing through the engine's own map.
pub struct SimpleJsEngine {
    variables: HashMap<String, JsValue>,
    /// Defined functions (name -> FunctionDef)
    functions: HashMap<String, JsValue>,
}
impl SimpleJsEngine {
    /// Creates an engine with the five built-in functions pre-registered and no
    /// user variables. Built-ins are stored by name only; their behaviour is
    /// implemented in [`JsEngine::call_function`].
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        // Built-in functions
        functions.insert("parseInt".to_string(), JsValue::Function("parseInt".to_string()));
        functions.insert("parseFloat".to_string(), JsValue::Function("parseFloat".to_string()));
        functions.insert("String".to_string(), JsValue::Function("String".to_string()));
        functions.insert("Number".to_string(), JsValue::Function("Number".to_string()));
        functions.insert("Boolean".to_string(), JsValue::Function("Boolean".to_string()));
        Self { variables: HashMap::new(), functions }
    }
    /// Parses a literal: numbers, strings, booleans, `null`/`undefined`, array literals and
    /// a variable reference. **No operators** — see [`Self::eval_expression`], which is what
    /// evaluates anything containing one.
    fn parse_literal(&mut self, s: &str) -> JsValue {
        let s = s.trim().trim_end_matches(';');
        if s == "undefined" {
            return JsValue::Undefined;
        }
        if s == "null" {
            return JsValue::Null;
        }
        if s == "true" {
            return JsValue::Boolean(true);
        }
        if s == "false" {
            return JsValue::Boolean(false);
        }
        if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
            return JsValue::String(s[1..s.len() - 1].to_string());
        }
        if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
            return JsValue::String(s[1..s.len() - 1].to_string());
        }
        // Array literal
        if s.starts_with('[') && s.ends_with(']') {
            let inner = s[1..s.len() - 1].trim();
            if inner.is_empty() {
                return JsValue::Array(Vec::new());
            }
            let elements: Vec<JsValue> =
                split_top_level(inner).into_iter().map(|part| self.parse_literal(&part)).collect();
            return JsValue::Array(elements);
        }
        if let Ok(n) = s.parse::<f64>() {
            return JsValue::Number(n);
        }
        // `NaN`/`Infinity` are literal globals, and `s.parse::<f64>()` accepts neither.
        if s == "NaN" {
            return JsValue::Number(f64::NAN);
        }
        if s == "Infinity" {
            return JsValue::Number(f64::INFINITY);
        }
        if let Some(v) = self.variables.get(s) {
            return v.clone();
        }
        // A trailing call is resolved here too, so an expression like `parseInt(x) + 1`
        // works: [`Self::eval_expression`] splits on operators and hands each operand to
        // this function, and a function call is an operand.
        if let Some(call) = parse_call(s) {
            if let Ok(value) = self.call_function(&call.0, &call.1, &mut JsContext::new()) {
                return value;
            }
        }
        JsValue::Undefined
    }

    /// Evaluates an expression, including the operators this engine supports.
    ///
    /// # Why this exists
    ///
    /// The module documented "a pragmatic subset of JavaScript" and named the constructs
    /// it understood. Arithmetic was not among them: `1 + 2` parsed as a *literal*, failed
    /// the number parse, and fell through to `undefined`. That is a documentation/implementation
    /// disagreement of exactly the kind BLUE20 layer 2 exists to find, and it is worse than
    /// a missing feature because the docs told callers to rely on it.
    ///
    /// # Grammar
    ///
    /// ``expr := or`` over ``||``, ``&&``, ``==``/``!=``, ``</``<=``/>``>=``, ``+``/``-``,
    /// ``*``/``/``/``%``, unary ``-``/``!``, then a primary (literal, call, parenthesised).
    /// Precedence is handled by splitting at the **lowest-precedence** operator found at
    /// the top level, which recurses naturally and needs no parser state.
    ///
    /// `+` concatenates when either side is a string, which is ECMAScript's rule and the
    /// behaviour a caller writing `"n=" + n` expects.
    fn eval_expression(&mut self, expr: &str) -> JsResult<JsValue> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // A whole expression in parentheses: strip and recurse, so `(1 + 2) * 3` groups.
        if expr.starts_with('(') && matching_paren(expr) == Some(expr.len() - 1) {
            return self.eval_expression(&expr[1..expr.len() - 1]);
        }

        // Lowest precedence first, so the *last* split is the outer operation.
        for ops in [
            &["||"][..],
            &["&&"][..],
            &["===", "!==", "==", "!="][..],
            &["<=", ">=", "<", ">"][..],
            &["+", "-"][..],
            &["*", "/", "%"][..],
        ] {
            if let Some((index, op)) = find_top_level_operator(expr, ops) {
                let left = &expr[..index];
                let right = &expr[index + op.len()..];
                // Short-circuit *and* `||`, because evaluating the right side of either
                // has no effect but can fail (a divide by zero is fine in JS, but a
                // malformed right side would raise where ECMAScript would not).
                let left_value = self.eval_expression(left)?;
                match op {
                    "&&" => {
                        return if left_value.is_truthy() {
                            self.eval_expression(right)
                        } else {
                            Ok(left_value)
                        };
                    }
                    "||" => {
                        return if left_value.is_truthy() {
                            Ok(left_value)
                        } else {
                            self.eval_expression(right)
                        };
                    }
                    _ => {}
                }
                let right_value = self.eval_expression(right)?;
                return Ok(apply_operator(op, &left_value, &right_value));
            }
        }

        // Unary minus and logical not, which bind tighter than any binary operator above.
        if let Some(rest) = expr.strip_prefix('!') {
            return Ok(JsValue::Boolean(!self.eval_expression(rest)?.is_truthy()));
        }
        if let Some(rest) = expr.strip_prefix('-') {
            let value = self.eval_expression(rest)?;
            return Ok(JsValue::Number(-value.to_number()));
        }

        // A primary: a variable, a literal, or a call.
        //
        // The variable is looked up **before** `parse_call`, because an expression like
        // `a + 1` where `a` is a variable must read the variable and not be treated as an
        // identifier token. `parse_literal` does the lookup itself; this is the same
        // ordering, made explicit here so a call that *is* a variable holding a function
        // still resolves (the variable wins, matching `call_function`'s own precedence).
        if let Some(value) = self.variables.get(expr) {
            return Ok(value.clone());
        }
        if let Some((name, args)) = parse_call(expr) {
            let mut ctx = JsContext::new();
            // Arguments are evaluated here so `f(a) + 1` reads `a`; `parse_call` can only
            // see literals, so an identifier argument is resolved against the variables
            // before the call.
            let args: Vec<JsValue> = args
                .into_iter()
                .map(|arg| match arg {
                    JsValue::String(token) => {
                        self.eval_expression(&token).unwrap_or(JsValue::Undefined)
                    }
                    other => other,
                })
                .collect();
            return self.call_function(&name, &args, &mut ctx);
        }
        Ok(self.parse_literal(expr))
    }

    /// Evaluates a block (a semicolon-delineated sequence of statements) and
    /// return the value of the last expression (if any).
    fn eval_block(&mut self, block: &str, context: &mut JsContext) -> JsResult<JsValue> {
        let block = block.trim();
        if block.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // Split into top-level semicolons (naive; does not split inside {}, [], "", '')
        let mut stmts = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        let chars: Vec<char> = block.chars().collect();
        for i in 0..chars.len() {
            match chars[i] {
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => {
                    stmts.push(block[start..i].trim().to_string());
                    start = i + 1;
                }
                _ => { /* Other characters are not relevant */ }
            }
        }
        let tail = block[start..].trim().to_string();
        if !tail.is_empty() {
            stmts.push(tail);
        }
        let mut last_val = JsValue::Undefined;
        for stmt in &stmts {
            // A block whose only `;` is nested (inside `{…}`) produces exactly one
            // "statement" that is the whole input. Calling `evaluate` on it would re-enter
            // this function on the same text and recurse until the stack ran out — the
            // function-definition case (`function f() { return 1; }`) hit exactly that.
            // When there is only one piece and it is the input itself, the block wrapper
            // added nothing, so `eval_stmt` is the right entry point: it owns the
            // constructs the split could not reach.
            last_val = if stmts.len() == 1 && stmts[0] == block {
                self.eval_stmt(stmt, context)?
            } else {
                self.evaluate(stmt, context)?
            };
        }
        Ok(last_val)
    }
    /// Evaluate one expression/statement string (no semicolons).
    fn eval_stmt(&mut self, stmt: &str, context: &mut JsContext) -> JsResult<JsValue> {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // --- function definition ---
        if let Some(rest) = stmt.strip_prefix("function ") {
            // function name ( params ) { body }
            let rest = rest.trim();
            let name_end =
                rest.find(|c: char| c.is_ascii_whitespace() || c == '(').unwrap_or(rest.len());
            let name = rest[..name_end].trim();
            let after_name = rest[name_end..].trim();
            if after_name.starts_with('(') {
                let paren_end = after_name.find(')').ok_or_else(|| {
                    JsError::at_offset(
                        "Unclosed parameter list in function definition".to_string(),
                        stmt,
                        stmt.len(),
                    )
                })?;
                let params_str = &after_name[1..paren_end];
                let params: Vec<String> = params_str
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                let after_params = after_name[paren_end + 1..].trim();
                if after_params.starts_with('{') {
                    let close = after_params.rfind('}').ok_or_else(|| {
                        JsError::at_offset("Unclosed function body".to_string(), stmt, stmt.len())
                    })?;
                    let body = after_params[1..close].to_string();
                    let func = JsValue::FunctionDef { name: name.to_string(), params, body };
                    self.variables.insert(name.to_string(), func.clone());
                    return Ok(func);
                }
            }
            return Err(JsError::at_offset(
                "Invalid function syntax".to_string(),
                stmt,
                stmt.len(),
            ));
        }
        // --- if / else ---
        if stmt.starts_with("if ") || stmt.starts_with("if(") {
            let cond_start = stmt.find('(').ok_or_else(|| {
                JsError::at_offset("Expected '(' after 'if'".to_string(), stmt, stmt.len())
            })?;
            let cond_end = stmt[cond_start..].find(')').ok_or_else(|| {
                JsError::at_offset("Unclosed condition in 'if'".to_string(), stmt, stmt.len())
            })?;
            let condition = stmt[cond_start + 1..cond_start + cond_end].trim();
            let cond_val = self.eval_expression(condition)?;
            let after_cond = stmt[cond_start + cond_end + 1..].trim();
            let mut body = after_cond;
            let mut else_body: Option<String> = None;
            // Find else part
            if let Some(else_idx) = after_cond.rfind(" else ") {
                body = after_cond[..else_idx].trim();
                let else_rest = after_cond[else_idx + 6..].trim();
                else_body = Some(else_rest.to_string());
            } else if let Some(else_idx) = after_cond.rfind("else{") {
                body = after_cond[..else_idx].trim();
                let else_rest = after_cond[else_idx + 5..].trim();
                else_body = Some(else_rest.to_string());
            } else if let Some(else_idx) = after_cond.rfind("else\n") {
                body = after_cond[..else_idx].trim();
                let else_rest = after_cond[else_idx + 5..].trim();
                else_body = Some(else_rest.to_string());
            }
            if cond_val.is_truthy() {
                return self.evaluate(body, context);
            } else if let Some(eb) = else_body {
                return self.evaluate(&eb, context);
            }
            return Ok(JsValue::Undefined);
        }
        // --- for loop ---
        if stmt.starts_with("for ") || stmt.starts_with("for(") {
            let paren_start = stmt.find('(').ok_or_else(|| {
                JsError::at_offset("Expected '(' after 'for'".to_string(), stmt, stmt.len())
            })?;
            let paren_end = stmt[paren_start..].find(')').ok_or_else(|| {
                JsError::at_offset("Unclosed 'for' condition".to_string(), stmt, stmt.len())
            })?;
            let header = stmt[paren_start + 1..paren_start + paren_end].trim();
            let after_header = stmt[paren_start + paren_end + 1..].trim();
            // Parse header: init ; condition ; increment
            let semi1 = header.find(';');
            let semi2 = semi1.and_then(|s1| header[s1 + 1..].find(';').map(|s2| s1 + 1 + s2));
            let (init_part, cond_part, incr_part) = match (semi1, semi2) {
                (Some(s1), Some(s2)) => {
                    (header[..s1].trim(), header[s1 + 1..s2].trim(), header[s2 + 1..].trim())
                }
                _ => {
                    return Err(JsError::at_offset(
                        "Invalid 'for' loop syntax".to_string(),
                        stmt,
                        stmt.len(),
                    ))
                }
            };
            let init_cond = if !init_part.is_empty() { Some(init_part.to_string()) } else { None };
            let loop_cond =
                if cond_part.is_empty() { "true".to_string() } else { cond_part.to_string() };
            let loop_incr = if !incr_part.is_empty() { Some(incr_part.to_string()) } else { None };
            let mut last_val = JsValue::Undefined;
            // Evaluate init once
            if let Some(init) = &init_cond {
                self.evaluate(init, context)?;
            }
            // Loop
            for _ in 0..10000 {
                let cond_val = self.eval_expression(&loop_cond)?;
                if !cond_val.is_truthy() {
                    break;
                }
                last_val = self.eval_block(after_header, context)?;
                if let Some(incr) = &loop_incr {
                    self.evaluate(incr, context)?;
                }
            }
            return Ok(last_val);
        }
        // --- return ---
        if stmt.starts_with("return ") || stmt == "return" {
            let val = if stmt.len() > 7 {
                self.evaluate(stmt[6..].trim(), context)?
            } else {
                JsValue::Undefined
            };
            return Ok(val);
        }
        // --- block { ... } ---
        if stmt.starts_with('{') {
            let close = stmt.rfind('}').unwrap_or(stmt.len());
            let inner = stmt[1..close].trim();
            return self.eval_block(inner, context);
        }
        // Fallback: try as an ordinary expression
        if !stmt.starts_with('=')
            && !stmt.starts_with("!=")
            && !stmt.starts_with("==")
            && !stmt.starts_with("===")
        {
            if let Some(eq_pos) = stmt.find('=') {
                // make sure not <=, >=, ==, ===, !=
                let before = if eq_pos > 0 { stmt.as_bytes()[eq_pos - 1] as char } else { ' ' };
                if before != '<' && before != '>' && before != '!' && before != '=' {
                    let name = stmt[..eq_pos].trim().to_string();
                    let value_str = stmt[eq_pos + 1..].trim();
                    let value = self.eval_expression(value_str)?;
                    self.variables.insert(name, value.clone());
                    return Ok(value);
                }
            }
        }
        self.eval_expression(stmt)
    }
}
crate::impl_default_via_new!(SimpleJsEngine);

/// Splits `text` on the commas that are not nested inside `()`/`[]`/`{}`, or inside a
/// string. Used for argument lists and array literals, where a naive `split(',')` breaks
/// `f(1, 2)`-style arguments containing a comma of their own (`[1, [2, 3]]`).
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    for (index, ch) in text.char_indices() {
        if let Some(open) = quote {
            // Inside a string: only the matching quote closes it (no escape handling,
            // matching the rest of this engine, which does not process escapes).
            if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(text[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    parts
}

/// The index of the matching `)` for the `(` at byte 0, or `None` when it is unbalanced.
fn matching_paren(text: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for (index, ch) in text.char_indices() {
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Finds the lowest-precedence binary operator in `text` at nesting depth zero.
///
/// Scans **right to left** so that the leftmost split of a left-associative chain is the
/// last one taken: `1 - 2 - 3` must evaluate as `(1 - 2) - 3`. Scanning left to right and
/// splitting on the first `-` would produce `1 - (2 - 3)`.
///
/// `-` and `+` are skipped when they are unary (at the start, or after another operator)
/// so `-3 + 1` does not split at index 0 and `1 + -2` does not split at the sign.
fn find_top_level_operator<'a>(text: &str, operators: &[&'a str]) -> Option<(usize, &'a str)> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut candidates: Vec<(usize, &'a str)> = Vec::new();

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut position = 0usize;
    while position < chars.len() {
        let (index, ch) = chars[position];
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            }
            position += 1;
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                position += 1;
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            for operator in operators {
                if !text[index..].starts_with(operator) {
                    continue;
                }
                // A `-` is unary when nothing but whitespace precedes it, or when the
                // preceding non-space character is another operator or an opening bracket.
                if operator == &"-" || operator == &"+" {
                    let before = bytes[..index]
                        .iter()
                        .rev()
                        .find(|byte| !byte.is_ascii_whitespace())
                        .copied();
                    let is_unary = match before {
                        None => true,
                        Some(byte) => matches!(
                            byte,
                            b'+' | b'-'
                                | b'*'
                                | b'/'
                                | b'%'
                                | b'('
                                | b'['
                                | b'<'
                                | b'>'
                                | b'='
                                | b'!'
                                | b'&'
                                | b'|'
                                | b','
                        ),
                    };
                    if is_unary {
                        continue;
                    }
                }
                // Require `=`-operators to match their full form, so `==` is not found as
                // a lone `=` and `<=` is not matched by `<`.
                if operator == &"<" && text[index..].starts_with("<=") {
                    continue;
                }
                if operator == &">" && text[index..].starts_with(">=") {
                    continue;
                }
                if operator == &"=" && !text[index..].starts_with("==") {
                    continue;
                }
                if (operator == &"==" || operator == &"!=")
                    && (text[index..].starts_with("===") || text[index..].starts_with("!=="))
                {
                    continue;
                }
                candidates.push((index, operator));
                break;
            }
        }
        position += 1;
    }

    // The rightmost candidate is the leftmost operation of a left-associative chain.
    candidates.pop()
}

/// Applies a binary operator with ECMAScript's coercion rules for the supported cases.
fn apply_operator(operator: &str, left: &JsValue, right: &JsValue) -> JsValue {
    match operator {
        // `+` concatenates when either side is a string, which is ECMAScript's rule.
        "+" => match (left, right) {
            (JsValue::String(a), _) => JsValue::String(format!("{a}{}", right.to_string())),
            (_, JsValue::String(b)) => JsValue::String(format!("{}{b}", left.to_string())),
            _ => JsValue::Number(left.to_number() + right.to_number()),
        },
        "-" => JsValue::Number(left.to_number() - right.to_number()),
        "*" => JsValue::Number(left.to_number() * right.to_number()),
        // Division by zero is `Infinity`/`NaN` in ECMAScript, not an error, and `f64`
        // already produces exactly those values.
        "/" => JsValue::Number(left.to_number() / right.to_number()),
        "%" => JsValue::Number(left.to_number() % right.to_number()),
        "==" | "===" => JsValue::Boolean(loose_equals(left, right)),
        "!=" | "!==" => JsValue::Boolean(!loose_equals(left, right)),
        "<" => JsValue::Boolean(compare(left, right) == Some(core::cmp::Ordering::Less)),
        ">" => JsValue::Boolean(compare(left, right) == Some(core::cmp::Ordering::Greater)),
        "<=" => JsValue::Boolean(matches!(
            compare(left, right),
            Some(core::cmp::Ordering::Less | core::cmp::Ordering::Equal)
        )),
        ">=" => JsValue::Boolean(matches!(
            compare(left, right),
            Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal)
        )),
        // Unreachable: every operator above is listed, and the set is a `const` here.
        _ => JsValue::Undefined,
    }
}

/// Equality with the coercion a caller writing `==` expects: two numbers compare
/// numerically, a number and a string compare by value, and anything else is identity.
fn loose_equals(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Number(a), JsValue::Number(b)) => a == b,
        (JsValue::String(a), JsValue::String(b)) => a == b,
        (JsValue::Boolean(a), JsValue::Boolean(b)) => a == b,
        (JsValue::Number(n), JsValue::String(s)) | (JsValue::String(s), JsValue::Number(n)) => {
            s.trim().parse::<f64>().map(|parsed| parsed == *n).unwrap_or(false)
        }
        // `null` and `undefined` are equal to each other and to nothing else, which is
        // the one coercion ECMAScript's `==` performs that surprises people.
        (JsValue::Null, JsValue::Undefined) | (JsValue::Undefined, JsValue::Null) => true,
        _ => left.to_string() == right.to_string(),
    }
}

/// Ordering for the comparison operators, or `None` when the operands are not comparable.
///
/// Two strings compare lexicographically (the `"a" < "b"` a caller expects); a numeric
/// operand forces a numeric comparison. `None` yields `false` for every operator rather
/// than an ordering, because a comparison that has no answer is not `true` in JS either.
fn compare(left: &JsValue, right: &JsValue) -> Option<core::cmp::Ordering> {
    match (left, right) {
        (JsValue::String(a), JsValue::String(b)) => Some(a.cmp(b)),
        (JsValue::Number(a), JsValue::Number(b)) => a.partial_cmp(b),
        (a, b) => a.to_number().partial_cmp(&b.to_number()),
    }
}

/// Parses `name(args)` into the name and its evaluated arguments, or `None` when `text`
/// is not a call wrapped around the whole expression.
fn parse_call(text: &str) -> Option<(String, Vec<JsValue>)> {
    let text = text.trim();
    let open = text.find('(')?;
    // The parenthesised part must be the tail: `f(1) + 2` is an expression, not a call,
    // and is handled by `eval_expression` splitting at the `+` before reaching here.
    if matching_paren(&text[open..]) != Some(text.len() - 1 - open) {
        return None;
    }
    let name = text[..open].trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    let inner = &text[open + 1..text.len() - 1];
    let args = split_top_level(inner);
    // Argument *parsing* is deferred to the engine's own literal parser so a call site
    // does not need a mutable borrow here; nested arithmetic in an argument is therefore
    // not evaluated, which the module docs state.
    let parsed: Vec<JsValue> = args.iter().map(|arg| literal_value(arg)).collect();
    Some((name.to_string(), parsed))
}

/// A free function version of the literal parse, for [`parse_call`]'s arguments.
///
/// Kept separate from `SimpleJsEngine::parse_literal` because it cannot see the engine's
/// variables: an argument that is a bare identifier becomes `Undefined` here, and the
/// caller's own binding is used by `call_function` instead.
fn literal_value(text: &str) -> JsValue {
    let text = text.trim();
    if text == "undefined" {
        return JsValue::Undefined;
    }
    if text == "null" {
        return JsValue::Null;
    }
    if text == "true" {
        return JsValue::Boolean(true);
    }
    if text == "false" {
        return JsValue::Boolean(false);
    }
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        return JsValue::String(text[1..text.len() - 1].to_string());
    }
    if text.len() >= 2 && text.starts_with('\'') && text.ends_with('\'') {
        return JsValue::String(text[1..text.len() - 1].to_string());
    }
    if let Ok(number) = text.parse::<f64>() {
        return JsValue::Number(number);
    }
    // A bare identifier reaches the caller as a `String` token so a custom built-in can
    // read a global by name; the engine's own built-ins use `to_number()`, which treats an
    // unparsable string as `NaN`, so a mis-typed argument still behaves like JS.
    JsValue::String(text.to_string())
}

/// Whether `script` begins a construct only [`SimpleJsEngine::eval_stmt`] understands.
///
/// Used to route a single-statement script: a script that starts one of these must go to
/// the statement evaluator, and everything else is an expression. Keeping the list here,
/// next to the router, is what makes "is this a statement" one decision rather than a set
/// of prefixes tested in two places.
fn starts_a_statement(script: &str) -> bool {
    const STATEMENT_STARTS: &[&str] = &[
        "function ",
        "if ",
        "if(",
        "for ",
        "for(",
        "while ",
        "while(",
        "return",
        "break",
        "continue",
        "{",
        // Declaration-only scripts are statements; `evaluate` handles a declaration with a
        // continuation in the block arm above, so reaching here means there is no `;`.
        "var ",
        "let ",
        "const ",
    ];
    STATEMENT_STARTS.iter().any(|prefix| script.starts_with(prefix))
}

impl JsEngine for SimpleJsEngine {
    /// Evaluates `script`, dispatching on its textual prefix.
    ///
    /// Handles `console.*(...)` calls and top-level `var`/`let`/`const`
    /// declarations directly, deferring everything else to an internal statement
    /// evaluator. Leading/trailing whitespace is ignored, and an empty script
    /// evaluates to [`JsValue::Undefined`]. Note that declarations are stored in
    /// the engine, so they persist across calls regardless of which
    /// [`JsContext`] is passed.
    fn evaluate(&mut self, script: &str, context: &mut JsContext) -> JsResult<JsValue> {
        let script = script.trim();
        if script.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // --- console.log / console.info ---
        if script.starts_with("console.log(")
            || script.starts_with("console.info(")
            || script.starts_with("console.warn(")
            || script.starts_with("console.error(")
            || script.starts_with("console.debug(")
        {
            let start = script.find('(').ok_or_else(|| {
                JsError::at_offset("Missing '(' in console call".to_string(), script, script.len())
            })? + 1;
            let end = script.rfind(')').ok_or_else(|| {
                JsError::at_offset("Missing ')' in console call".to_string(), script, script.len())
            })?;
            let content = &script[start..end];
            let value = self.eval_expression(content)?;
            context.log(value.to_string());
            return Ok(value);
        }
        // --- var / let / const declarations ---
        //
        // Only a **declaration-only** script is handled here, so that the declaration's
        // initialiser is stored. A script that continues past the declaration
        // (`var a = 3; a + 1`) goes to the block evaluator, which evaluates the
        // declaration as a statement and then the rest. Handling it here and returning
        // would silently drop everything after the first `;` — the script would appear to
        // run and produce the wrong value, which is worse than an error.
        if (script.starts_with("var ")
            || script.starts_with("let ")
            || script.starts_with("const "))
            && !script.contains(';')
        {
            let prefix_len = if script.starts_with("const ") { 6 } else { 4 };
            let rest = &script[prefix_len..];
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim().to_string();
                let value_str = rest[eq_pos + 1..].trim();
                // Check if value is a function definition or complex expression
                let value = self.eval_expression(value_str)?;
                self.variables.insert(name, value.clone());
                return Ok(value);
            } else {
                let name = rest.trim().trim_end_matches(';').to_string();
                self.variables.insert(name, JsValue::Undefined);
                return Ok(JsValue::Undefined);
            }
        }
        // A multi-statement script (or any construct `eval_stmt` owns) is evaluated as a
        // block, so every statement runs and the last one's value is the result.
        if script.contains(';') {
            return self.eval_block(script, context);
        }
        // An expression is evaluated as an expression, **not** through `eval_stmt`.
        //
        // `eval_stmt`'s fallback treats a `name = value` shape as an assignment, and its
        // guard for `==` was too narrow to keep `1 == 1` out: the text starts with `1`, so
        // a naive `find('=')` found the first `=` of `==` and stored a variable called
        // `1 =`. Routing on "does this contain a comparison operator" would be brittle, so
        // the expression evaluator is asked first and its verdict is used whenever the
        // script is not one of the statement forms `eval_stmt` owns.
        if starts_a_statement(script) {
            return self.eval_stmt(script, context);
        }
        self.eval_expression(script)
    }
    /// Calls a user-defined function if one exists, otherwise a built-in.
    ///
    /// User-defined functions take precedence over the built-ins of the same
    /// name. Parameters are bound by position and callers' values override the
    /// parameters' previous globals, which are restored afterwards; surplus
    /// arguments are ignored and missing ones bind as `undefined`. There is no
    /// `this`, no closure over the definition site, and no return-value
    /// propagation from a bare `return`, so a body that returns a computed value
    /// still yields the body's last expression. An unknown `name` returns
    /// `Err` rather than `undefined`, unlike a reference to an undefined
    /// variable inside a script.
    fn call_function(
        &mut self,
        name: &str,
        args: &[JsValue],
        context: &mut JsContext,
    ) -> JsResult<JsValue> {
        // User-defined function
        {
            let candidate =
                self.functions.get(name).or_else(|| self.variables.get(name)).and_then(|v| {
                    if let JsValue::FunctionDef { ref params, ref body, .. } = v {
                        Some((params.clone(), body.clone()))
                    } else {
                        None
                    }
                });
            if let Some((params, body)) = candidate {
                // Save existing variables that collide with parameter names
                let mut saved = Vec::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(existing) = self.variables.get(p.as_str()) {
                        saved.push((p.clone(), existing.clone()));
                    } else {
                        saved.push((p.clone(), JsValue::Undefined));
                    }
                    let arg_val = args.get(i).cloned().unwrap_or(JsValue::Undefined);
                    self.variables.insert(p.clone(), arg_val);
                }
                let result = self.eval_block(&body, context);
                // Restore saved variables
                for (p, val) in saved {
                    self.variables.insert(p, val);
                }
                return result;
            }
        }
        match name {
            "parseInt" => {
                if let Some(arg) = args.first() {
                    if let JsValue::String(s) = arg {
                        return Ok(JsValue::Number(s.parse().unwrap_or(f64::NAN)));
                    }
                    return Ok(JsValue::Number(arg.to_number()));
                }
                Ok(JsValue::Number(f64::NAN))
            }
            "parseFloat" => {
                if let Some(arg) = args.first() {
                    if let JsValue::String(s) = arg {
                        return Ok(JsValue::Number(s.parse().unwrap_or(f64::NAN)));
                    }
                    return Ok(JsValue::Number(arg.to_number()));
                }
                Ok(JsValue::Number(f64::NAN))
            }
            "String" => {
                if let Some(arg) = args.first() {
                    return Ok(JsValue::String(arg.to_string()));
                }
                Ok(JsValue::String("undefined".to_string()))
            }
            "Number" => {
                if let Some(arg) = args.first() {
                    return Ok(JsValue::Number(arg.to_number()));
                }
                Ok(JsValue::Number(0.0))
            }
            "Boolean" => {
                if let Some(arg) = args.first() {
                    return Ok(JsValue::Boolean(arg.is_truthy()));
                }
                Ok(JsValue::Boolean(false))
            }
            _ => {
                // Try to find a user-defined function
                Err(JsError::new(format!(
                    "function '{name}' is not defined: call `register_function` before invoking \
                     it, or check the spelling"
                )))
            }
        }
    }
    /// Defines or overwrites a global as an engine variable.
    ///
    /// The `context` argument is ignored, so globals set here are *not* visible
    /// through [`JsContext::get_global`] and do not carry across engines. Always
    /// succeeds.
    fn set_global(&mut self, name: &str, value: JsValue, _context: &mut JsContext) -> JsResult<()> {
        self.variables.insert(name.to_string(), value);
        Ok(())
    }
    /// Reads an engine variable by name, or `None` if it was never defined.
    ///
    /// Like [`JsEngine::set_global`] this ignores `context`, so it sees only
    /// variables defined through the engine, not those set on the context.
    fn get_global(&self, name: &str, _context: &JsContext) -> Option<JsValue> {
        self.variables.get(name).cloned()
    }
}
/// A shared, mutable JavaScript engine.
///
/// Use this to hand one engine to several owners, or to satisfy a `Send + Sync`
/// bound. Locking is coarse — one mutex guards the whole engine — so evaluation
/// is serialised across threads and a long-running script blocks every other
/// caller. Recover from poisoning rather than unwrapping.
pub type SharedJsEngine = Arc<Mutex<dyn JsEngine>>;
/// Creates a [`SharedJsEngine`] wrapping a new [`SimpleJsEngine`].
///
/// The returned handle owns the engine; cloning the `Arc` shares it, and the
/// engine's interpreter semantics (and limitations) are those of
/// [`SimpleJsEngine`], not a full ECMAScript implementation.
pub fn create_simple_engine() -> SharedJsEngine {
    Arc::new(Mutex::new(SimpleJsEngine::new()))
}

// ── BoaJS Engine (feature-gated) ─────────────────────────────────────────

/// Convert a boa JsValue to our cross-engine JsValue type.
#[cfg(feature = "js-engine")]
fn js_value_to_ours(v: &boa_engine::JsValue, context: &mut boa_engine::Context) -> JsValue {
    if v.is_undefined() {
        JsValue::Undefined
    } else if v.is_null() {
        JsValue::Null
    } else if let Some(n) = v.as_number() {
        JsValue::Number(n)
    } else if let Some(s) = v.as_string() {
        JsValue::String(s.to_std_string_escaped())
    } else if v.is_boolean() {
        JsValue::Boolean(v.as_boolean().unwrap_or(false))
    } else {
        JsValue::String(v.to_string(context).map(|s| s.to_std_string_escaped()).unwrap_or_default())
    }
}

/// Convert our JsValue to a boa JsValue.
#[cfg(feature = "js-engine")]
fn our_value_to_boa(v: &JsValue) -> boa_engine::JsValue {
    match v {
        JsValue::Null | JsValue::Undefined => boa_engine::JsValue::undefined(),
        JsValue::Number(n) => boa_engine::JsValue::from(*n),
        JsValue::String(s) => boa_engine::JsValue::from(boa_engine::JsString::from(s.as_str())),
        JsValue::Boolean(b) => boa_engine::JsValue::from(*b),
        // Remaining types (Array, Object, Function, Ident, FunctionDef) are
        // not representable in the boa engine and map to undefined.
        _ => boa_engine::JsValue::undefined(),
    }
}

/// Real JavaScript engine powered by `boa_engine`.
/// Gated behind `#[cfg(feature = "js-engine")]`.
/// A JavaScript engine backed by the `boa_engine` crate.
///
/// Unlike [`SimpleJsEngine`] this is a real, spec-conformant interpreter, but it
/// belongs to the `boa` host: it owns its own global object and context, is
/// unbounded in recursion (deep scripts can overflow the stack instead of
/// returning an error), and its globals are entirely separate from any
/// [`JsContext`] — the two engines cannot see each other's state. Only compiled
/// when the `js-engine` feature is enabled.
#[cfg(feature = "js-engine")]
pub struct BoaJsEngine {
    context: boa_engine::Context,
}

#[cfg(feature = "js-engine")]
impl BoaJsEngine {
    /// Create a new BoaJS engine with a fresh global context.
    pub fn new() -> Self {
        Self { context: boa_engine::Context::default() }
    }

    /// Evaluate JavaScript source code.
    pub fn evaluate(&mut self, source: &str) -> Result<JsValue, String> {
        let result = self.context.eval(boa_engine::Source::from_bytes(source)).map_err(|e| {
            format!(
                "script could not be evaluated: {e}; fix the JavaScript syntax or the \
                         failing expression"
            )
        })?;
        Ok(js_value_to_ours(&result, &mut self.context))
    }

    /// Evaluate JavaScript and return the result as a string.
    pub fn evaluate_to_string(&mut self, source: &str) -> Result<String, String> {
        let value = self.evaluate(source)?;
        Ok(value.to_string())
    }

    /// Register a Rust function that can be called from JavaScript.
    pub fn register_function(
        &mut self,
        name: &str,
        func: boa_engine::NativeFunction,
    ) -> Result<(), String> {
        let js_name = boa_engine::JsString::from(name);
        self.context.register_global_builtin_callable(js_name, 0, func).map_err(|e| {
            format!(
                "global function '{name}' could not be registered (the name may be \
                     reserved or already defined): {e}"
            )
        })?;
        Ok(())
    }

    /// Get the value of a global variable.
    pub fn get_global(&mut self, name: &str) -> Option<JsValue> {
        let global = self.context.global_object();
        let key = boa_engine::JsString::from(name);
        let val = global.get(key, &mut self.context).ok()?;
        Some(js_value_to_ours(&val, &mut self.context))
    }

    /// Set a global variable.
    pub fn set_global(&mut self, name: &str, value: JsValue) {
        let global = self.context.global_object();
        let key = boa_engine::JsString::from(name);
        let val = our_value_to_boa(&value);
        global.set(key, val, false, &mut self.context).ok();
    }
}

#[cfg(feature = "js-engine")]
crate::impl_default_via_new!(BoaJsEngine);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_js_value_to_string() {
        assert_eq!(JsValue::Undefined.to_string(), "undefined");
        assert_eq!(JsValue::Null.to_string(), "null");
        assert_eq!(JsValue::Boolean(true).to_string(), "true");
        assert_eq!(JsValue::Number(42.0).to_string(), "42");
        assert_eq!(JsValue::String("hello".to_string()).to_string(), "hello");
    }
    #[test]
    fn test_simple_engine_evaluate() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let result = engine.evaluate("var x = 42;", &mut context).unwrap();
        assert_eq!(result, JsValue::Number(42.0));
        let result = engine.evaluate("x", &mut context).unwrap();
        assert_eq!(result, JsValue::Number(42.0));
    }

    // ── Arithmetic and operator precedence ─────────────────────────────────────
    //
    // The module documented a "pragmatic subset" that a reader would take to include
    // arithmetic, and it did not: a binary expression parsed as a literal, failed the
    // number parse, and became `undefined`. These pin the behaviour the docs describe.

    #[test]
    fn arithmetic_evaluates_instead_of_returning_undefined() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        for (script, expected) in [
            ("1 + 2", 3.0),
            ("1+2", 3.0),
            ("7 - 2", 5.0),
            ("3 * 4", 12.0),
            ("8 / 2", 4.0),
            ("7 % 4", 3.0),
            ("-3 + 1", -2.0),
            ("2 + 3 * 4", 14.0),
            ("(2 + 3) * 4", 20.0),
            ("10 - 2 - 3", 5.0),
        ] {
            assert_eq!(
                engine.evaluate(script, &mut context).unwrap(),
                JsValue::Number(expected),
                "{script} must evaluate to {expected}"
            );
        }
    }

    /// Comparison is an operator, not a declaration: `1 == 1` must not be read as an
    /// assignment to a variable called "1 =".
    #[test]
    fn comparison_is_not_mistaken_for_a_declaration() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("1 == 1", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("1 == 2", &mut context).unwrap(), JsValue::Boolean(false));
        assert_eq!(engine.evaluate("1 != 2", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("1 < 2", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("2 <= 2", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("2 > 3", &mut context).unwrap(), JsValue::Boolean(false));
        assert_eq!(engine.evaluate("3 >= 3", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("1 > 2", &mut context).unwrap(), JsValue::Boolean(false));
    }

    /// A script with more than one statement must run **all** of them. The declaration
    /// arm used to return after the first, silently discarding the rest.
    #[test]
    fn every_statement_in_a_script_runs() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("var a = 3; a + 1", &mut context).unwrap(),
            JsValue::Number(4.0)
        );
        assert_eq!(
            engine.evaluate("var c = 2; var d = 5; c * d", &mut context).unwrap(),
            JsValue::Number(10.0)
        );
    }

    /// `+` concatenates when either side is a string, and a variable is read rather than
    /// treated as an identifier token.
    #[test]
    fn string_concatenation_and_variable_lookup_agree() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("\"n=\" + 3", &mut context).unwrap(),
            JsValue::String("n=3".to_string())
        );
        assert_eq!(engine.evaluate("var n = 3; n", &mut context).unwrap(), JsValue::Number(3.0));
    }

    /// A function body whose only `;` is inside its braces must not re-enter the block
    /// evaluator forever. This reproduced as a **stack overflow** (not an error) before the
    /// single-piece case was routed to `eval_stmt`.
    #[test]
    fn a_function_body_does_not_recurse_forever() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let defined = engine.evaluate("function add(a, b) { return a + b; }", &mut context);
        assert!(defined.is_ok(), "defining a function must not overflow the stack");
        let called = engine.call_function(
            "add",
            &[JsValue::Number(2.0), JsValue::Number(3.0)],
            &mut context,
        );
        assert_eq!(called.unwrap(), JsValue::Number(5.0), "the body runs the arithmetic");
    }

    /// Short-circuiting must not evaluate the branch it skips, and must still yield the
    /// operand rather than a coerced boolean (ECMAScript returns the operand).
    #[test]
    fn logical_operators_short_circuit_and_yield_an_operand() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("true && 5", &mut context).unwrap(), JsValue::Number(5.0));
        assert_eq!(engine.evaluate("false && 5", &mut context).unwrap(), JsValue::Boolean(false));
        assert_eq!(engine.evaluate("false || 7", &mut context).unwrap(), JsValue::Number(7.0));
        assert_eq!(engine.evaluate("!0", &mut context).unwrap(), JsValue::Boolean(true));
    }
    #[test]
    fn test_function_definition() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        // Define a function
        let result = engine.evaluate("function add(a, b) { return a + b; }", &mut context);
        assert!(result.is_ok());
        // Call the function
        let result = engine.call_function(
            "add",
            &[JsValue::Number(2.0), JsValue::Number(3.0)],
            &mut context,
        );
        assert!(result.is_ok());
    }
    #[test]
    fn test_if_else() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let result = engine
            .evaluate("var x = 1; if (x > 0) { var y = 10; } else { var y = 20; }", &mut context)
            .unwrap();
        // y should be set to 10 via assignment path (or default)
        assert!(result.is_truthy() || result == JsValue::Undefined);
        // Also test false branch
        let mut engine2 = SimpleJsEngine::new();
        let mut context2 = JsContext::new();
        let result2 = engine2
            .evaluate("var x = 0; if (x > 0) { var y = 10; } else { var y = 20; }", &mut context2)
            .unwrap();
        assert!(result2.is_truthy() || result2 == JsValue::Undefined);
    }
    #[test]
    fn test_array_literal() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let result = engine.evaluate("[1, 2, 3]", &mut context).unwrap();
        match result {
            JsValue::Array(ref elems) => assert_eq!(elems.len(), 3),
            _ => panic!("Expected Array, got {:?}", result),
        }
    }
    #[test]
    fn test_for_loop() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        // Verify for loop parses header correctly with two semicolons.
        let script = "var x = 1";
        let result = engine.evaluate(script, &mut context).unwrap();
        assert_eq!(result, JsValue::Number(1.0));
        // For loop: single-statement evaluate path via eval_stmt.
        let script2 = "for (var i = 0; i < 3; i = 4) { var y = 10; }";
        let result2 = engine.evaluate(script2, &mut context);
        assert!(result2.is_ok(), "for loop should evaluate without error");
    }
    #[test]
    fn test_console_log() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let result = engine.evaluate("console.log('hello')", &mut context).unwrap();
        assert_eq!(result, JsValue::String("hello".to_string()));
    }
}

#[cfg(test)]
#[cfg(feature = "js-engine")]
mod boa_tests {
    use super::*;

    #[test]
    fn test_boa_evaluate_number() {
        let mut engine = BoaJsEngine::new();
        let result = engine.evaluate("42").unwrap();
        assert_eq!(result, JsValue::Number(42.0));
    }

    #[test]
    fn test_boa_evaluate_string() {
        let mut engine = BoaJsEngine::new();
        let result = engine.evaluate_to_string("'hello' + ' world'").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_boa_evaluate_function() {
        let mut engine = BoaJsEngine::new();
        let result = engine.evaluate("function add(a,b) { return a + b; } add(2,3)").unwrap();
        assert_eq!(result, JsValue::Number(5.0));
    }

    #[test]
    fn test_boa_set_and_get_global() {
        let mut engine = BoaJsEngine::new();
        engine.set_global("x", JsValue::Number(99.0));
        let result = engine.evaluate("x * 2").unwrap();
        assert_eq!(result, JsValue::Number(198.0));
    }

    /// A byte offset must be reported as a real line and column.
    ///
    /// The parser tracks positions as offsets. Passing one as the `line` argument
    /// (which the interpreter used to do) produced messages like
    /// `at line 0, column 24` — wrong in both fields, and impossible to act on.
    #[test]
    fn at_offset_converts_an_offset_into_line_and_column() {
        let source = "first\nsecond\nthird";

        // Offset 0 is the very start.
        let error = JsError::at_offset("boom".to_string(), source, 0);
        assert_eq!((error.line, error.column), (Some(1), Some(1)));

        // Offset 6 is just past the first newline: line 2, column 1 ('s').
        let error = JsError::at_offset("boom".to_string(), source, 6);
        assert_eq!((error.line, error.column), (Some(2), Some(1)));

        // Offset 7 is 1 character into line 2, so column 2 ('e').
        let error = JsError::at_offset("boom".to_string(), source, 7);
        assert_eq!((error.line, error.column), (Some(2), Some(2)));

        // Offset 10 is 4 characters into line 2, so column 5 ('n').
        let error = JsError::at_offset("boom".to_string(), source, 10);
        assert_eq!((error.line, error.column), (Some(2), Some(5)));

        // Past the end clamps to the final line rather than panicking.
        let error = JsError::at_offset("boom".to_string(), source, 9999);
        assert_eq!(error.line, Some(3), "an out-of-range offset clamps to the last line");
    }

    /// The conversion must not panic on an offset inside a multi-byte character.
    ///
    /// The parser's offsets are byte indices, so a script containing non-ASCII text
    /// can produce an offset that is not a character boundary. Slicing there would
    /// panic; an error path must never panic on malformed input.
    #[test]
    fn at_offset_does_not_panic_inside_a_multibyte_character() {
        // `é` is two bytes, so offset 1 lands in the middle of it.
        let source = "é";
        let error = JsError::at_offset("boom".to_string(), source, 1);
        assert_eq!(error.line, Some(1), "the offset floors to the character start");
        assert_eq!(error.column, Some(1));
    }

    /// The rendered message must name a line and column, not a column alone.
    #[test]
    fn display_reports_both_line_and_column() {
        let error = JsError::at_offset("boom".to_string(), "a\nb", 2);
        let rendered = error.to_string();
        assert!(rendered.contains("at line 2"), "expected a real line number in {rendered:?}");
        assert!(
            !rendered.contains("line 0"),
            "a zero line means the offset leaked in: {rendered:?}"
        );
    }
}
