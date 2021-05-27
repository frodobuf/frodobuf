pub mod rust;
/// utility for running 'rustfmt'
#[cfg(not(target_arch = "wasm32"))]
pub mod rustfmt;
use thiserror::Error as ThisError;

/// Errors from the code generator
#[derive(ThisError, Debug)]
pub enum CodegenError {
    /// Error occurred parsing the handlebars template
    #[error("Error reading handlebars template: {0}")]
    HandlebarsTemplate(#[from] handlebars::TemplateError),

    /// Error occurred when rendering handlebars template
    #[error("Error processing handlebars template: {0}")]
    HandlebarsRender(#[from] handlebars::RenderError),

    /// A file IO error occurred
    #[error("IO Error")]
    Io(#[from] std::io::Error),

    /// A function was invoked with invalid parameters
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    /// Some other error occurred
    #[error("unexpected error: {0}")]
    Other(String),
}

/// Extracts the base filename, without extension, converting to snake case
/// ```rust
///   use frodobuf::codegen::module_name_from_file;
///   use std::path::PathBuf;
///   assert_eq!(
///     module_name_from_file(&PathBuf::from("../foo/bar.midl")),
///     "bar".to_string());
/// ```
pub fn module_name_from_file(file: &std::path::Path) -> String {
    let path = &file.to_string_lossy();
    let basename = match path.rsplit_once('/') {
        None => path,
        Some((_left, right)) => right,
    };
    let no_suffix = match basename.split_once('.') {
        None => basename,
        Some((left, _right)) => left,
    };
    crate::strings::to_snake_case(no_suffix)
}
