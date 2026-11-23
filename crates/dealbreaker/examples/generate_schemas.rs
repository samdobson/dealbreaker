//! Regenerate `docs/schemas/*.schema.json` from the core Rust types.
//!
//! Run from the repository root:
//! ```sh
//! cargo run -p dealbreaker --example generate_schemas --features schema
//! ```

use std::fs;
use std::path::Path;

use dealbreaker::schema;

fn main() {
    let dir = Path::new("docs/schemas");
    fs::create_dir_all(dir).expect("create docs/schemas");
    let files = [
        ("rules.schema.json", schema::rules_schema_json()),
        ("state.schema.json", schema::state_schema_json()),
        ("step.schema.json", schema::step_schema_json()),
        (
            "solve-options.schema.json",
            schema::solve_options_schema_json(),
        ),
        (
            "hint-options.schema.json",
            schema::hint_options_schema_json(),
        ),
    ];
    for (name, mut content) in files {
        content.push('\n');
        let path = dir.join(name);
        fs::write(&path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}
