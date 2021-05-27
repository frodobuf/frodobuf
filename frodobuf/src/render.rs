//! Code generation
//!
use crate::codegen::CodegenError;
use std::collections::BTreeMap;
//use chrono::DateTime;
use handlebars::{Handlebars, JsonValue};
//use serde_json::Value as JsonValue;
//use toml::value::Value as TomlValue;

type VarMap = BTreeMap<String, JsonValue>;

// these defaults can be overridden by the config file
/// Pairing of template name and contents
///
pub type Template<'template> = (&'template str, &'template str);

/// Languages available for output generation
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OutputLanguage {
    Rust,
}

impl Default for OutputLanguage {
    fn default() -> OutputLanguage {
        OutputLanguage::Rust
    }
}

impl std::str::FromStr for OutputLanguage {
    type Err = CodegenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rust" => Ok(OutputLanguage::Rust),
            _ => Err(CodegenError::InvalidParameter(format!(
                "Unsupported language {}",
                s
            ))),
        }
    }
}

#[derive(Debug)]
pub struct RenderConfig<'render> {
    /// Templates to be loaded for renderer. List of template name, data
    pub templates: Vec<Template<'render>>,
    /// Whether parser is in strict mode (e.g. if true, a variable used in template
    /// that is undefined would raise an error; if false, it would evaluate to 'falsey'
    pub strict_mode: bool,

    /// Output language
    pub language: OutputLanguage,
}

impl<'render> Default for RenderConfig<'render> {
    fn default() -> Self {
        Self {
            templates: Vec::new(),
            strict_mode: true,
            language: OutputLanguage::Rust,
        }
    }
}

/// HBTemplate processor for code generation
pub struct Renderer<'gen> {
    /// Handlebars processor
    hb: Handlebars<'gen>,
    /// Additional dictionary that supplements data passed to render() method
    vars: VarMap,

    /// lanaguage for codegen
    language: OutputLanguage,
}

impl<'gen> Default for Renderer<'gen> {
    fn default() -> Self {
        // unwrap ok because only error condition occurs with templates, and default has none.
        Self::init(&RenderConfig::default()).unwrap()
    }
}

impl<'gen> Renderer<'gen> {
    /// Initialize handlebars template processor.
    pub fn init(config: &RenderConfig) -> Result<Self, CodegenError> {
        let mut hb = Handlebars::new();
        // don't use strict mode because
        // it's easier in templates to use if we allow undefined ~= false-y
        hb.set_strict_mode(config.strict_mode);
        hb.register_escape_fn(handlebars::no_escape); //html escaping is the default and cause issue0

        // add common helpers and templates
        add_base_helpers(&mut hb);
        for t in &config.templates {
            hb.register_template_string(t.0, t.1)?;
        }

        // add language-specific helpers and templates
        match &config.language {
            OutputLanguage::Rust => {
                crate::codegen::rust::add_helpers(&mut hb)?;
                crate::codegen::rust::add_templates(&mut hb)?;
            }
        }

        let renderer = Self {
            hb,
            vars: VarMap::default(),
            language: config.language,
        };
        Ok(renderer)
    }

    /// Set a value in the renderer dict. If the key was previously set, it is replaced.
    /// Values in the renderer dict override any values passed to render()
    pub fn set<K: Into<String>, V: Into<JsonValue>>(&mut self, key: K, val: V) {
        self.vars.insert(key.into(), val.into());
    }

    /// Remove key if it was present
    pub fn remove(&mut self, key: &str) {
        self.vars.remove(key);
    }

    /// Adds template to internal dictionary
    pub fn add_template(&mut self, template: Template) -> Result<(), CodegenError> {
        self.hb.register_template_string(template.0, template.1)?;
        Ok(())
    }

    /// Render a template
    pub fn render<W>(
        &self,
        template_name: &str,
        //other_vars: &mut VarMap,
        writer: &mut W,
    ) -> Result<(), CodegenError>
    where
        W: std::io::Write,
    {
        self.hb.render_to_write(template_name, &self.vars, writer)?;
        Ok(())
    }

    // FIXME
    //r.set("module_name", module_name);

    /// render code dependent on schema changes
    pub fn codegen_for_schema(
        &mut self,
        schema: &frodobuf_schema::model::Schema,
        output_dir: &std::path::Path,
    ) -> Result<(), CodegenError> {
        match &self.language {
            OutputLanguage::Rust => {
                crate::codegen::rust::codegen_schema_rust(self, schema, output_dir)?;
            }
        }
        Ok(())
    }
}

/// Convert Value to string without adding quotes around strings
fn json_value_to_string(v: &JsonValue) -> String {
    match v {
        JsonValue::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

// convert an ident object to a String
// also accepts an actual string
// TODO: omits namespace field
pub fn ident_to_string(v: &JsonValue) -> Result<String, handlebars::RenderError> {
    match v {
        serde_json::Value::Object(map) => match map.iter().find(|(k, _)| k.as_str() == "name") {
            Some((_, val)) => Ok(val
                .as_str()
                .ok_or_else(|| {
                    handlebars::RenderError::new(format!(
                        "expected string value for name, got {:?}",
                        val
                    ))
                })?
                .to_string()),
            None => Err(handlebars::RenderError::new(format!(
                "missing name attribute for {:?}",
                v
            ))),
        },
        serde_json::Value::String(s) => Ok(s.to_string()),

        _ => Err(handlebars::RenderError::new(format!(
            "expected identifier, got {:?}",
            v
        ))),
    }
}

/// Add template helpers functions
///  'join-csv' turns array of values into comma-separate list
///  'format-date' rewrites an ISO8601-formatted date into another format
fn add_base_helpers(hb: &mut Handlebars) {
    use handlebars::{Context, Helper, HelperResult, Output, RenderContext, RenderError};

    // "snake-case" converts a simple identifier to snake_case
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "to-snake-case",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("param not found"))?
                    .value();
                let term = ident_to_string(value)?;
                if term.contains(':') || term.contains('.') {
                    return Err(RenderError::new(format!(
                        "string must be a simple term with no '.' or ':'. This is invalid: '{}'",
                        term
                    )));
                }
                let mod_name = crate::strings::to_snake_case(&term);
                out.write(&mod_name)?;
                Ok(())
            },
        ),
    );

    // "to-pascal-case" converts a simple identifier to snake_case
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "to-pascal-case",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("param not found"))?
                    .value();
                let term = ident_to_string(value)?;
                let ident = crate::strings::to_pascal_case(&term);
                out.write(&ident)?;
                Ok(())
            },
        ),
    );

    // "to-camel-case" converts a simple identifier to snake_case
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "to-camel-case",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("param not found"))?
                    .value();
                let term = ident_to_string(value)?;
                let ident = crate::strings::to_camel_case(&term);
                out.write(&ident)?;
                Ok(())
            },
        ),
    );

    // "ident" converts an ident object to a string
    // This can be used to make safe method names, module names, and varialbe names
    hb.register_helper(
        "ident",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("param not found"))?
                    .value();
                let term = ident_to_string(value)?;
                out.write(&term)?;
                Ok(())
            },
        ),
    );

    // "join-csv" turns array of values into comma-separated list
    // Converts each value using to_string()
    hb.register_helper(
        "join-csv",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let csv = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("param not found"))?
                    .value()
                    .as_array()
                    .ok_or_else(|| RenderError::new("expected array"))?
                    .iter()
                    .map(json_value_to_string)
                    .collect::<Vec<String>>()
                    .join(",");
                out.write(&csv)?;
                Ok(())
            },
        ),
    );
    //
    // format-date: strftime-like function to reformat date
    hb.register_helper(
        "format-date",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                // get first arg as string, an ISO8601-formatted date
                let date = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("expect first param as date"))?
                    .value()
                    .as_str()
                    .ok_or_else(|| RenderError::new("expect strings"))?;
                // parse into DateTime
                let date = chrono::DateTime::parse_from_rfc3339(date)
                    .map_err(|e| RenderError::from_error("date parse", e))?;
                // get second arg - the format string
                let format = h
                    .param(1)
                    .ok_or_else(|| RenderError::new("expect second param as format"))?
                    .value()
                    .as_str()
                    .ok_or_else(|| RenderError::new("expect strings"))?;
                // print date in specified format
                let formatted = date.format(format).to_string();
                out.write(&formatted)?;
                Ok(())
            },
        ),
    );
}

#[test]
fn initializers() {
    let mut r1 = Renderer::default();
    r1.set(
        "x".to_string(),
        serde_json::Value::String("xyz".to_string()),
    );
    assert!(true);

    let mut r2 = Renderer::init(&RenderConfig::default()).expect("ok");
    r2.set(
        "x".to_string(),
        serde_json::Value::String("xyz".to_string()),
    );
    assert!(true);
}
