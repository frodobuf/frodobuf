use anyhow::anyhow;
use frodobuf::render::{OutputLanguage, RenderConfig, Renderer};
use midl_parser::parse_string;

const INPUT_FILE: &str = "./system.midl";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(&std::env::var("OUT_DIR").unwrap());

    let t = std::time::SystemTime::now();
    println!("cargo:rerun-if-changed={}", INPUT_FILE);
    eprintln!("# codegen ran at t={:?}", t);

    let idl_text = std::fs::read_to_string(INPUT_FILE)
        .map_err(|e| anyhow!("reading input file '{}': {}", INPUT_FILE, e))?;
    let descriptor = parse_string(&idl_text).map_err(|e| anyhow!("problem with the idl: {}", e))?;
    let schema = descriptor.schema;

    let mut renderer = Renderer::init(&RenderConfig {
        language: OutputLanguage::Rust,
        ..Default::default()
    })?;
    renderer.set("schema", serde_json::to_value(&schema)?);
    renderer.codegen_for_schema(&schema, &out_dir)?;
    Ok(())
}
