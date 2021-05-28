//! Rust language code generation
//!
//!
use crate::{
    codegen::{rustfmt, CodegenError},
    render::{ident_to_string, Renderer},
    strings::{to_snake_case, unquote},
};
use frodobuf_schema::model::{Attribute, Schema};
use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonValue, Output, RenderContext, RenderError,
};
use std::{fs, path::Path};

/// return the helper param
#[inline]
fn param<'h>(h: &'h Helper, n: usize) -> Result<&'h JsonValue, RenderError> {
    Ok(h.param(n)
        .ok_or_else(|| RenderError::new("param not found"))?
        .value())
}

/// get object field
fn get<'h>(v: &'h JsonValue, key: &'_ str) -> Result<&'h JsonValue, RenderError> {
    v.as_object()
        .map(|map| map.get(key))
        .unwrap_or_default()
        .ok_or_else(|| RenderError::new(format!("expected object with field {}", key)))
}

/// get attributes of model object
fn get_attributes(v: &JsonValue) -> Result<Vec<Attribute>, RenderError> {
    serde_json::from_value(get(v, "attributes")?.clone())
        .map_err(|e| RenderError::new(format!("invalid attributes: {}", e)))
}

fn field_type_to_rust_type(type_val: &JsonValue) -> Result<String, String> {
    let rust_type = match type_val {
        JsonValue::String(s) => {
            match s.as_str() {
                "Uint8" => "u8",
                "Uint32" => "u32",
                "Uint64" => "u64",
                "Int8" => "u8",
                "Int32" => "u32",
                "Int64" => "u64",
                "Bool" => "bool",
                "Float32" => "f32",
                "Float64" => "f64",
                "String" => "String",
                "Bytes" => "Vec<u8>",
                "DateTime" => "String", // TODO: broken
                _ => {
                    return Err(format!("unexpected string type {:?}", type_val));
                }
            }
            .to_string()
        }
        JsonValue::Object(map) => {
            let (k, v) = map.iter().find(|_| true).unwrap();
            match k.as_str() {
                "ObjectOrEnum" => ident_to_string(v).map_err(|e| e.to_string())?,
                "Array" => {
                    let item_type = field_type_to_rust_type(v)
                        .map_err(|e| format!("invalid array item type {}", &e))?;
                    format!("Vec<{}>", item_type)
                }
                "Map" => {
                    if let JsonValue::Array(parts) = v {
                        if parts.len() == 2 {
                            let key_type = field_type_to_rust_type(parts.get(0).unwrap())
                                .map_err(|e| format!("invalid map key type {}", &e))?;
                            let val_type = field_type_to_rust_type(parts.get(1).unwrap())
                                .map_err(|e| format!("invalid map value type {}", &e))?;
                            format!("std::collections::HashMap<{},{}>", key_type, val_type)
                        } else {
                            return Err(
                                "invalid map: expecting two subtypes: map<key_type,value_type>"
                                    .to_string(),
                            );
                        }
                    } else {
                        return Err("invalid map encoding".to_string());
                    }
                }
                _ => {
                    return Err(format!("unexpected Object type {}", k));
                }
            }
        }
        _ => {
            panic!("expecting typename, found {:?}", type_val);
            //return Err(format!("expecting typename, found {:?}", type_val));
        }
    };
    Ok(rust_type)
}

/// genreate rust code dependent on schema - called for incremental builds after idl changes
pub fn codegen_schema_rust(
    r: &mut Renderer,
    schema: &frodobuf_schema::model::Schema,
    output_dir: &Path,
) -> Result<(), CodegenError> {
    // Most project files (Cargo.toml, build.rs, and lib.rs) don't change with idl changes.
    // The only file that needs updating is the project.rs in the build output dir
    let module_name = to_snake_case(&schema.namespace.name);
    let service_file = output_dir.join(format!("{}.rs", &module_name));
    let mut out = fs::File::create(&service_file)?;
    r.render("rust-service", &mut out)?;

    // run rustfmt
    let rust_format = rustfmt::RustFmtCommand::default();
    rust_format
        .execute(vec![service_file.as_path()])
        .map_err(|e| CodegenError::Other(format!("rustfmt: {}", e)))?;

    Ok(())
}

/// Helper functions - "macros" used within templates
/// These are used to ensure consistency when generating symbol names
pub fn add_helpers(hb: &mut Handlebars) -> Result<(), CodegenError> {
    // "to-type" converts a data type to a Rust type.
    // If it's an identifier, uses PascalCase
    hb.register_helper(
        "to-type",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let type_val = param(h, 0)?;
                let rust_type = field_type_to_rust_type(type_val).map_err(RenderError::new)?;
                out.write(&rust_type)?;
                Ok(())
            },
        ),
    );

    // like to-type but adds & in front
    hb.register_helper(
        "to-arg-ref",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let type_val = param(h, 0)?;
                let rust_type = field_type_to_rust_type(type_val).map_err(RenderError::new)?;
                out.write(&format!("&{}", &rust_type))?;
                Ok(())
            },
        ),
    );

    // "field-serde" adds any serde attributes for this field
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "field-serde",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let field = param(h, 0)?;
                let typ = get(field, "typ")?;
                if matches!(typ.as_str(), Some("Bytes")) {
                    out.write("#[serde(with=\"serde_bytes\")]\n")?;
                }
                // use declared name in serialized json, even if the rust field name is different
                let name = ident_to_string(get(field, "name")?)?;
                if name != to_snake_case(&name) {
                    out.write(&format!("#[serde(rename=\"{}\")]\n", name))?;
                }
                Ok(())
            },
        ),
    );

    // "docs" adds documentation attributes
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "docs",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let obj = param(h, 0)?;
                let attributes = get_attributes(obj)?;
                for doc in attributes
                    .iter()
                    .filter(|a| a.key.name.as_str() == "doc")
                    .map(|a| &a.values)
                {
                    for line in doc.iter().map(|x| x.1.to_string()) {
                        out.write(&format!("/// {}\n", unquote(&line.to_string())))?;
                    }
                }
                Ok(())
            },
        ),
    );

    Ok(())
}

/// Add rust code generation templates
pub fn add_templates(hb: &mut Handlebars) -> Result<(), CodegenError> {
    let templates: Vec<(&str, &str)> = vec![
        (
            "cargo-toml",
            include_str!("../../templates/rust/Cargo.toml.hbs"),
        ),
        ("rust-lib", include_str!("../../templates/rust/lib.rs.hbs")),
        (
            "rust-service",
            include_str!("../../templates/rust/service.rs.hbs"),
        ),
        (
            "build-rs",
            include_str!("../../templates/rust/build.rs.hbs"),
        ),
    ];
    for t in templates.iter() {
        hb.register_template_string(t.0, t.1)?;
    }
    Ok(())
}

/// Parameters needed for generating a set of files for a new Rust project
pub struct CreateProject<'cp> {
    /// path to idl input file
    pub input: &'cp Path,

    /// override package name from file
    pub package: &'cp str,

    /// rust edition. default=2018
    pub edition: &'cp str,

    /// output directory.
    pub output: &'cp Path,
}

/// Generate the full set of rust files for a project
pub fn create_project<'cp>(
    r: &mut Renderer,
    _schema: &Schema,
    arg: CreateProject<'cp>,
) -> Result<(), CodegenError> {
    let project_dir = arg.output.join("rust");
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // define variables needed for Cargo.toml and build.rs
    r.set("cargo-package", arg.package);
    r.set("cargo-edition", arg.edition);
    r.set(
        "idl-source",
        format!("../{}", &arg.input.display()).as_str(),
    );

    // generate Cargo.toml
    let cargo_out = project_dir.join("Cargo.toml");
    let mut out = fs::File::create(&cargo_out)?;
    r.render("cargo-toml", &mut out)?;

    // build.rs
    let build_rs_out = project_dir.join("build.rs");
    let mut out = fs::File::create(&build_rs_out)?;
    r.render("build-rs", &mut out)?;

    // lib.rs
    // the other source file that lib.rs is generated later by build.rs
    let lib_file = src_dir.join("lib.rs");
    let mut out = fs::File::create(&lib_file)?;
    r.render("rust-lib", &mut out)?;

    // run rustfmt on lib.rs
    let rust_format = rustfmt::RustFmtCommand::default();
    rust_format
        .execute(vec![lib_file.as_path()])
        .map_err(|e| CodegenError::Other(format!("rustfmt: {}", e)))?;

    Ok(())
}
