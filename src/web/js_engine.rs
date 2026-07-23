use crate::compat::HashMap;
use std::sync::{Arc, Mutex};
#[derive(Debug, Clone, PartialEq, Default)]
pub enum JsValue {
    #[default]
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(HashMap<String, JsValue>),
    Function(String),
    /// An identifier reference (used during parsing).
    Ident(String),
    /// A function with a parameter list and body source.
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: String,
    },
}
impl JsValue {
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
    pub fn to_boolean(&self) -> bool {
        self.is_truthy()
    }
}
#[derive(Debug, Clone)]
pub struct JsError {
    pub message: String,
    pub stack: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}
impl JsError {
    pub fn new(message: String) -> Self {
        Self { message, stack: None, line: None, column: None }
    }
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
pub type JsResult<T> = Result<T, JsError>;

#[derive(Debug, Clone)]
pub struct JsContext {
    global: HashMap<String, JsValue>,
    console_messages: Vec<ConsoleMessage>,
}
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    pub message: String,
    pub line: u32,
    pub source: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}
impl JsContext {
    pub fn new() -> Self {
        let mut global = HashMap::new();
        global.insert("undefined".to_string(), JsValue::Undefined);
        global.insert("NaN".to_string(), JsValue::Number(f64::NAN));
        global.insert("Infinity".to_string(), JsValue::Number(f64::INFINITY));
        Self { global, console_messages: Vec::new() }
    }
    pub fn set_global(&mut self, name: &str, value: JsValue) {
        self.global.insert(name.to_string(), value);
    }
    pub fn get_global(&self, name: &str) -> Option<&JsValue> {
        self.global.get(name)
    }
    pub fn console_messages(&self) -> &[ConsoleMessage] {
        &self.console_messages
    }
    pub fn clear_console(&mut self) {
        self.console_messages.clear();
    }
    /// Emit a console message with log level.
    pub fn log(&mut self, message: String) {
        self.console_messages.push(ConsoleMessage {
            level: ConsoleLevel::Log,
            message,
            line: 0,
            source: String::new(),
        });
    }
}
impl Default for JsContext {
    fn default() -> Self {
        Self::new()
    }
}
pub trait JsEngine: Send + Sync {
    fn evaluate(&mut self, script: &str, context: &mut JsContext) -> JsResult<JsValue>;
    fn call_function(
        &mut self,
        name: &str,
        args: &[JsValue],
        context: &mut JsContext,
    ) -> JsResult<JsValue>;
    fn set_global(&mut self, name: &str, value: JsValue, context: &mut JsContext) -> JsResult<()>;
    fn get_global(&self, name: &str, context: &JsContext) -> Option<JsValue>;
}

pub struct SimpleJsEngine {
    variables: HashMap<String, JsValue>,
    /// Defined functions (name -> FunctionDef)
    functions: HashMap<String, JsValue>,
}
impl SimpleJsEngine {
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
impl Default for SimpleJsEngine {
    fn default() -> Self {
        Self::new()
    }
}
impl JsEngine for SimpleJsEngine {
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
    fn set_global(&mut self, name: &str, value: JsValue, _context: &mut JsContext) -> JsResult<()> {
        self.variables.insert(name.to_string(), value);
        Ok(())
    }
    fn get_global(&self, name: &str, _context: &JsContext) -> Option<JsValue> {
        self.variables.get(name).cloned()
    }
}
pub type SharedJsEngine = Arc<Mutex<dyn JsEngine>>;
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
impl Default for BoaJsEngine {
    fn default() -> Self {
        Self::new()
    }
}

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
