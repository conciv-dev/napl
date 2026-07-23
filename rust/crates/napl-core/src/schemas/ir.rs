//! Intermediate-representation (IR) schema.

use serde::Deserialize;

use super::{require_non_empty, SchemaError};

/// A named IR type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IrType {
    pub name: String,
    pub description: String,
}

/// A named IR function.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IrFunction {
    pub name: String,
    pub signature: String,
    pub behavior: String,
}

fn default_contract() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// An IR test case; `given`/`expect` accept an object or a string.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct IrTest {
    pub name: String,
    #[serde(default = "default_contract")]
    pub given: serde_json::Value,
    #[serde(default = "default_contract")]
    pub expect: serde_json::Value,
}

/// The full IR document.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Ir {
    pub module: String,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub types: Vec<IrType>,
    #[serde(default)]
    pub functions: Vec<IrFunction>,
    #[serde(default)]
    pub tests: Vec<IrTest>,
}

fn validate_contract(value: &serde_json::Value, field: &str) -> Result<(), SchemaError> {
    if value.is_object() || value.is_string() {
        Ok(())
    } else {
        Err(SchemaError::Validation(format!(
            "{field} must be an object or a string"
        )))
    }
}

/// Validate an IR document, mirroring `validateIr`.
pub fn validate_ir(value: serde_json::Value) -> Result<Ir, SchemaError> {
    let ir: Ir =
        serde_json::from_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    require_non_empty(&ir.module, "module")?;
    for test in &ir.tests {
        validate_contract(&test.given, "given")?;
        validate_contract(&test.expect, "expect")?;
    }
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_well_formed_ir_with_defaults() {
        let ir = validate_ir(json!({
            "module": "greeting",
            "functions": [{ "name": "greet", "signature": "greet(name): string", "behavior": "returns Hello" }]
        }))
        .unwrap();
        assert_eq!(ir.module, "greeting");
        assert!(ir.deps.is_empty());
        assert!(ir.types.is_empty());
        assert!(ir.tests.is_empty());
        assert_eq!(ir.functions[0].name, "greet");
    }

    #[test]
    fn rejects_ir_without_module() {
        assert!(validate_ir(json!({ "functions": [] })).is_err());
    }

    #[test]
    fn rejects_empty_module() {
        assert!(validate_ir(json!({ "module": "", "functions": [] })).is_err());
    }

    #[test]
    fn rejects_malformed_function_entries() {
        assert!(validate_ir(json!({ "module": "x", "functions": [{ "name": "f" }] })).is_err());
    }

    #[test]
    fn contract_accepts_object_or_string() {
        let ir = validate_ir(json!({
            "module": "m",
            "tests": [
                { "name": "a", "given": { "x": 1 }, "expect": "ok" }
            ]
        }))
        .unwrap();
        assert_eq!(ir.tests[0].expect, json!("ok"));
    }

    #[test]
    fn contract_defaults_to_empty_object() {
        let ir = validate_ir(json!({ "module": "m", "tests": [{ "name": "a" }] })).unwrap();
        assert_eq!(ir.tests[0].given, json!({}));
    }
}
