//! Query parameters and their `qs`-compatible encoding.

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde_json::{Map, Number, Value};

use crate::error::Error;

/// Everything except the RFC 3986 unreserved set (`A-Z a-z 0-9 - _ . ~`).
pub(crate) const STRICT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Query parameters for a request.
///
/// Values can be any JSON: nested objects and arrays are encoded the way the
/// `qs` package does (`filter[status]=open`, `ids[0]=a`), which is what the
/// Truto API expects. Anything that converts into a JSON object also works
/// wherever a query is accepted, e.g. `serde_json::json!({"limit": 10})`.
///
/// ```
/// use truto::Query;
/// let q = Query::new().set("integrated_account_id", "ia_1").set("limit", 50);
/// assert_eq!(q.encode(), "integrated_account_id=ia_1&limit=50");
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query(Map<String, Value>);

impl Query {
    /// An empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `key` to `value`, builder style.
    pub fn set(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    /// Set `key` to `value` in place.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> &mut Self {
        self.0.insert(key.into(), value.into());
        self
    }

    /// The value for `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Remove `key`, returning its value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.0.remove(key)
    }

    /// `true` when there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The parameters as a JSON object.
    pub fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }

    /// Consume the query and return the parameters as a JSON object.
    pub fn into_map(self) -> Map<String, Value> {
        self.0
    }

    /// Encode as a query string (without the leading `?`).
    pub fn encode(&self) -> String {
        encode(&self.0)
    }

    pub(crate) fn merge(&mut self, other: Map<String, Value>) {
        for (k, v) in other {
            self.0.insert(k, v);
        }
    }
}

impl From<Map<String, Value>> for Query {
    fn from(map: Map<String, Value>) -> Self {
        Self(map)
    }
}

/// Types accepted as query parameters: [`Query`], a JSON object
/// ([`serde_json::Value`] or [`serde_json::Map`]), or references to them.
///
/// A `Value` that is not an object (or `null`) is rejected when the request
/// is sent.
pub trait IntoQuery {
    /// Convert into a JSON object of parameters.
    fn into_query(self) -> Result<Map<String, Value>, Error>;
}

impl IntoQuery for Query {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        Ok(self.0)
    }
}

impl IntoQuery for &Query {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        Ok(self.0.clone())
    }
}

impl IntoQuery for Map<String, Value> {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        Ok(self)
    }
}

impl IntoQuery for &Map<String, Value> {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        Ok(self.clone())
    }
}

impl IntoQuery for Value {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        match self {
            Value::Object(map) => Ok(map),
            Value::Null => Ok(Map::new()),
            other => Err(Error::InvalidRequest(format!(
                "query must be a JSON object, got {other}"
            ))),
        }
    }
}

impl IntoQuery for &Value {
    fn into_query(self) -> Result<Map<String, Value>, Error> {
        self.clone().into_query()
    }
}

/// Encode a JSON object exactly like `qs.stringify(obj, { sort })`.
pub fn encode(map: &Map<String, Value>) -> String {
    let mut parts = Vec::new();
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for key in keys {
        encode_value(key, &map[key.as_str()], &mut parts);
    }
    parts.join("&")
}

fn encode_value(prefix: &str, value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for key in keys {
                encode_value(&format!("{prefix}[{key}]"), &obj[key.as_str()], parts);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                encode_value(&format!("{prefix}[{i}]"), item, parts);
            }
        }
        Value::Null => parts.push(format!("{}=", escape(prefix))),
        Value::Bool(b) => parts.push(format!("{}={b}", escape(prefix))),
        Value::Number(n) => parts.push(format!("{}={}", escape(prefix), number(n))),
        Value::String(s) => parts.push(format!("{}={}", escape(prefix), escape(s))),
    }
}

/// Format a number the way JavaScript's `String(n)` does for common values.
fn number(n: &Number) -> String {
    if n.is_i64() || n.is_u64() {
        return n.to_string();
    }
    match n.as_f64() {
        Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e21 => format!("{f:.0}"),
        _ => n.to_string(),
    }
}

fn escape(s: &str) -> String {
    utf8_percent_encode(s, STRICT).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn whole_floats_drop_the_fraction() {
        let q = json!({"a": 50.0, "b": 1.25});
        assert_eq!(encode(q.as_object().unwrap()), "a=50&b=1.25");
    }

    #[test]
    fn builder_merges() {
        let mut q = Query::new().set("a", 1);
        q.merge(json!({"b": "x"}).into_query().unwrap());
        assert_eq!(q.encode(), "a=1&b=x");
    }
}
