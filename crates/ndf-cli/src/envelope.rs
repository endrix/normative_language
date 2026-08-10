use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct Envelope {
    schema_version: &'static str,
    command: &'static str,
    pub ok: bool,
    data: Value,
    diagnostics: Vec<Value>,
}

impl Envelope {
    pub fn success<T: Serialize>(
        command: &'static str,
        data: T,
        diagnostics: Vec<Value>,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            schema_version: "0.1",
            command,
            ok: true,
            data: serde_json::to_value(data)?,
            diagnostics,
        })
    }

    pub fn checked<T: Serialize>(
        command: &'static str,
        ok: bool,
        data: T,
        diagnostics: Vec<Value>,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            schema_version: "0.1",
            command,
            ok,
            data: serde_json::to_value(data)?,
            diagnostics,
        })
    }

    pub fn failure(command: &'static str, message: impl Into<String>) -> Self {
        Self {
            schema_version: "0.1",
            command,
            ok: false,
            data: Value::Object(serde_json::Map::new()),
            diagnostics: vec![serde_json::json!({"message": message.into()})],
        }
    }
}
