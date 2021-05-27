//! Parser for MIDL files
//!
//! The parser in this crate is based on a protobuf parser
//! github.com/stepancheg/rust-protobuf
//!
#![deny(missing_docs)]
#![deny(broken_intra_doc_links)]

use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf, StripPrefixError},
};

use amend_io_error::amend_io_error;
use linked_hash_map::LinkedHashMap;

mod amend_io_error;
mod linked_hash_map;
mod model;
mod parser;

use crate::model::FileDescriptor;
pub use parser::parse_string;

//#[cfg(test)]
//mod test_against_protobuf_protos;
// Used by text format parser and by pure-rust codegen parsed
// this it is public but hidden module.
// https://github.com/rust-lang/rust/issues/44663
#[doc(hidden)]
pub(crate) mod lexer;

/// Current version of midl parser crate
pub const MIDL_PARSER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
struct WithFileError {
    file: String,
    error: CodegenError,
}

#[derive(Debug)]
enum CodegenError {
    ParserErrorWithLocation(parser::ParserErrorWithLocation),
    //ConvertError(convert::ConvertError),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::ParserErrorWithLocation(e) => write!(f, "{}", e),
            //CodegenError::ConvertError(e) => write!(f, "{}", e),
        }
    }
}

impl From<parser::ParserErrorWithLocation> for CodegenError {
    fn from(e: parser::ParserErrorWithLocation) -> Self {
        CodegenError::ParserErrorWithLocation(e)
    }
}

/*
impl From<convert::ConvertError> for CodegenError {
    fn from(e: convert::ConvertError) -> Self {
        CodegenError::ConvertError(e)
    }
}
 */

impl fmt::Display for WithFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error in {}: {}", self.file, self.error)
    }
}

impl Error for WithFileError {}

struct Run<'a> {
    parsed_files: LinkedHashMap<PathBuf, FileDescriptor>,
    includes: &'a [PathBuf],
}

impl<'a> Run<'a> {
    fn get_file_and_all_deps_already_parsed(
        &self,
        protobuf_path: &Path,
        result: &mut LinkedHashMap<PathBuf, FileDescriptor>,
    ) {
        if result.get(protobuf_path).is_some() {
            return;
        }

        let parsed = self
            .parsed_files
            .get(protobuf_path)
            .expect("must be already parsed");
        result.insert(protobuf_path.to_owned(), parsed.clone());

        self.get_all_deps_already_parsed(parsed, result);
    }

    fn get_all_deps_already_parsed(
        &self,
        parsed: &model::FileDescriptor,
        result: &mut LinkedHashMap<PathBuf, FileDescriptor>,
    ) {
        for import in &parsed.imports {
            self.get_file_and_all_deps_already_parsed(Path::new(&import.path), result);
        }
    }

    fn add_file(&mut self, protobuf_path: &Path, fs_path: &Path) -> io::Result<()> {
        if self.parsed_files.get(protobuf_path).is_some() {
            return Ok(());
        }

        let content = fs::read_to_string(fs_path)
            .map_err(|e| amend_io_error(e, format!("failed to read {:?}", fs_path)))?;

        self.add_file_content(protobuf_path, fs_path, &content)
    }

    fn add_file_content(
        &mut self,
        protobuf_path: &Path,
        fs_path: &Path,
        content: &str,
    ) -> io::Result<()> {
        let parsed = model::FileDescriptor::parse(content).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                WithFileError {
                    file: format!("{}", fs_path.display()),
                    error: e.into(),
                },
            )
        })?;

        for import in &parsed.imports {
            self.add_imported_file(Path::new(&import.path))?;
        }

        let mut this_file_deps = LinkedHashMap::new();
        self.get_all_deps_already_parsed(&parsed, &mut this_file_deps);

        self.parsed_files.insert(protobuf_path.to_owned(), parsed);

        Ok(())
    }

    fn add_imported_file(&mut self, protobuf_path: &Path) -> io::Result<()> {
        for include_dir in self.includes {
            let fs_path = include_dir.join(protobuf_path);
            if fs_path.exists() {
                return self.add_file(protobuf_path, &fs_path);
            }
        }
        Ok(())
    }

    fn strip_prefix<'b>(path: &'b Path, prefix: &Path) -> Result<&'b Path, StripPrefixError> {
        // special handling of `.` to allow successful `strip_prefix("foo.proto", ".")
        if prefix == Path::new(".") {
            Ok(path)
        } else {
            path.strip_prefix(prefix)
        }
    }

    fn add_fs_file(&mut self, fs_path: &Path) -> io::Result<PathBuf> {
        let relative_path = self
            .includes
            .iter()
            .filter_map(|include_dir| Self::strip_prefix(fs_path, include_dir).ok())
            .next();

        match relative_path {
            Some(relative_path) => {
                assert!(relative_path.is_relative());
                self.add_file(relative_path, fs_path)?;
                Ok(relative_path.to_owned())
            }
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "file {:?} must reside in include path {:?}",
                    fs_path, self.includes
                ),
            )),
        }
    }
}

/// Validated model from parser
pub struct ParsedAndTypechecked {
    /// Paths loaded
    pub relative_paths: Vec<PathBuf>,
    /// Schemas read
    pub parsed_files: LinkedHashMap<PathBuf, FileDescriptor>,
}

/// Parse and validate input, and generate model schema
pub fn parse_and_typecheck(
    includes: &[PathBuf],
    input: &[PathBuf],
) -> io::Result<ParsedAndTypechecked> {
    let mut run = Run {
        parsed_files: LinkedHashMap::new(),
        includes,
    };

    let mut relative_paths = Vec::new();

    for input in input {
        println!("adding input file {}", input.display());
        relative_paths.push(run.add_fs_file(input)?);
    }

    Ok(ParsedAndTypechecked {
        relative_paths,
        parsed_files: run.parsed_files,
    })
}

/// A field occurrence: how any times field may appear
/// moved from model since it's just for parsing now
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Occurrence {
    /// A well-formed message can have zero or one of this field (but not more than one).
    Optional,
    /// This field can be repeated any number of times (including zero) in a well-formed message.
    /// The order of the repeated values will be preserved.
    Repeated,
    /// A well-formed message must have exactly one of this field.
    Required,
}

#[cfg(test)]
mod tests {
    use crate::{model::FileDescriptor, parser::Parser};
    use frodobuf_schema::model::{Constant, Field, FieldType, HasAttributes, Message};

    fn parse(input: &str) -> FileDescriptor {
        let mut parser = Parser::new(input);
        let r = parser
            .next_proto()
            .expect(&format!("parse failed at {}", parser.tokenizer.loc()));
        let eof = parser
            .tokenizer
            .syntax_eof()
            .expect(&format!("check eof failed at {}", parser.tokenizer.loc()));
        assert!(eof, "{}", parser.tokenizer.loc());
        r
    }

    #[test]
    fn simple_message() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
                string b = 1;
        }
        "#;
        let parsed = parse(proto);
        assert_eq!(parsed.schema.messages.len(), 1);
        Ok(())
    }

    #[test]
    fn nested_message() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A
        {
            message B {
                repeated int32 a = 1;
                optional string b = 2;
            }
            optional string b = 1;
        }"#;
        let parsed = parse(proto);
        assert_eq!(parsed.schema.messages.len(), 1);
        Ok(())
    }

    // returns the nth field in the message
    fn get_field(message: &Message, n: usize) -> &Field {
        let fields = &message.fields;
        &fields.get(n).unwrap()
    }

    #[test]
    fn data_types() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
            int32 a = 1;
            uint32 b = 2;
            int64 c = 3;
            uint64 d = 4;
            int8 e = 5;
            uint8 f = 6;
            float32 g = 7;
            float64 h = 8;
            float i = 9; // alias for float32
            double j = 10; // alias for float64
            bool k = 11;
            string x = 12;
            bytes bb = 13;
            map<uint32,bytes> y = 14;
            [bool] ff = 15;
        }
        "#;
        let parsed = parse(proto);
        //println!("A: {:#?}", &parsed);
        let message = &parsed.schema.messages.get(0).unwrap();

        assert_eq!(message.fields[0].typ, FieldType::Int32);
        assert_eq!(message.fields[1].typ, FieldType::Uint32);
        assert_eq!(message.fields[2].typ, FieldType::Int64);
        assert_eq!(message.fields[3].typ, FieldType::Uint64);
        assert_eq!(message.fields[4].typ, FieldType::Int8);
        assert_eq!(message.fields[5].typ, FieldType::Uint8);
        assert_eq!(message.fields[6].typ, FieldType::Float32);
        assert_eq!(message.fields[7].typ, FieldType::Float64);
        assert_eq!(message.fields[8].typ, FieldType::Float32);
        assert_eq!(message.fields[9].typ, FieldType::Float64);
        assert_eq!(message.fields[10].typ, FieldType::Bool);
        assert_eq!(message.fields[11].typ, FieldType::String);
        assert_eq!(message.fields[12].typ, FieldType::Bytes);

        if let FieldType::Map(b) = &get_field(message, 13).typ {
            assert_eq!(b.as_ref(), &(FieldType::Uint32, FieldType::Bytes));
        } else {
            assert!(false, "not a map");
        }

        if let FieldType::Array(a) = &get_field(message, 14).typ {
            assert_eq!(a.as_ref(), &FieldType::Bool);
        } else {
            assert!(false, "not an array");
        }

        Ok(())
    }

    #[test]
    fn proto_options() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        option proto_foo = 99;
        message A {
                string b = 1;
        }
        "#;
        let parsed = parse(proto);
        //println!("A: {:#?}", &parsed);
        assert_eq!(
            parsed.schema.attributes.len(),
            1usize,
            "proto options count"
        );
        let opt = parsed.schema.attributes.get(0).unwrap();
        assert_eq!(opt.values.len(), 1usize, "proto options kv count");
        let kv = opt.values.get(0).unwrap();
        assert_eq!(kv.0, "proto_foo");
        assert_eq!(kv.1, Constant::U64(99));
        Ok(())
    }

    #[test]
    fn message_field() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
                string b = 1;
        }
        "#;
        let parsed = parse(proto);

        let message = &parsed.schema.messages[0];
        let field = &message.fields[0];
        assert_eq!(
            (&field.typ, field.name.as_str(), field.number),
            (&FieldType::String, "b", 1)
        );

        Ok(())
    }

    #[test]
    fn field_attribute() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
                @msg_foo(value = 99);
                string b = 1;
        }
        "#;
        let parsed = parse(proto);

        let message = &parsed.schema.messages[0];
        let field = &message.fields[0];

        let attr = field.get_attribute("msg_foo").unwrap();
        assert_eq!(attr.values.len(), 1usize, "one value");
        assert_eq!(attr.values[0], ("value".to_string(), Constant::U64(99)));

        Ok(())
    }

    #[test]
    fn field_attr_ident() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
                @a1
                string b = 1;
        }
        "#;
        let parsed = parse(proto);
        let msg = &parsed.schema.messages[0];
        let field = &msg.fields[0];

        let attr = field.get_attribute("a1").unwrap();
        assert!(attr.values.is_empty());
        Ok(())
    }

    #[test]
    fn message_attr_ident() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        @a1
        message A {
                string b = 1;
        }
        "#;
        let parsed = parse(proto);
        let msg = &parsed.schema.messages[0];

        let attr = msg.get_attribute("a1").unwrap();
        assert!(attr.values.is_empty());
        Ok(())
    }

    #[test]
    fn message_attr_values() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        @a1(k1,k2=100)
        message A {
                string b = 1;
        }
        "#;
        let parsed = parse(proto);

        let msg = &parsed.schema.messages[0];

        let attr = msg.get_attribute("a1").unwrap();
        assert_eq!(attr.values.len(), 2usize);
        assert_eq!(attr.values[0], ("k1".to_string(), Constant::Bool(true)));
        assert_eq!(attr.values[1], ("k2".to_string(), Constant::U64(100)));

        Ok(())
    }

    #[test]
    fn optional_field() -> Result<(), Box<dyn std::error::Error>> {
        let proto = r#"package t;
        message A {
                string b? = 1;
                optional string y = 2;
                string x = 3;
                required string z = 4;
        }
        "#;
        let parsed = parse(proto);
        let msg = &parsed.schema.messages.get(0).unwrap();
        // optional
        for i in 0..=1 {
            assert!(msg.fields.get(i).unwrap().optional);
        }
        // required
        for i in 2..=3 {
            assert!(!msg.fields.get(i).unwrap().optional);
        }
        Ok(())
    }
}
