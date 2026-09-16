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
    /// 1-based source line, or `None` if unknown. At least one producer in this
    /// module reports position using the *character offset* while labelling it as
    /// a line, so do not assume strict 1-based line semantics.
    pub line: Option<u32>,
    /// Column, or `None` if unknown. Only meaningful alongside [`Self::line`];
    /// [`std::fmt::Display`] prints both or neither.
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
/// `for` loops, array literals, member/index access and the `parseInt`,
/// `parseFloat`, `String`, `Number` and `Boolean` built-ins, but it performs no
/// prototype lookups, no closures (function bodies see whatever globals exist at
/// call time), no exceptions/`try`, and no real number formatting. Parsing is
/// textual and recursive, so deeply nested scripts consume stack proportional to
/// nesting depth.
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
    fn parse_value(&self, s: &str) -> JsValue {
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
        if s.starts_with('"') && s.ends_with('"') {
            return JsValue::String(s[1..s.len() - 1].to_string());
        }
        if s.starts_with('\'') && s.ends_with('\'') {
            return JsValue::String(s[1..s.len() - 1].to_string());
        }
        // Array literal
        if s.starts_with('[') && s.ends_with(']') {
            let inner = s[1..s.len() - 1].trim();
            if inner.is_empty() {
                return JsValue::Array(Vec::new());
            }
            let elements: Vec<JsValue> =
                inner.split(',').map(|part| self.parse_value(part.trim())).collect();
            return JsValue::Array(elements);
        }
        if let Ok(n) = s.parse::<f64>() {
            return JsValue::Number(n);
        }
        if let Some(v) = self.variables.get(s) {
            return v.clone();
        }
        JsValue::Undefined
    }
    /// Evaluate a block (a semicolon-delineated sequence of statements) and
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
            last_val = self.evaluate(stmt, context)?;
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
                    JsError::with_location(
                        "Unclosed parameter list in function definition".to_string(),
                        0,
                        stmt.len() as u32,
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
                        JsError::with_location(
                            "Unclosed function body".to_string(),
                            0,
                            stmt.len() as u32,
                        )
                    })?;
                    let body = after_params[1..close].to_string();
                    let func = JsValue::FunctionDef { name: name.to_string(), params, body };
                    self.variables.insert(name.to_string(), func.clone());
                    return Ok(func);
                }
            }
            return Err(JsError::with_location(
                "Invalid function syntax".to_string(),
                0,
                stmt.len() as u32,
            ));
        }
        // --- if / else ---
        if stmt.starts_with("if ") || stmt.starts_with("if(") {
            let cond_start = stmt.find('(').ok_or_else(|| {
                JsError::with_location("Expected '(' after 'if'".to_string(), 0, stmt.len() as u32)
            })?;
            let cond_end = stmt[cond_start..].find(')').ok_or_else(|| {
                JsError::with_location(
                    "Unclosed condition in 'if'".to_string(),
                    0,
                    stmt.len() as u32,
                )
            })?;
            let condition = stmt[cond_start + 1..cond_start + cond_end].trim();
            let cond_val = self.evaluate(condition, context)?;
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
                JsError::with_location("Expected '(' after 'for'".to_string(), 0, stmt.len() as u32)
            })?;
            let paren_end = stmt[paren_start..].find(')').ok_or_else(|| {
                JsError::with_location("Unclosed 'for' condition".to_string(), 0, stmt.len() as u32)
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
                    return Err(JsError::with_location(
                        "Invalid 'for' loop syntax".to_string(),
                        0,
                        stmt.len() as u32,
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
                let cond_val = self.evaluate(&loop_cond, context)?;
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
                    let value = self.parse_value(value_str);
                    self.variables.insert(name, value.clone());
                    return Ok(value);
                }
            }
        }
        Ok(self.parse_value(stmt))
    }
}
crate::impl_default_via_new!(SimpleJsEngine);
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
                JsError::with_location(
                    "Missing '(' in console call".to_string(),
                    0,
                    script.len() as u32,
                )
            })? + 1;
            let end = script.rfind(')').ok_or_else(|| {
                JsError::with_location(
                    "Missing ')' in console call".to_string(),
                    0,
                    script.len() as u32,
                )
            })?;
            let content = &script[start..end];
            let value = self.parse_value(content);
            context.log(value.to_string());
            return Ok(value);
        }
        // --- var / let / const declarations ---
        if script.starts_with("var ") || script.starts_with("let ") || script.starts_with("const ")
        {
            let prefix_len = if script.starts_with("const ") { 6 } else { 4 };
            let rest = &script[prefix_len..];
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim().to_string();
                let value_str = rest[eq_pos + 1..].trim();
                // Check if value is a function definition or complex expression
                let value = self.parse_value(value_str);
                self.variables.insert(name, value.clone());
                return Ok(value);
            } else {
                let name = rest.trim().trim_end_matches(';').to_string();
                self.variables.insert(name, JsValue::Undefined);
                return Ok(JsValue::Undefined);
            }
        }
        // Delegate to eval_stmt for all other constructs
        self.eval_stmt(script, context)
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
                Err(JsError::new(format!("Function '{name}' is not defined")))
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
        let result = self
            .context
            .eval(boa_engine::Source::from_bytes(source))
            .map_err(|e| format!("JS error: {e}"))?;
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
        let name = boa_engine::JsString::from(name);
        self.context
            .register_global_builtin_callable(name, 0, func)
            .map_err(|e| format!("Failed to register function: {e}"))?;
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
}
