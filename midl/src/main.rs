use anyhow::{anyhow, Result};
use clap::{self, Clap, ValueHint};
use frodobuf::{
    codegen::rust,
    render::{OutputLanguage, RenderConfig, Renderer},
};
use frodobuf_schema::model::Schema;
use midl_parser::parse_string;
use std::{fs, path::PathBuf};

#[derive(Clap, Debug)]
#[clap(name = "midl", about, version)]
struct Opt {
    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Subcommand
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Clap)]
pub enum Command {
    /// Parse idl into json
    #[clap(name = "json", alias = "schema")]
    Json(JsonOpt),

    /// Update source files after change in idl source
    #[clap(name = "update")]
    Update(UpdateOpt),

    /// create new source project from idl file
    #[clap(name = "create")]
    Create(CreateOpt),
}

#[derive(Clap, Debug)]
pub struct JsonOpt {
    /// Output file
    #[clap(short, long)]
    output: PathBuf,

    /// Write output in prettified format
    #[clap(long)]
    pretty: bool,

    /// Input files to process
    #[clap(short, long)]
    input: PathBuf,
}

#[derive(Clap, Debug)]
pub struct CreateOpt {
    /// Input idl file
    #[clap(short, long)]
    input: PathBuf,

    /// Output language. Multiple languages may be specified as `-l lang1 -l lang2 ...`
    #[clap(short, long, alias = "lang")]
    language: Vec<OutputLanguage>,

    /// Existing output directory where files will be generated. Defaults to current directory.
    #[clap(short, long, parse(from_os_str), value_hint = ValueHint::FilePath)]
    output_dir: Option<PathBuf>,

    /// Rust edition (default 2018)
    #[clap(long, default_value = "2018")]
    edition: String,

    /// Package name for Cargo.toml.
    /// Default value is "X-interface", where X is the base name of the midl file.
    #[clap(long)]
    package: Option<String>,
}

#[derive(Clap, Debug)]
pub struct UpdateOpt {
    /// Input schema
    #[clap(short, long, parse(from_os_str), value_hint = ValueHint::AnyPath)]
    input: PathBuf,

    /// Output language.
    #[clap(short, long, alias = "lang")]
    language: OutputLanguage,

    /// Existing output directory where file will be generated. Defaults to current directory.
    #[clap(short, long, parse(from_os_str), value_hint = ValueHint::FilePath)]
    output_dir: Option<PathBuf>,
}

fn current_dir() -> PathBuf {
    match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => PathBuf::from("."),
    }
}

fn main() {
    let opt = Opt::parse();
    if opt.verbose > 2 {
        println!("{:#?}", &opt);
    }

    if let Err(e) = run(opt) {
        eprintln!("Error: {}", e.to_string());
    }
}

fn run(opt: Opt) -> Result<()> {
    match &opt.command {
        Command::Json(json_opt) => to_json(json_opt)?,
        Command::Update(gen_opt) => update(gen_opt)?,
        Command::Create(create_opt) => create(create_opt)?,
    }
    Ok(())
}

// parse idl and save schema json
fn to_json(opt: &JsonOpt) -> Result<()> {
    let schema = parse_idl(&opt.input)?;
    let schema_json = if opt.pretty {
        serde_json::to_string_pretty(&schema)?
    } else {
        serde_json::to_string(&schema)?
    };

    fs::write(&opt.output, &schema_json.as_bytes())
        .map_err(|e| anyhow!("writing output file '{}': {}", &opt.output.display(), e))?;
    Ok(())
}

/// Create a project (only rust currently supported)
fn create(opt: &CreateOpt) -> Result<()> {
    // first ensure we can read the schema
    let schema = parse_idl(&opt.input)?;
    let schema_json = serde_json::to_value(&schema)?;
    let package = if let Some(package) = opt.package.as_ref() {
        package.clone()
    } else {
        schema.namespace.name.clone()
    };
    let output = match opt.output_dir.as_ref() {
        Some(o) => o.clone(),
        None => current_dir(),
    };
    if !output.is_dir() {
        return Err(anyhow!(
            "output-dir parameter must be an existing directory"
        ));
    }
    for language in opt.language.iter() {
        let mut renderer = Renderer::init(&RenderConfig {
            language: *language,
            ..Default::default()
        })?;
        renderer.set("schema", schema_json.clone());
        match language {
            OutputLanguage::Rust => {
                rust::create_project(
                    &mut renderer,
                    &schema,
                    rust::CreateProject {
                        input: &opt.input,
                        output: &output,
                        package: &package,
                        edition: &opt.edition,
                    },
                )?;
            }
        }
    }

    Ok(())
}

fn update(opt: &UpdateOpt) -> Result<()> {
    let schema = parse_idl(&opt.input)?;
    let schema_json = serde_json::to_value(&schema)?;
    let output = match opt.output_dir.as_ref() {
        Some(o) => o.clone(),
        None => current_dir(),
    };
    if !output.is_dir() {
        return Err(anyhow!(
            "output-dir parameter must be an existing directory"
        ));
    }

    let mut renderer = Renderer::init(&RenderConfig {
        language: opt.language,
        ..Default::default()
    })?;
    renderer.set("schema", schema_json);
    renderer.codegen_for_schema(&schema, &output)?;

    Ok(())
}

/// Read idl file and convert to Schema
fn parse_idl(input: &std::path::Path) -> Result<Schema> {
    let text = fs::read_to_string(input)
        .map_err(|e| anyhow!("reading input file '{}': {}", &input.to_string_lossy(), e))?;
    let descriptor = parse_string(&text)?;
    // set package name to the file base if not otherwise set
    Ok(descriptor.schema)
}
