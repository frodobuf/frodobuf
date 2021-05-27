//! A nom-based protobuf file parser
//!
//! This crate can be seen as a rust transcription of the
//! [descriptor.proto](https://github.com/google/protobuf/blob/master/src/google/protobuf/descriptor.proto) file

//use crate::lexer::float;
use crate::lexer::Loc;
//use crate::lexer::StrLit;
use frodobuf_schema::model::Schema;

use crate::parser::Parser;
pub use crate::parser::ParserError;
pub use crate::parser::ParserErrorWithLocation;

#[derive(Debug, Clone, PartialEq)]
pub struct WithLoc<T> {
    pub loc: Loc,
    pub t: T,
}

//impl<T> WithLoc<T> {
//    pub fn with_loc(loc: Loc) -> impl FnOnce(T) -> WithLoc<T> {
//        move |t| WithLoc { loc, t }
//    }
//}

/// Visibility of import statement
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ImportVis {
    Default,
    Public,
    Weak,
}

impl Default for ImportVis {
    fn default() -> Self {
        ImportVis::Default
    }
}

/// Import statement
#[derive(Debug, Default, Clone)]
pub struct Import {
    pub path: String,
    pub vis: ImportVis,
}

/// A File descriptor representing a whole .proto file
#[derive(Debug, Default, Clone)]
pub struct FileDescriptor {
    /// Imports
    pub imports: Vec<Import>,

    /// Schema
    pub schema: Schema,
}

impl FileDescriptor {
    /// Parses a .proto file content into a `FileDescriptor`
    pub fn parse<S: AsRef<str>>(file: S) -> Result<Self, ParserErrorWithLocation> {
        let mut parser = Parser::new(file.as_ref());
        match parser.next_proto() {
            Ok(r) => Ok(r),
            Err(error) => {
                let Loc { line, col } = parser.tokenizer.loc();
                Err(ParserErrorWithLocation { error, line, col })
            }
        }
    }
}
