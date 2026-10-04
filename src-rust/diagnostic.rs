use serde::Serialize;
use std::fmt;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Location {
    pub byte_offset: usize,
    pub line: usize,
    pub byte_column: usize,
    pub record: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub stage: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Box<Location>>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub context: std::collections::BTreeMap<String, serde_json::Value>,
}
impl Diagnostic {
    pub fn new(stage: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            code: code.into(),
            message: message.into(),
            source: None,
            context: std::collections::BTreeMap::new(),
        }
    }
    pub fn at(mut self, source: Location) -> Self {
        self.source = Some(Box::new(source));
        self
    }
    pub fn with(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]: {}", self.stage, self.code, self.message)?;
        if let Some(s) = &self.source {
            write!(
                f,
                " (line {}, byte column {}, record {:?})",
                s.line, s.byte_column, s.record
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for Diagnostic {}
pub type Result<T> = std::result::Result<T, Diagnostic>;
