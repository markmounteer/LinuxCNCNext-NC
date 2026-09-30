//! Bounded, duplicate-key rejecting JSON and the legacy semantic fingerprint's
//! ECMAScript number spelling. Object insertion order is deliberately retained.
use crate::{part21::Limits, Diagnostic, Result};
use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::{cell::Cell, fmt};

struct Budget<'a> {
    limits: &'a Limits,
    values: Cell<usize>,
}
struct Seed<'a> {
    budget: &'a Budget<'a>,
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Value, D::Error> {
        if self.depth > self.budget.limits.nesting {
            return Err(D::Error::custom("JSON nesting limit exceeded"));
        }
        let count = self.budget.values.get();
        if count >= self.budget.limits.values {
            return Err(D::Error::custom("JSON value limit exceeded"));
        }
        self.budget.values.set(count + 1);
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON with unique object keys")
    }
    fn visit_bool<E: DeError>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_unit<E: DeError>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: DeError>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: DeError>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: DeError>(self, v: f64) -> std::result::Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("Nonfinite JSON number"))
    }
    fn visit_str<E: DeError>(self, v: &str) -> std::result::Result<Value, E> {
        self.visit_string(v.into())
    }
    fn visit_string<E: DeError>(self, v: String) -> std::result::Result<Value, E> {
        if v.len() > self.budget.limits.string_bytes {
            return Err(E::custom("JSON string limit exceeded"));
        }
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            if items.len() >= self.budget.limits.aggregate_items {
                return Err(A::Error::custom("JSON aggregate limit exceeded"));
            }
            items.push(item);
        }
        Ok(Value::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut items = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if items.len() >= self.budget.limits.aggregate_items
                || key.len() > self.budget.limits.string_bytes
            {
                return Err(A::Error::custom("JSON object/key limit exceeded"));
            }
            if items.contains_key(&key) {
                return Err(A::Error::custom("Duplicate JSON object key"));
            }
            items.insert(
                key,
                map.next_value_seed(Seed {
                    budget: self.budget,
                    depth: self.depth + 1,
                })?,
            );
        }
        Ok(Value::Object(items))
    }
}
pub fn parse(text: &str, limits: &Limits) -> Result<Value> {
    limits.check()?;
    if text.len() > limits.input_bytes {
        return Err(Diagnostic::new(
            "json",
            "INPUT_SIZE",
            "JSON exceeds byte limit",
        ));
    }
    let budget = Budget {
        limits,
        values: Cell::new(0),
    };
    let mut d = serde_json::Deserializer::from_str(text);
    let value = Seed {
        budget: &budget,
        depth: 0,
    }
    .deserialize(&mut d)
    .map_err(json_error)?;
    d.end().map_err(json_error)?;
    Ok(value)
}
fn json_error(e: serde_json::Error) -> Diagnostic {
    Diagnostic::new("json", "JSON", e.to_string())
        .with("jsonLine", e.line())
        .with("jsonByteColumn", e.column())
}
/// Semantic hashing, not a wire ABI. Call only on validated bounded model data.
pub fn stringify(value: &Value) -> Result<String> {
    fn emit(value: &Value, out: &mut String) -> Result<()> {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
            Value::Number(n) => {
                let n = n.as_f64().filter(|n| n.is_finite()).ok_or_else(|| {
                    Diagnostic::new("serialize", "NUMBER", "Invalid semantic number")
                })?;
                if n == 0.0 {
                    out.push('0');
                } else {
                    out.push_str(ryu_js::Buffer::new().format_finite(n));
                }
            }
            Value::String(s) => out.push_str(
                &serde_json::to_string(s)
                    .map_err(|e| Diagnostic::new("serialize", "STRING", e.to_string()))?,
            ),
            Value::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    emit(v, out)?;
                }
                out.push(']');
            }
            Value::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    emit(&Value::String(k.clone()), out)?;
                    out.push(':');
                    emit(v, out)?;
                }
                out.push('}');
            }
        }
        Ok(())
    }
    let mut out = String::new();
    emit(value, &mut out)?;
    Ok(out)
}
