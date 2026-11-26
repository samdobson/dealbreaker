//! JSON Schema generation for rules, positions, steps, and solver options.
//!
//! Enabled by the optional `schema` Cargo feature (`dealbreaker = { features = ["schema"] }`).
//! Use these schemas with editors (VS Code, JetBrains) or code generators so custom game JSON
//! files get live autocomplete and validation.

use schemars::schema::RootSchema;

use crate::{GradeOptions, HintOptions, Rules, SolveOptions, StateData, Step};

/// JSON Schema for [`Rules`] (custom game definitions).
pub fn rules_schema() -> RootSchema {
    schemars::schema_for!(Rules)
}

/// JSON Schema for [`StateData`] (positions).
pub fn state_schema() -> RootSchema {
    schemars::schema_for!(StateData)
}

/// JSON Schema for [`Step`] (player-visible moves).
pub fn step_schema() -> RootSchema {
    schemars::schema_for!(Step)
}

/// JSON Schema for [`SolveOptions`].
pub fn solve_options_schema() -> RootSchema {
    schemars::schema_for!(SolveOptions)
}

/// JSON Schema for [`HintOptions`].
pub fn hint_options_schema() -> RootSchema {
    schemars::schema_for!(HintOptions)
}

/// JSON Schema for [`GradeOptions`].
pub fn grade_options_schema() -> RootSchema {
    schemars::schema_for!(GradeOptions)
}

/// Pretty-printed JSON Schema for [`GradeOptions`].
pub fn grade_options_schema_json() -> String {
    serde_json::to_string_pretty(&grade_options_schema()).expect("schema serializes")
}

/// Pretty-printed JSON Schema for [`Rules`].
pub fn rules_schema_json() -> String {
    serde_json::to_string_pretty(&rules_schema()).expect("schema serializes")
}

/// Pretty-printed JSON Schema for [`StateData`].
pub fn state_schema_json() -> String {
    serde_json::to_string_pretty(&state_schema()).expect("schema serializes")
}

/// Pretty-printed JSON Schema for [`Step`].
pub fn step_schema_json() -> String {
    serde_json::to_string_pretty(&step_schema()).expect("schema serializes")
}

/// Pretty-printed JSON Schema for [`SolveOptions`].
pub fn solve_options_schema_json() -> String {
    serde_json::to_string_pretty(&solve_options_schema()).expect("schema serializes")
}

/// Pretty-printed JSON Schema for [`HintOptions`].
pub fn hint_options_schema_json() -> String {
    serde_json::to_string_pretty(&hint_options_schema()).expect("schema serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_generate_valid_json() {
        for (name, json) in [
            ("rules", rules_schema_json()),
            ("state", state_schema_json()),
            ("step", step_schema_json()),
            ("solve-options", solve_options_schema_json()),
            ("hint-options", hint_options_schema_json()),
        ] {
            let v: serde_json::Value = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("{name} schema invalid JSON: {e}"));
            assert!(v.is_object(), "{name} schema must be an object");
            assert!(
                v.get("$schema").is_some(),
                "{name} schema should declare $schema"
            );
        }
    }

    #[test]
    fn rules_accept_dollar_schema_field() {
        let mut v: serde_json::Value =
            serde_json::from_str(&crate::presets::freecell().to_json()).unwrap();
        v["$schema"] = serde_json::json!(
            "https://raw.githubusercontent.com/samdobson/dealbreaker/main/docs/schemas/rules.schema.json"
        );
        let with_schema =
            Rules::from_json(&v.to_string()).expect("rules with $schema should parse");
        assert_eq!(with_schema, crate::presets::freecell());
        // $schema is not re-emitted when serializing rules back out.
        assert!(!with_schema.to_json().contains("$schema"));
    }

    #[test]
    fn checked_in_schemas_are_up_to_date() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
        if !root.exists() {
            return;
        }
        for (name, generated) in [
            ("rules.schema.json", rules_schema_json()),
            ("state.schema.json", state_schema_json()),
            ("step.schema.json", step_schema_json()),
            ("solve-options.schema.json", solve_options_schema_json()),
            ("hint-options.schema.json", hint_options_schema_json()),
        ] {
            let on_disk = std::fs::read_to_string(root.join(name))
                .unwrap_or_else(|e| panic!("missing docs/schemas/{name}: {e}"));
            assert_eq!(
                on_disk.trim_end(),
                generated.trim_end(),
                "docs/schemas/{name} is out of date; run `cargo run -p dealbreaker --example generate_schemas --features schema`"
            );
        }
    }
}
