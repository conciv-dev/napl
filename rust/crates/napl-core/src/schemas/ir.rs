//! Stage1 adapter over the NAPL-generated `schemas_ir` crate. Its
//! `IrValidationError` is mapped to the shared `SchemaError`.

use super::SchemaError;

pub use schemas_ir::{Ir, IrFunction, IrTest, IrType};

pub fn validate_ir(value: serde_json::Value) -> Result<Ir, SchemaError> {
    schemas_ir::validate_ir(value).map_err(|e| SchemaError::Deserialize(e.to_string()))
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
