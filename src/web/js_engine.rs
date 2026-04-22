use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(HashMap<String, JsValue>),
    Function(String),
}

impl Default for JsValue {
    fn default() -> Self {
        Self::Undefined
    }
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
            JsValue::Function(_) => true,
        }
    }

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
                let items: Vec<String> = o
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            JsValue::Function(name) => format!("[Function: {}]", name),
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
            JsValue::Array(_) | JsValue::Object(_) | JsValue::Function(_) => f64::NAN,
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
        Self {
            message,
            stack: None,
            line: None,
            column: None,
        }
    }

    pub fn with_location(message: String, line: u32, column: u32) -> Self {
        Self {
            message,
            stack: None,
            line: Some(line),
            column: Some(column),
        }
    }
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsError: {}", self.message)?;
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, " at line {}, column {}", line, column)?;
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

        Self {
            global,
            console_messages: Vec::new(),
        }
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

    // ...existing code...
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
}

impl SimpleJsEngine {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    fn parse_value(&self, s: &str) -> JsValue {
        let s = s.trim();

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
        if let Ok(n) = s.parse::<f64>() {
            return JsValue::Number(n);
        }

        if let Some(v) = self.variables.get(s) {
            return v.clone();
        }

        JsValue::Undefined
    }
}

impl Default for SimpleJsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl JsEngine for SimpleJsEngine {
    fn evaluate(&mut self, script: &str, _context: &mut JsContext) -> JsResult<JsValue> {
        let script = script.trim();

        if script.is_empty() {
            return Ok(JsValue::Undefined);
        }

        if script.starts_with("console.log(") || script.starts_with("console.info(") {
            let start = script.find('(').unwrap() + 1;
            let end = script.rfind(')').unwrap();
            let content = &script[start..end];
            let value = self.parse_value(content);
            return Ok(value);
        }

        if script.starts_with("var ") {
            let rest = &script[4..];
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim().to_string();
                let value_str = rest[eq_pos + 1..].trim();
                let value = self.parse_value(value_str);
                self.variables.insert(name, value.clone());
                return Ok(value);
            }
        }

        if script.starts_with("let ") || script.starts_with("const ") {
            let rest = if script.starts_with("let ") {
                &script[4..]
            } else {
                &script[6..]
            };
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim().to_string();
                let value_str = rest[eq_pos + 1..].trim();
                let value = self.parse_value(value_str);
                self.variables.insert(name, value.clone());
                return Ok(value);
            }
        }

        if script.contains('=') && !script.starts_with('=') {
            let parts: Vec<&str> = script.splitn(2, '=').collect();
            if parts.len() == 2 {
                let name = parts[0].trim().to_string();
                let value_str = parts[1].trim();
                let value = self.parse_value(value_str);
                self.variables.insert(name, value.clone());
                return Ok(value);
            }
        }

        Ok(self.parse_value(script))
    }

    fn call_function(
        &mut self,
        name: &str,
        args: &[JsValue],
        _context: &mut JsContext,
    ) -> JsResult<JsValue> {
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
            _ => Ok(JsValue::Undefined),
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
}
