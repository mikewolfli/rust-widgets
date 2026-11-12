// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
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
        /// The function body as raw source text.
        ///
        /// The body is *not* re-parsed on every call: `eval_block` caches the statement
        /// split in `block_cache`, keyed by this text, so a tick handler pays one map
        /// lookup instead of a re-split. What remains per call is the copy of this
        /// string (and of `params`) needed to release the immutable borrow before the
        /// body runs against `&mut self`. Storing `Arc<str>` here would remove that copy
        /// but changes this public variant's field type, so the copy is the documented
        /// residual rather than a hidden re-parse.
        body: String,
    },
}
impl JsValue {
    /// Whether this value counts as `true` in a JavaScript boolean context.
    ///
    /// Follows ECMAScript `ToBoolean`: `undefined`/`null` are false, `NaN` and
    /// both zeros are false, empty strings are false, and every object —
    /// including an empty array and an empty object — is true, as are functions.
    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Boolean(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            // Arrays and objects are always truthy regardless of how many
            // elements they hold: an empty collection is a non-null reference.
            JsValue::Array(_) | JsValue::Object(_) => true,
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
/// keep separate) globals: an engine imports the context's globals before running
/// a script and writes them back afterwards. It is a plain value type with no
/// interior mutability.
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
    /// Iterates every global defined in this context.
    ///
    /// An engine imports these before a script runs, so a value written through
    /// [`Self::set_global`] is visible to the next evaluation.
    pub fn globals(&self) -> impl Iterator<Item = (&String, &JsValue)> {
        self.global.iter()
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
/// mutex (see [`SharedJsEngine`]). Globals are exchanged with the caller through
/// the [`JsContext`] passed to every call: `SimpleJsEngine` imports the context's
/// globals before a script runs and writes its own back afterwards, so a host can
/// seed and inspect them.
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

/// Maximum depth of nested user-function calls before the interpreter reports a `JsError`.
///
/// Chosen so a runaway recursion is caught long before the native stack is exhausted, while
/// ordinary scripts (and the tests' small call chains) stay far below it.
const MAX_CALL_DEPTH: usize = 32;

/// A small interpreter that understands a pragmatic *subset* of JavaScript.
///
/// It is not a conforming engine: it supports var/let/const declarations,
/// function definitions and calls with positional parameters, `if`/`else`,
/// `for` loops, array literals, the **arithmetic, comparison and
/// logical operators** (`+ - * / %`, `== != < <= > >=`, `&& || !`, with the usual
/// precedence and left associativity), and the `parseInt`, `parseFloat`, `String`,
/// `Number` and `Boolean` built-ins. It performs no prototype lookups, no closures
/// (function bodies see whatever globals exist at call time), no exceptions/`try`, and
/// no real number formatting. Parsing is textual and recursive, so deeply nested
/// scripts consume stack proportional to nesting depth.
///
/// # What it does not do
///
/// There is no member or index access: `a.b` and `a[0]` are not evaluated, and an array
/// literal can be built but not read back element-wise. A dotted identifier is looked up as
/// one flat global name, which is why a call such as `console.log(1)` resolves only if a global
/// named exactly that exists. An argument that is itself an expression (`f(a + 1)`) is evaluated
/// like any other expression, and a bare identifier argument reads its variable (an identifier
/// that was never defined is `undefined`). There are no escapes inside string literals, and no
/// arrow functions (`=>`).
///
/// Globals are stored in the engine and mirrored into the passed [`JsContext`] after every
/// [`JsEngine::evaluate`] call, so a host can inspect and seed them through the context.
pub struct SimpleJsEngine {
    variables: HashMap<String, JsValue>,
    /// Defined functions (name -> FunctionDef)
    functions: HashMap<String, JsValue>,
    /// Parsed statement lists, keyed by the block text.
    ///
    /// `eval_block` splits a source string on top-level semicolons and allocates one `String`
    /// per statement. Scripts evaluated repeatedly (a frame/tick handler) paid that parse on
    /// every call. The split result is a pure function of the text, so it is cached; the cache
    /// is bounded and cleared wholesale when it overflows, because an interpreter that keeps
    /// every script it has ever seen would grow without limit.
    block_cache: HashMap<String, Arc<Vec<String>>>,
    /// Current depth of user-function calls, bounded by [`MAX_CALL_DEPTH`].
    ///
    /// The interpreter is a tree-walker, so a script such as `function f(n) { return f(n); }`
    /// would otherwise consume native stack until the process aborts — not a `JsError` the host
    /// could handle. The bound follows the `for` loop's runaway policy: report it, never crash.
    call_depth: usize,
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
        Self { variables: HashMap::new(), functions, block_cache: HashMap::new(), call_depth: 0 }
    }
    /// Parses a literal: numbers, strings, booleans, `null`/`undefined`, array literals and
    /// a variable reference. **No operators** — see [`Self::eval_expression`], which is what
    /// evaluates anything containing one.
    fn parse_literal(&mut self, s: &str, context: &mut JsContext) -> JsValue {
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
            let elements: Vec<JsValue> = split_top_level(inner)
                .into_iter()
                .map(|part| self.parse_literal(&part, context))
                .collect();
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
        if let Some((name, raw_args)) = parse_call(s) {
            let mut evaluated = Vec::with_capacity(raw_args.len());
            for arg in &raw_args {
                evaluated.push(self.eval_expression(arg, context).unwrap_or(JsValue::Undefined));
            }
            if let Ok(value) = self.call_function(&name, &evaluated, context) {
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
    fn eval_expression(&mut self, expr: &str, context: &mut JsContext) -> JsResult<JsValue> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // A whole expression in parentheses: strip and recurse, so `(1 + 2) * 3` groups.
        if expr.starts_with('(') && matching_paren(expr) == Some(expr.len() - 1) {
            return self.eval_expression(&expr[1..expr.len() - 1], context);
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
                let left_value = self.eval_expression(left, context)?;
                match op {
                    "&&" => {
                        return if left_value.is_truthy() {
                            self.eval_expression(right, context)
                        } else {
                            Ok(left_value)
                        };
                    }
                    "||" => {
                        return if left_value.is_truthy() {
                            Ok(left_value)
                        } else {
                            self.eval_expression(right, context)
                        };
                    }
                    _ => {}
                }
                let right_value = self.eval_expression(right, context)?;
                return Ok(apply_operator(op, &left_value, &right_value));
            }
        }

        // Unary minus and logical not, which bind tighter than any binary operator above.
        if let Some(rest) = expr.strip_prefix('!') {
            return Ok(JsValue::Boolean(!self.eval_expression(rest, context)?.is_truthy()));
        }
        if let Some(rest) = expr.strip_prefix('-') {
            let value = self.eval_expression(rest, context)?;
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
        if let Some((name, raw_args)) = parse_call(expr) {
            // Arguments are expressions: a quoted literal stays a string, a bare identifier
            // reads its variable, and `f(a + 1)` really computes the sum. Passing the raw
            // text through a literal parser turned `String('hello')` into `"undefined"`.
            let mut evaluated = Vec::with_capacity(raw_args.len());
            for arg in &raw_args {
                evaluated.push(self.eval_expression(arg, context).unwrap_or(JsValue::Undefined));
            }
            return self.call_function(&name, &evaluated, context);
        }
        Ok(self.parse_literal(expr, context))
    }

    /// Evaluates a block (a semicolon-delineated sequence of statements) and
    /// return the value of the last expression (if any).
    fn eval_block(&mut self, block: &str, context: &mut JsContext) -> JsResult<JsValue> {
        let block = block.trim();
        if block.is_empty() {
            return Ok(JsValue::Undefined);
        }
        // Split into top-level semicolons. Brackets and quoted strings are respected, so
        // a `;`, `{` or `}` inside a literal does not split the statement (comments and
        // escapes are still not handled).
        //
        // `char_indices` rather than `block.chars().collect::<Vec<_>>()`: the collected
        // vector was one heap allocation per block (and blocks nest), only to recover byte
        // offsets that `char_indices` already gives.
        //
        // The resulting statement list is cached: re-evaluating the same script (a tick
        // handler, a polling expression) used to re-split and re-allocate every statement.
        let stmts: Arc<Vec<String>> = if let Some(cached) = self.block_cache.get(block) {
            Arc::clone(cached)
        } else {
            let mut parsed = Vec::new();
            let mut depth = 0usize;
            let mut quote: Option<char> = None;
            let mut start = 0usize;
            for (i, ch) in block.char_indices() {
                if let Some(open) = quote {
                    // Inside a string literal: only the matching quote closes it, so every
                    // delimiter up to that point is ordinary text.
                    if ch == open {
                        quote = None;
                    }
                    continue;
                }
                match ch {
                    '"' | '\'' => quote = Some(ch),
                    '{' | '[' | '(' => depth += 1,
                    '}' | ']' | ')' => depth = depth.saturating_sub(1),
                    ';' if depth == 0 => {
                        parsed.push(block[start..i].trim().to_string());
                        start = i + ch.len_utf8();
                    }
                    _ => { /* Other characters are not relevant */ }
                }
            }
            let tail = block[start..].trim().to_string();
            if !tail.is_empty() {
                parsed.push(tail);
            }
            let parsed = Arc::new(parsed);
            if self.block_cache.len() >= 64 {
                self.block_cache.clear();
            }
            self.block_cache.insert(block.to_string(), Arc::clone(&parsed));
            parsed
        };
        let mut last_val = JsValue::Undefined;
        for stmt in stmts.iter() {
            // A block whose only `;` is nested (inside `{…}`) produces exactly one
            // "statement" that is the whole input. Calling `evaluate` on it would re-enter
            // this function on the same text and recurse until the stack ran out — the
            // function-definition case (`function f() { return 1; }`) hit exactly that.
            // When there is only one piece and it is the input itself, the block wrapper
            // added nothing, so `eval_stmt` is the right entry point: it owns the
            // constructs the split could not reach.
            // A single statement goes straight to `eval_stmt`. Comparing against `block`
            // (rather than just the length) missed the ordinary `return a;` body: the
            // statement is trimmed and has lost its `;`, so it never compared equal and the
            // statement re-entered `evaluate`. That nested evaluate exports the whole variable
            // namespace — including a call's **parameters** — into the caller's context, which
            // leaked `a` from `function f(a) { return a; } f(1)` (T3-47).
            last_val = if stmts.len() == 1 {
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
        // --- unsupported control flow ---
        //
        // `starts_a_statement` routes these four prefixes here, but this evaluator has no arm for
        // them. Without this check a `while` body fell through to the assignment fallback, which
        // found the body's first `=` and created a variable named after its left-hand side — the
        // script "ran" and wrote garbage state. An explicit error is the honest answer until
        // control-flow signals are designed.
        if stmt.starts_with("while ")
            || stmt.starts_with("while(")
            || stmt.starts_with("break")
            || stmt.starts_with("continue")
        {
            return Err(JsError::at_offset(
                "while/break/continue are not part of this engine's supported subset".to_string(),
                stmt,
                stmt.len(),
            ));
        }
        // --- var / let / const declaration ---
        //
        // A declaration normally reaches `evaluate`'s dedicated arm, but one whose initialiser
        // contains a `;` (inside a string literal) was routed here instead. Without this arm the
        // fallback below read `var s = ...` as an assignment and created a variable called
        // `var s`, silently losing the declaration.
        if let Some(rest) = stmt
            .strip_prefix("var ")
            .or_else(|| stmt.strip_prefix("let "))
            .or_else(|| stmt.strip_prefix("const "))
        {
            let rest = rest.trim();
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim();
                if name.is_empty() {
                    return Err(JsError::at_offset(
                        "Declaration needs a name".to_string(),
                        stmt,
                        stmt.len(),
                    ));
                }
                let value = self.eval_expression(rest[eq_pos + 1..].trim(), context)?;
                self.variables.insert(name.to_string(), value.clone());
                return Ok(value);
            }
            let name = rest.trim_end_matches(';').trim();
            if name.is_empty() {
                return Err(JsError::at_offset(
                    "Declaration needs a name".to_string(),
                    stmt,
                    stmt.len(),
                ));
            }
            self.variables.insert(name.to_string(), JsValue::Undefined);
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
            let cond_end = matching_paren(&stmt[cond_start..]).ok_or_else(|| {
                JsError::at_offset("Unclosed condition in 'if'".to_string(), stmt, stmt.len())
            })?;
            let condition = stmt[cond_start + 1..cond_start + cond_end].trim();
            let cond_val = self.eval_expression(condition, context)?;
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
            // Matched, not `find(')'`: the first `)` can belong to a nested call, which
            // truncated `if (parseInt("5") > 3)` at `parseInt("5"` and silently took the
            // else branch.
            let paren_end = matching_paren(&stmt[paren_start..]).ok_or_else(|| {
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
            let mut completed = false;
            // `0..=10000`: the range's last iteration is the condition test that ends an
            // exactly-10000-iteration loop. `0..10000` ran the body 10000 times but never
            // re-tested, so a loop that finishes on its 10000th pass was reported as runaway.
            for _ in 0..=10000 {
                let cond_val = self.eval_expression(&loop_cond, context)?;
                if !cond_val.is_truthy() {
                    completed = true;
                    break;
                }
                last_val = self.eval_block(after_header, context)?;
                if let Some(incr) = &loop_incr {
                    self.evaluate(incr, context)?;
                }
            }
            if !completed {
                // Silent truncation used to let a runaway script "finish" with a wrong
                // result. The caller has to see that the loop was cut off.
                return Err(JsError::at_offset(
                    "for loop exceeded 10000 iterations".to_string(),
                    stmt,
                    stmt.len(),
                ));
            }
            return Ok(last_val);
        }
        // --- return ---
        if stmt.starts_with("return ") || stmt == "return" {
            let val = if stmt.len() > 7 {
                // Evaluate the returned expression, not the whole statement pipeline: `evaluate`
                // synchronises every engine variable into the caller's context afterwards, and
                // during a call that namespace still holds the **parameters**. That leaked
                // `a` as a global from `function f(a) { return a; } f(1)` the moment the real
                // context was threaded through instead of a throwaway one (T3-47).
                self.eval_expression(stmt[7..].trim(), context)?
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
                    let value = self.eval_expression(value_str, context)?;
                    self.variables.insert(name, value.clone());
                    return Ok(value);
                }
            }
        }
        self.eval_expression(stmt, context)
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
                // Only the start of an operator token counts. `===` also matched a trailing
                // `==` one byte later, and the rightmost-candidate rule then picked that inner
                // one, so `1 === 1` evaluated as `==` over a stray `=` operand.
                if (operator == &"==" || operator == &"!=")
                    && index > 0
                    && matches!(bytes[index - 1], b'=' | b'!')
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
        "==" => JsValue::Boolean(loose_equals(left, right)),
        "!=" => JsValue::Boolean(!loose_equals(left, right)),
        "===" => JsValue::Boolean(strict_equals(left, right)),
        "!==" => JsValue::Boolean(!strict_equals(left, right)),
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

/// Equality with no coercion (`===`): both operands must have the same variant and the
/// same value.
///
/// The parser deliberately keeps `===`/`!==` distinct from `==`/`!=`, so throwing that
/// distinction away here made `1 === '1'` and `null === undefined` true.
fn strict_equals(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Number(a), JsValue::Number(b)) => a == b,
        (JsValue::String(a), JsValue::String(b)) => a == b,
        (JsValue::Boolean(a), JsValue::Boolean(b)) => a == b,
        (JsValue::Null, JsValue::Null) | (JsValue::Undefined, JsValue::Undefined) => true,
        // Arrays and functions are reference values; this engine has no identity tracking,
        // so two distinct literals are never strictly equal (matching JS).
        _ => false,
    }
}

/// Parses the integer prefix of `text`, the way ECMAScript's `parseInt` does.
///
/// Digits are read until the first non-digit (`"12px"` is `12`), a leading sign is kept, and
/// text with no digit at all is `NaN`. The plain `str::parse::<f64>` used before required the
/// whole string to be numeric and returned a fraction for `parseInt(3.9)`.
fn parse_int_text(text: &str) -> f64 {
    let text = text.trim();
    let mut end = 0usize;
    for (index, ch) in text.char_indices() {
        let sign = index == 0 && (ch == '+' || ch == '-');
        if !sign && !ch.is_ascii_digit() {
            break;
        }
        end = index + ch.len_utf8();
    }
    let digits = &text[..end];
    if digits.is_empty() || digits == "+" || digits == "-" {
        return f64::NAN;
    }
    digits.parse().unwrap_or(f64::NAN)
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

/// Parses `name(args)` into the name and its raw argument texts, or `None` when `text`
/// is not a call wrapped around the whole expression.
///
/// The arguments stay textual because only the caller can evaluate them: this free
/// function cannot see the engine's variables, and a quoted literal's content must not be
/// mistaken for an identifier.
fn parse_call(text: &str) -> Option<(String, Vec<String>)> {
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
    Some((name.to_string(), args))
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

/// Whether `script` is a top-level `name = value` assignment.
///
/// Assignments are statements, not expressions, so the router below must send them to
/// `eval_stmt`. Without this, `i = 4` in a `for` increment was evaluated as a literal and
/// never updated the variable, which made every such loop run to its iteration cap; the old
/// silent truncation hid the defect behind a wrong (but `Ok`) result.
fn is_assignment_expression(script: &str) -> bool {
    let bytes = script.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    for (index, &byte) in bytes.iter().enumerate() {
        if let Some(open) = quote {
            if byte == open {
                quote = None;
            }
            continue;
        }
        match byte {
            b'"' | b'\'' => quote = Some(byte),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                let before = index.checked_sub(1).map(|i| bytes[i]);
                let after = bytes.get(index + 1).copied();
                if after == Some(b'=') {
                    continue;
                }
                if matches!(before, Some(b'<' | b'>' | b'!' | b'=')) {
                    continue;
                }
                let name = script[..index].trim();
                return !name.is_empty()
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
            }
            _ => {}
        }
    }
    false
}

impl JsEngine for SimpleJsEngine {
    /// Evaluates `script`, dispatching on its textual prefix.
    ///
    /// Handles `console.*(...)` calls and top-level `var`/`let`/`const`
    /// declarations directly, deferring everything else to an internal statement
    /// evaluator. Leading/trailing whitespace is ignored, and an empty script
    /// evaluates to [`JsValue::Undefined`]. The context's globals are imported before
    /// the script runs and the engine's are written back afterwards, so declarations
    /// persist both in the engine and in the context it was handed.
    fn evaluate(&mut self, script: &str, context: &mut JsContext) -> JsResult<JsValue> {
        // The context is the shared namespace: import its globals so a value a host wrote
        // through `JsContext::set_global` (or another engine sharing this context) is visible.
        for (name, value) in context.globals() {
            self.variables.insert(name.clone(), value.clone());
        }
        // The body below is a closure so every early returns still funnels through the export; a
        // copied export at each `return` is exactly the kind of drift this avoids.
        let result = (|| -> JsResult<JsValue> {
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
                    JsError::at_offset(
                        "Missing '(' in console call".to_string(),
                        script,
                        script.len(),
                    )
                })? + 1;
                let end = script.rfind(')').ok_or_else(|| {
                    JsError::at_offset(
                        "Missing ')' in console call".to_string(),
                        script,
                        script.len(),
                    )
                })?;
                let content = &script[start..end];
                let value = self.eval_expression(content, context)?;
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
                    let value = self.eval_expression(value_str, context)?;
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
            if starts_a_statement(script) || is_assignment_expression(script) {
                return self.eval_stmt(script, context);
            }
            self.eval_expression(script, context)
        })();
        // Write the namespace back so the context (and any engine sharing it) sees the
        // script's declarations and assignments.
        for (name, value) in &self.variables {
            context.set_global(name, value.clone());
        }
        result
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
            // A user-defined function takes precedence over a built-in of the same name, as
            // the contract says: `variables` holds script-defined functions, `functions` the
            // five built-ins.
            let candidate =
                self.variables.get(name).or_else(|| self.functions.get(name)).and_then(|v| {
                    if let JsValue::FunctionDef { ref params, ref body, .. } = v {
                        Some((params.clone(), body.clone()))
                    } else {
                        None
                    }
                });
            if let Some((params, body)) = candidate {
                if self.call_depth >= MAX_CALL_DEPTH {
                    return Err(JsError::new(format!(
                        "call stack exceeded {MAX_CALL_DEPTH} frames while calling `{name}`"
                    )));
                }
                self.call_depth = self.call_depth.saturating_add(1);
                // Save existing variables that collide with parameter names. `None` means the
                // name was not defined before the call, so restoring must remove it rather than
                // leave `undefined` behind as a global the script never declared.
                let mut saved: Vec<(String, Option<JsValue>)> = Vec::new();
                for (i, p) in params.iter().enumerate() {
                    saved.push((p.clone(), self.variables.get(p.as_str()).cloned()));
                    let arg_val = args.get(i).cloned().unwrap_or(JsValue::Undefined);
                    self.variables.insert(p.clone(), arg_val);
                }
                let result = self.eval_block(&body, context);
                self.call_depth = self.call_depth.saturating_sub(1);
                // Restore saved variables
                for (p, val) in saved {
                    match val {
                        Some(value) => {
                            self.variables.insert(p, value);
                        }
                        None => {
                            self.variables.remove(&p);
                        }
                    }
                }
                return result;
            }
        }
        match name {
            "parseInt" => {
                if let Some(arg) = args.first() {
                    if let JsValue::String(s) = arg {
                        return Ok(JsValue::Number(parse_int_text(s)));
                    }
                    // `parseInt` returns an integer: JS truncates toward zero.
                    return Ok(JsValue::Number(arg.to_number().trunc()));
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
    /// Defines or overwrites a global as an engine variable, and mirrors it into `context`.
    ///
    /// `evaluate` imports the context's globals at the start of every call and writes the
    /// engine's back afterwards, so this value is visible to the next script and to any other
    /// engine sharing the context. Always succeeds.
    fn set_global(&mut self, name: &str, value: JsValue, context: &mut JsContext) -> JsResult<()> {
        self.variables.insert(name.to_string(), value.clone());
        context.set_global(name, value);
        Ok(())
    }
    /// Reads an engine variable by name, or `None` if it was never defined.
    ///
    /// A name that only the context knows is still answered, because the two namespaces are
    /// synchronised around every `evaluate`.
    fn get_global(&self, name: &str, context: &JsContext) -> Option<JsValue> {
        self.variables.get(name).cloned().or_else(|| context.get_global(name).cloned())
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
        JsValue::Null => boa_engine::JsValue::null(),
        JsValue::Undefined => boa_engine::JsValue::undefined(),
        JsValue::Number(n) => boa_engine::JsValue::from(*n),
        JsValue::String(s) => boa_engine::JsValue::from(boa_engine::JsString::from(s.as_str())),
        JsValue::Boolean(b) => boa_engine::JsValue::from(*b),
        // Remaining types (Array, Object, Function, Ident, FunctionDef) are
        // not representable in the boa engine and map to undefined.
        _ => boa_engine::JsValue::undefined(),
    }
}

/// Why writing a global into a [`BoaJsEngine`] failed.
///
/// The engine's `set` operation can fail in two distinct ways: the property may
/// exist but be read-only, or a setter on the property may have thrown. Keeping
/// them apart lets a caller react differently to "the value was rejected" versus
/// "user code ran and failed".
#[cfg(feature = "js-engine")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetGlobalError {
    /// The property exists but is not writable, so the engine rejected the write
    /// without running any setter. Carries the property (global) name.
    PropertyRejected(String),
    /// A setter on the property threw while the write was being applied. Carries
    /// the engine's description of the thrown error.
    SetterThrew(String),
}

#[cfg(feature = "js-engine")]
impl std::fmt::Display for SetGlobalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetGlobalError::PropertyRejected(name) => {
                write!(f, "global '{name}' is read-only and rejected the write")
            }
            SetGlobalError::SetterThrew(message) => {
                write!(f, "global setter threw: {message}")
            }
        }
    }
}

#[cfg(feature = "js-engine")]
impl std::error::Error for SetGlobalError {}

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
    ///
    /// Failures are silently ignored; use [`Self::try_set_global`] when the
    /// caller must learn whether the write actually happened.
    pub fn set_global(&mut self, name: &str, value: JsValue) {
        let _ = self.try_set_global(name, value);
    }

    /// Set a global variable, reporting why the write failed.
    ///
    /// Returns `Ok(())` when the write succeeded. A read-only property yields
    /// [`SetGlobalError::PropertyRejected`], while a setter that throws yields
    /// [`SetGlobalError::SetterThrew`].
    pub fn try_set_global(&mut self, name: &str, value: JsValue) -> Result<(), SetGlobalError> {
        let global = self.context.global_object();
        let key = boa_engine::JsString::from(name);
        let val = our_value_to_boa(&value);
        match global.set(key, val, false, &mut self.context) {
            Ok(true) => Ok(()),
            Ok(false) => Err(SetGlobalError::PropertyRejected(name.to_string())),
            Err(error) => Err(SetGlobalError::SetterThrew(error.to_string())),
        }
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

    /// ECMAScript `ToBoolean` treats every object — including empty arrays and
    /// empty objects — as truthy, unlike empty strings and zero-valued primitives.
    #[test]
    fn empty_collections_are_truthy_like_ecmascript_objects() {
        assert!(JsValue::Array(Vec::new()).is_truthy());
        assert!(JsValue::Array(vec![JsValue::Null]).is_truthy());
        assert!(JsValue::Object(std::collections::HashMap::new()).is_truthy());
        assert!(JsValue::Array(Vec::new()).to_boolean());
        assert!(JsValue::Object(std::collections::HashMap::new()).to_boolean());
        // The falsey cases stay falsey.
        assert!(!JsValue::String(String::new()).is_truthy());
        assert!(!JsValue::Number(0.0).is_truthy());
        assert!(!JsValue::Number(f64::NAN).is_truthy());
        assert!(!JsValue::Null.is_truthy());
        assert!(!JsValue::Undefined.is_truthy());
    }

    #[test]
    fn boolean_builtin_coerces_an_empty_array_to_true() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("Boolean([])", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("Boolean('')", &mut context).unwrap(), JsValue::Boolean(false));
    }

    #[test]
    fn an_empty_array_enters_the_if_branch() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("if ([]) { 1 } else { 2 }", &mut context).unwrap(),
            JsValue::Number(1.0)
        );
        assert_eq!(
            engine.evaluate("if ('') { 1 } else { 2 }", &mut context).unwrap(),
            JsValue::Number(2.0)
        );
    }

    #[test]
    fn an_empty_array_drives_logical_operators_as_truthy() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("[] && 5", &mut context).unwrap(), JsValue::Number(5.0));
        assert_eq!(engine.evaluate("[] || 7", &mut context).unwrap(), JsValue::Array(Vec::new()));
        assert_eq!(engine.evaluate("![]", &mut context).unwrap(), JsValue::Boolean(false));
    }
    /// The router lists `while`/`break`/`continue` as statement forms, so each spelling must
    /// answer an error rather than silently evaluating a loop body as an assignment.
    #[test]
    fn unsupported_control_flow_reports_an_error_instead_of_writing_garbage() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        for script in
            ["while (i < 3) { i = i + 1 }", "while(i < 3) { i = i + 1 }", "break", "continue"]
        {
            let error = engine
                .evaluate(script, &mut context)
                .expect_err("{script} is not in the supported subset and must not evaluate");
            assert!(
                error.message.contains("not part of this engine's supported subset"),
                "{script} must name the unsupported construct, got {:?}",
                error.message
            );
        }
        // The rejected `while` body must not have created a variable named `i`.
        assert!(
            !engine.variables.contains_key("i"),
            "the rejected loop body must not have been stored as an assignment"
        );
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

    /// A semicolon or brace inside a string literal is text, not a statement delimiter.
    ///
    /// The block splitter tracked brackets but not quotes, so `var s = "a;b{c}"` was cut at
    /// the `;` inside the literal (and the `{` even changed the nesting depth), leaving
    /// fragments that evaluated to `undefined` instead of the string.
    #[test]
    fn string_literals_do_not_split_the_statement() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("var s = \"a;b{c}\"; s", &mut context).unwrap(),
            JsValue::String("a;b{c}".to_string())
        );
        assert_eq!(
            engine.get_global("s", &context),
            Some(JsValue::String("a;b{c}".to_string())),
            "the whole literal is the stored value"
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

    /// A single-statement function body is routed to `eval_stmt` rather than re-entering the
    /// block evaluator. This reproduced as a **stack overflow** (not an error) before that
    /// routing existed.
    #[test]
    fn a_single_statement_function_body_is_evaluated() {
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

    /// Runaway user-function recursion is reported as a `JsError`, not left to abort the process
    /// with a native stack overflow (T3-06).
    #[test]
    fn runaway_recursion_is_an_error_not_a_stack_overflow() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        engine
            .evaluate("function down(n) { return down(n); }", &mut context)
            .expect("defining a recursive function must not overflow the stack");
        let error = engine
            .call_function("down", &[JsValue::Number(1.0)], &mut context)
            .expect_err("the call must be refused once the depth bound is reached");
        assert!(
            error.to_string().contains("call stack exceeded"),
            "the error must name the cause: {error}"
        );
        // The counter is restored, so an ordinary call after the failure still works.
        let seven = engine
            .call_function("parseInt", &[JsValue::String("7".to_string())], &mut context)
            .expect("the failed recursion must leave the engine usable");
        assert_eq!(seven, JsValue::Number(7.0));
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

    /// `===` compares without coercion, as the parser's distinct token promises.
    #[test]
    fn strict_equality_does_not_coerce() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("1 === '1'", &mut context).unwrap(), JsValue::Boolean(false));
        assert_eq!(engine.evaluate("1 == '1'", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("1 !== '1'", &mut context).unwrap(), JsValue::Boolean(true));
        assert_eq!(
            engine.evaluate("null === undefined", &mut context).unwrap(),
            JsValue::Boolean(false)
        );
        assert_eq!(engine.evaluate("'a' === 'a'", &mut context).unwrap(), JsValue::Boolean(true));
    }

    /// A nested call in an `if`/`for` header does not truncate the condition at its own `)`.
    #[test]
    fn a_nested_call_does_not_truncate_a_header() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("if (parseInt('5') > 3) { 1 } else { 2 }", &mut context).unwrap(),
            JsValue::Number(1.0),
            "the nested call's parenthesis must not end the condition"
        );
        let result = engine
            .evaluate("for (var i = 0; i < parseInt('3'); i = i + 1) { var y = i; }", &mut context);
        assert!(result.is_ok(), "the for header keeps its nested call: {result:?}");
    }

    /// Call arguments are expressions: quoted text stays text and an identifier reads itself.
    #[test]
    fn call_arguments_are_evaluated_as_expressions() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(
            engine.evaluate("String('hello')", &mut context).unwrap(),
            JsValue::String("hello".to_string())
        );
        engine.evaluate("var n = 5;", &mut context).unwrap();
        assert_eq!(
            engine.evaluate("String(n)", &mut context).unwrap(),
            JsValue::String("5".to_string()),
            "a bare identifier argument reads its variable"
        );
        assert_eq!(
            engine.evaluate("Number(n + 1)", &mut context).unwrap(),
            JsValue::Number(6.0),
            "an argument may be a whole expression"
        );
    }

    /// A loop that finishes on its 10000th pass is not reported as runaway.
    #[test]
    fn a_loop_at_the_iteration_limit_completes() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        let result =
            engine.evaluate("for (var i = 0; i < 10000; i = i + 1) { var y = i; }", &mut context);
        assert!(result.is_ok(), "10000 iterations is within the limit: {result:?}");
    }

    /// `parseInt` truncates and reads the integer prefix, as ECMAScript's does.
    #[test]
    fn parse_int_truncates_and_reads_a_prefix() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        assert_eq!(engine.evaluate("parseInt(3.9)", &mut context).unwrap(), JsValue::Number(3.0));
        assert_eq!(
            engine.evaluate("parseInt('12px')", &mut context).unwrap(),
            JsValue::Number(12.0)
        );
        assert_eq!(engine.evaluate("parseInt('-7')", &mut context).unwrap(), JsValue::Number(-7.0));
    }

    /// A parameter that did not exist before the call is not left behind as a global.
    #[test]
    fn parameters_are_removed_after_the_call() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        engine.evaluate("function f(a) { return a; }", &mut context).unwrap();
        engine.evaluate("f(1)", &mut context).unwrap();
        assert_eq!(engine.get_global("a", &context), None, "`a` was never declared globally");
    }

    /// A script-defined function shadows a built-in of the same name.
    #[test]
    fn a_user_function_shadows_a_builtin() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        engine.evaluate("function parseInt(x) { return 42; }", &mut context).unwrap();
        assert_eq!(engine.evaluate("parseInt(1)", &mut context).unwrap(), JsValue::Number(42.0));
    }

    /// Context globals reach the script, and script globals reach the context.
    #[test]
    fn the_context_is_the_shared_namespace() {
        let mut engine = SimpleJsEngine::new();
        let mut context = JsContext::new();
        context.set_global("x", JsValue::Number(3.0));
        assert_eq!(
            engine.evaluate("x + 1", &mut context).unwrap(),
            JsValue::Number(4.0),
            "a host-seeded context global is readable"
        );
        engine.evaluate("var y = 5;", &mut context).unwrap();
        assert_eq!(
            context.get_global("y"),
            Some(&JsValue::Number(5.0)),
            "the script's declaration is written back"
        );

        let mut second = SimpleJsEngine::new();
        assert_eq!(
            second.evaluate("y + 1", &mut context).unwrap(),
            JsValue::Number(6.0),
            "a second engine sharing the context sees the same globals"
        );
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

    #[test]
    fn test_boa_null_round_trips_distinctly_from_undefined() {
        let mut engine = BoaJsEngine::new();
        engine.set_global("probe_null", JsValue::Null);
        engine.set_global("probe_undef", JsValue::Undefined);
        // The two keep their identities through the bridge.
        assert_eq!(engine.get_global("probe_null"), Some(JsValue::Null));
        assert_eq!(engine.get_global("probe_undef"), Some(JsValue::Undefined));
        // Strict equality must not confuse them.
        assert_eq!(engine.evaluate("probe_null === null").unwrap(), JsValue::Boolean(true));
        assert_eq!(engine.evaluate("probe_undef === null").unwrap(), JsValue::Boolean(false));
        assert_eq!(engine.evaluate("probe_undef === undefined").unwrap(), JsValue::Boolean(true));
    }

    #[test]
    fn test_boa_set_global_reports_read_only_rejection() {
        let mut engine = BoaJsEngine::new();
        engine
            .evaluate("Object.defineProperty(globalThis, 'ro', { value: 1, writable: false });")
            .unwrap();
        // Writing 2 over the read-only property fails with a property rejection.
        let result = engine.try_set_global("ro", JsValue::Number(2.0));
        assert!(matches!(result, Err(SetGlobalError::PropertyRejected(name)) if name == "ro"));
        // The property keeps its original value.
        assert_eq!(engine.get_global("ro"), Some(JsValue::Number(1.0)));
        // The legacy infallible API stays callable and silently no-ops on rejection.
        engine.set_global("ro", JsValue::Number(3.0));
        assert_eq!(engine.get_global("ro"), Some(JsValue::Number(1.0)));
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
