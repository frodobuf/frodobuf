use std::collections::HashMap;
use std::str;

use crate::fmt;
use crate::lexer::{
    int, LexerError, NumLit, ParserLanguage, StrLitDecodeError, Token, Tokenizer, TokenizerError,
};
use crate::model::{FileDescriptor, Import, ImportVis};
//use crate::ProtobufIdent;
use frodobuf_schema::model::{
    Attribute, Constant, EnumValue, Enumeration, Field, FieldType, Ident, Message, Method, Schema,
    Service, ATTRIBUTE_ID_OPTION, ATTRIBUTE_UNNAMED,
};
use sha2::Digest;

type SchemaHash = sha2::digest::Output<sha2::Sha256>;

const SYM_LCURLY: char = '{';
const SYM_RCURLY: char = '}';
const SYM_SEMICOLON: char = ';';
const SYM_LPAREN: char = '(';
const SYM_RPAREN: char = ')';
const SYM_EQUALS: char = '=';
const SYM_PERIOD: char = '.';
const SYM_COMMA: char = ',';
const SYM_LT: char = '<';
const SYM_GT: char = '>';

/// Basic information about parsing error.
#[derive(Debug)]
pub enum ParserError {
    TokenizerError(TokenizerError),
    IncorrectInput,
    NotUtf8,
    ExpectConstant,
    UnknownSyntax,
    IntegerOverflow,
    LabelNotAllowed,
    LabelRequired,
    GroupNameShouldStartWithUpperCase,
    StrLitDecodeError(StrLitDecodeError),
    LexerError(LexerError),
    MapKeyType,
    RepeatedArray,
    DanglingAttributes,
    DuplicateFieldNumber(u32),
    MissingPackage,
    Serialization(String),
    InternalHash(String),
    OnlyOnePackage,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::TokenizerError(e) => write!(f, "{}", e),
            ParserError::IncorrectInput => write!(f, "incorrect input"),
            ParserError::NotUtf8 => write!(f, "not UTF-8"),
            ParserError::ExpectConstant => write!(f, "expecting a constant"),
            ParserError::UnknownSyntax => write!(f, "unknown syntax"),
            ParserError::IntegerOverflow => write!(f, "integer overflow"),
            ParserError::LabelNotAllowed => write!(f, "label not allowed"),
            ParserError::LabelRequired => write!(f, "label required"),
            ParserError::RepeatedArray => write!(f, "use 'repeated' or array[], but not both"),
            ParserError::MapKeyType => write!(
                f,
                "unsupported map key type: must be an integer type or string"
            ),
            ParserError::GroupNameShouldStartWithUpperCase => {
                write!(f, "group name should start with upper case")
            }
            ParserError::StrLitDecodeError(e) => write!(f, "string literal decode error: {}", e),
            ParserError::LexerError(e) => write!(f, "lexer error: {}", e),
            ParserError::DanglingAttributes => write!(
                f,
                "'@' attributes defined without applicable type or service"
            ),
            ParserError::DuplicateFieldNumber(n) => write!(f, "duplicate field number ({})", n),
            ParserError::MissingPackage => write!(f, "missing required 'package' statement"),
            ParserError::InternalHash(s) => write!(f, "internal hash error {}", s),
            ParserError::Serialization(s) => write!(f, "serialization error: {}", s),
            ParserError::OnlyOnePackage => {
                write!(f, "Only one package declaration is allowed per midl file")
            }
        }
    }
}

impl From<TokenizerError> for ParserError {
    fn from(e: TokenizerError) -> Self {
        ParserError::TokenizerError(e)
    }
}

impl From<serde_json::Error> for ParserError {
    fn from(e: serde_json::Error) -> Self {
        ParserError::Serialization(e.to_string())
    }
}

impl From<StrLitDecodeError> for ParserError {
    fn from(e: StrLitDecodeError) -> Self {
        ParserError::StrLitDecodeError(e)
    }
}

impl From<LexerError> for ParserError {
    fn from(e: LexerError) -> Self {
        ParserError::LexerError(e)
    }
}

impl From<int::Overflow> for ParserError {
    fn from(_: int::Overflow) -> Self {
        ParserError::IntegerOverflow
    }
}

#[derive(Debug)]
pub struct ParserErrorWithLocation {
    pub error: ParserError,
    /// 1-based
    pub line: u32,
    /// 1-based
    pub col: u32,
}

impl fmt::Display for ParserErrorWithLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}:{}: {}", self.line, self.col, self.error)
    }
}

impl std::error::Error for ParserErrorWithLocation {}

pub type ParserResult<T> = Result<T, ParserError>;

fn attrib_for_loc(loc: &crate::lexer::Loc) -> Attribute {
    Attribute {
        key: Ident::from_namespace(None, "_source".to_string()),
        values: vec![
            ("line".to_string(), Constant::U64(loc.line as u64)),
            ("col".to_string(), Constant::U64(loc.col as u64)),
        ],
    }
}

trait ToU8 {
    fn to_u8(&self) -> ParserResult<u8>;
}

trait ToI32 {
    fn to_i32(&self) -> ParserResult<i32>;
}

trait ToI64 {
    fn to_i64(&self) -> ParserResult<i64>;
}

trait ToChar {
    fn to_char(&self) -> ParserResult<char>;
}

impl ToI32 for u64 {
    fn to_i32(&self) -> ParserResult<i32> {
        if *self <= i32::max_value() as u64 {
            Ok(*self as i32)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToI32 for i64 {
    fn to_i32(&self) -> ParserResult<i32> {
        if *self <= i32::max_value() as i64 && *self >= i32::min_value() as i64 {
            Ok(*self as i32)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToI64 for u64 {
    fn to_i64(&self) -> Result<i64, ParserError> {
        if *self <= i64::max_value() as u64 {
            Ok(*self as i64)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToChar for u8 {
    fn to_char(&self) -> Result<char, ParserError> {
        if *self <= 0x7f {
            Ok(*self as char)
        } else {
            Err(ParserError::NotUtf8)
        }
    }
}

impl ToU8 for u32 {
    fn to_u8(&self) -> Result<u8, ParserError> {
        if *self as u8 as u32 == *self {
            Ok(*self as u8)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

/// Parse file into schema.
/// Does not import any of the 'imports' or resolve foreign references
pub fn parse_string(text: &str) -> Result<FileDescriptor, ParserErrorWithLocation> {
    let mut parser = Parser::new(&text);
    match parser.next_proto() {
        Ok(r) => Ok(r),
        Err(error) => {
            let crate::lexer::Loc { line, col } = parser.tokenizer.loc();
            Err(ParserErrorWithLocation { error, line, col })
        }
    }
}

#[derive(Clone)]
pub(crate) struct Parser<'a> {
    pub tokenizer: Tokenizer<'a>,
}

trait NumLitEx {
    fn to_option_value(&self, sign_is_plus: bool) -> ParserResult<Constant>;
}

impl NumLitEx for NumLit {
    fn to_option_value(&self, sign_is_plus: bool) -> ParserResult<Constant> {
        Ok(match (*self, sign_is_plus) {
            (NumLit::U64(u), true) => Constant::U64(u),
            (NumLit::F64(f), true) => Constant::F64(f),
            (NumLit::U64(u), false) => Constant::I64(int::neg(u)?),
            (NumLit::F64(f), false) => Constant::F64(-f),
        })
    }
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Parser<'a> {
        Parser {
            tokenizer: Tokenizer::new(input, ParserLanguage::Proto),
        }
    }

    // Protobuf grammar

    // fullIdent = ident { "." ident }*
    // Also accepts '::' as separator
    fn next_full_ident(&mut self) -> ParserResult<Ident> {
        let mut full_ident = self.tokenizer.next_ident()?;
        let has_path_separator = |t: &Token| {
            if matches!(t, Token::DoubleColon | Token::Symbol(SYM_PERIOD)) {
                Some(Token::Symbol(SYM_PERIOD))
            } else {
                None
            }
        };
        loop {
            if self
                .tokenizer
                .next_token_if_map(has_path_separator)?
                .is_some()
            {
                full_ident.push_str(frodobuf_schema::model::IDENT_PATH_DELIMITER);
            } else {
                break;
            }
            full_ident.push_str(&self.tokenizer.next_ident()?);
        }
        Ok(full_ident.into())
    }

    // emptyStatement = ";"
    fn next_empty_statement_opt(&mut self) -> ParserResult<Option<()>> {
        if self.tokenizer.next_symbol_if_eq(SYM_SEMICOLON)? {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    // Boolean

    // boolLit = "true" | "false"
    fn next_bool_lit_opt(&mut self) -> ParserResult<Option<bool>> {
        Ok(if self.tokenizer.next_ident_if_eq("true")? {
            Some(true)
        } else if self.tokenizer.next_ident_if_eq("false")? {
            Some(false)
        } else {
            None
        })
    }

    // Constant

    fn next_num_lit(&mut self) -> ParserResult<NumLit> {
        self.tokenizer
            .next_token_check_map(|token| Ok(token.to_num_lit()?))
    }

    // lit =  ( [ "-" | "+" ] intLit ) | ( [ "-" | "+" ] floatLit ) |
    //            strLit | boolLit
    fn next_lit_opt(&mut self) -> ParserResult<Option<Constant>> {
        if let Some(b) = self.next_bool_lit_opt()? {
            return Ok(Some(Constant::Bool(b)));
        }

        if let Token::Symbol(c) = *(self.tokenizer.lookahead_some()?) {
            if c == '+' || c == '-' {
                self.tokenizer.advance()?;
                let sign = c == '+';
                return self.next_num_lit()?.to_option_value(sign).map(Some);
            }
        }

        if let Some(r) = self.tokenizer.next_token_if_map(|token| match *token {
            Token::StrLit(ref s) => Some(Constant::String(s.to_string())),
            _ => None,
        })? {
            return Ok(Some(r));
        }

        if matches!(
            self.tokenizer.lookahead_some()?,
            &Token::IntLit(..) | &Token::FloatLit(..)
        ) {
            return self.next_num_lit()?.to_option_value(true).map(Some);
        }
        Ok(None)
    }

    // constant = fullIdent | ( [ "-" | "+" ] intLit ) | ( [ "-" | "+" ] floatLit ) |
    //            strLit | boolLit
    fn next_constant(&mut self) -> ParserResult<Constant> {
        if let Some(lit) = self.next_lit_opt()? {
            return Ok(lit);
        }
        // We could just call next_full_ident here, but if it's not an identifier,
        // ExpectConstant is a better error to return than ExpectIdent
        if matches!(self.tokenizer.lookahead_some()?, &Token::Ident(..)) {
            return Ok(Constant::Ident(self.next_full_ident()?));
        }
        Err(ParserError::ExpectConstant)
    }

    fn next_int_lit(&mut self) -> ParserResult<u64> {
        self.tokenizer.next_token_check_map(|token| match *token {
            Token::IntLit(i) => Ok(i),
            _ => Err(ParserError::IncorrectInput),
        })
    }

    // Import Statement

    // import = "import" [ "weak" | "public" ] strLit ";"
    fn next_import_opt(&mut self) -> ParserResult<Option<Import>> {
        if self.tokenizer.next_ident_if_eq("import")? {
            let vis = if self.tokenizer.next_ident_if_eq("weak")? {
                ImportVis::Weak
            } else if self.tokenizer.next_ident_if_eq("public")? {
                ImportVis::Public
            } else {
                ImportVis::Default
            };
            let path = self.tokenizer.next_str_lit()?.decode_utf8()?;
            self.tokenizer.next_symbol_expect_eq(SYM_SEMICOLON)?;
            Ok(Some(Import { path, vis }))
        } else {
            Ok(None)
        }
    }

    // Package

    // package = "package" fullIdent ";"
    fn next_package_opt(&mut self) -> ParserResult<Option<Ident>> {
        if self.tokenizer.next_ident_if_eq("package")? {
            let package = self.next_full_ident()?;
            self.tokenizer.next_symbol_expect_eq(SYM_SEMICOLON)?;
            Ok(Some(package))
        } else {
            Ok(None)
        }
    }

    // @attrib
    // @attrib( name [=value], ... )
    // @attrib(=value ) (anonymous value)

    // trailing comma is ok
    fn next_attribute_opt(&mut self) -> ParserResult<Option<Attribute>> {
        if self.tokenizer.next_symbol_if_eq('@')? {
            let key = self.next_full_ident()?;
            let mut values = Vec::new();
            if self.tokenizer.next_symbol_if_eq(SYM_LPAREN)? {
                loop {
                    if self.tokenizer.next_symbol_if_eq(SYM_RPAREN)? {
                        break;
                    }
                    // lit (anon const) or  'name=value' or 'name'
                    if let Some(lit) = self.next_lit_opt()? {
                        // anon const , e.g., @doc("hear ye")
                        values.push((ATTRIBUTE_UNNAMED.to_string(), lit));
                        // optional comma
                        let _ = self.tokenizer.next_symbol_if_eq(SYM_COMMA)?;
                        continue;
                    }
                    // 'name=value' or 'name'
                    let opt_name = self.tokenizer.next_ident()?;
                    if self.tokenizer.next_symbol_if_eq(SYM_COMMA)? {
                        let opt_value = Constant::Bool(true);
                        values.push((opt_name, opt_value));
                        continue;
                    }
                    if self.tokenizer.next_symbol_if_eq(SYM_RPAREN)? {
                        let opt_value = Constant::Bool(true);
                        values.push((opt_name, opt_value));
                        break;
                    }
                    self.tokenizer.next_symbol_expect_eq(SYM_EQUALS)?;
                    let value = self.next_constant()?;
                    values.push((opt_name, value));
                }
            }
            // optionally followed by ';'
            let _ = self.tokenizer.next_symbol_if_eq(SYM_SEMICOLON);
            Ok(Some(Attribute { key, values }))
        } else {
            Ok(None)
        }
    }

    // option = "option" optionName  "=" constant ";"
    // encode as attribute: "@(optionName = constant)"
    fn next_option_opt(&mut self) -> ParserResult<Option<Attribute>> {
        if self.tokenizer.next_ident_if_eq("option")? {
            let name = self.next_full_ident()?;
            self.tokenizer.next_symbol_expect_eq(SYM_EQUALS)?;
            let value = self.next_constant()?;
            self.tokenizer.next_symbol_expect_eq(SYM_SEMICOLON)?;
            Ok(Some(Attribute {
                key: Ident::from_namespace(None, ATTRIBUTE_ID_OPTION.into()),
                values: vec![(name.to_string(), value)],
            }))
        } else {
            Ok(None)
        }
    }

    // Fields

    // label = "required" | "optional" | "repeated"
    fn next_label(&mut self) -> ParserResult<Option<Occurrence>> {
        let map = &[
            ("optional", Occurrence::Optional),
            ("required", Occurrence::Required),
            ("repeated", Occurrence::Repeated),
        ];
        for (name, value) in map.iter() {
            let mut clone = self.clone();
            if clone.tokenizer.next_ident_if_eq(name)? {
                *self = clone;
                return Ok(Some(value.clone()));
            }
        }
        Ok(None)
    }

    fn next_field_type(&mut self) -> ParserResult<FieldType> {
        let simple = &[
            ("int32", FieldType::Int32),
            ("int64", FieldType::Int64),
            ("uint32", FieldType::Uint32),
            ("uint64", FieldType::Uint64),
            ("int8", FieldType::Int8),
            ("uint8", FieldType::Uint8),
            ("bool", FieldType::Bool),
            ("string", FieldType::String),
            ("bytes", FieldType::Bytes),
            ("float", FieldType::Float32), // alias for float32
            ("float32", FieldType::Float32),
            ("float64", FieldType::Float64),
            ("double", FieldType::Float64), // alias for float64
        ];

        for &(ref n, ref t) in simple {
            if self.tokenizer.next_ident_if_eq(n)? {
                return Ok(t.clone());
            }
        }

        if let Some(t) = self.next_map_field_type_opt()? {
            return Ok(t);
        }

        if let Some(t) = self.next_array_field_type_opt()? {
            return Ok(t);
        }

        Ok(FieldType::ObjectOrEnum(self.next_full_ident()?))
    }

    fn next_field_number(&mut self) -> ParserResult<u32> {
        // TODO: not all integers are valid field numbers
        self.tokenizer.next_token_check_map(|token| match *token {
            Token::IntLit(i) => Ok(i as u32),
            _ => Err(ParserError::IncorrectInput),
        })
    }

    // field = label type fieldName "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    fn next_field(&mut self) -> ParserResult<Field> {
        let loc = self.tokenizer.lookahead_loc();
        let attributes = vec![attrib_for_loc(&loc)];
        let occurrence = self.next_label()?;
        let typ = {
            match (&occurrence, self.next_field_type()?) {
                (Some(Occurrence::Repeated), FieldType::Array(_)) => {
                    return Err(ParserError::RepeatedArray);
                }
                // turn 'repeated' into array
                (Some(Occurrence::Repeated), typ) => FieldType::Array(Box::new(typ)),
                (_, typ) => typ,
            }
        };
        let name = self.tokenizer.next_ident()?;
        let optional = self.tokenizer.next_symbol_if_eq('?')?
            || matches!(occurrence, Some(Occurrence::Optional));

        // unlike protobuf, "= num"  is optional; default to zero
        // if zero, will be replaced in message body as 1-based sequence number
        let number = if self.tokenizer.next_symbol_if_eq(SYM_EQUALS)? {
            self.next_field_number()?
        } else {
            0
        };
        // must terminate with ';'
        self.tokenizer.next_symbol_expect_eq(SYM_SEMICOLON)?;
        let field = Field {
            name,
            optional,
            typ,
            number,
            attributes,
        };
        Ok(field)
    }

    // mapField = "map" "<" keyType "," type ">" mapName "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    // keyType = "int8" | "int32" | "int64" | "uint8" | "uint32" | "uint64" | "string"
    fn next_map_field_type_opt(&mut self) -> ParserResult<Option<FieldType>> {
        if self.tokenizer.next_ident_if_eq("map")? {
            self.tokenizer.next_symbol_expect_eq(SYM_LT)?;
            let key = self.next_field_type()?;
            if !key.is_integer() && !matches!(&key, FieldType::String) {
                return Err(ParserError::MapKeyType);
            }
            self.tokenizer.next_symbol_expect_eq(SYM_COMMA)?;
            let value = self.next_field_type()?;
            self.tokenizer.next_symbol_expect_eq(SYM_GT)?;
            Ok(Some(FieldType::Map(Box::new((key, value)))))
        } else {
            Ok(None)
        }
    }

    // arrayField = "[" keyType  "]" ident "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    // keyType = any
    fn next_array_field_type_opt(&mut self) -> ParserResult<Option<FieldType>> {
        if self.tokenizer.next_symbol_if_eq('[')? {
            let item_type = self.next_field_type()?;
            self.tokenizer.next_symbol_expect_eq(']')?;
            Ok(Some(FieldType::Array(Box::new(item_type))))
        } else {
            Ok(None)
        }
    }

    // Top Level definitions

    // Enum definition

    // https://github.com/google/protobuf/issues/4561
    fn next_enum_value(&mut self) -> ParserResult<i32> {
        let minus = self.tokenizer.next_symbol_if_eq('-')?;
        let lit = self.next_int_lit()?;
        Ok(if minus {
            let unsigned = lit.to_i64()?;
            match unsigned.checked_neg() {
                Some(neg) => neg.to_i32()?,
                None => return Err(ParserError::IntegerOverflow),
            }
        } else {
            lit.to_i32()?
        })
    }

    // enumField = ident "=" intLit [ "[" enumValueOption { ","  enumValueOption } "]" ]";"
    fn next_enum_field(&mut self) -> ParserResult<EnumValue> {
        let name = self.tokenizer.next_ident()?;
        self.tokenizer.next_symbol_expect_eq(SYM_EQUALS)?;
        let number = self.next_enum_value()?;
        Ok(EnumValue {
            name,
            number,
            attributes: vec![],
        })
    }

    // enum = "enum" enumName enumBody
    // enumBody = "{" { option | enumField | emptyStatement } "}"
    fn next_enum_opt(&mut self) -> ParserResult<Option<Enumeration>> {
        if self.tokenizer.next_ident_if_eq("enum")? {
            let name = self.tokenizer.next_ident()?;

            let mut values = Vec::new();
            let mut attributes = Vec::new();

            self.tokenizer.next_symbol_expect_eq(SYM_LCURLY)?;
            while self.tokenizer.lookahead_if_symbol()? != Some(SYM_RCURLY) {
                // emptyStatement
                if self.tokenizer.next_symbol_if_eq(SYM_SEMICOLON)? {
                    continue;
                }

                // collection 'option's, append to attributes of enum
                if let Some(attr) = self.next_option_opt()? {
                    attributes.push(attr);
                    continue;
                }

                values.push(self.next_enum_field()?);
            }
            self.tokenizer.next_symbol_expect_eq(SYM_RCURLY)?;
            Ok(Some(Enumeration {
                name,
                values,
                attributes,
            }))
        } else {
            Ok(None)
        }
    }

    // Message definition
    // messageBody = "{" { field | enum | message |
    //               option | mapField | reserved | emptyStatement } "}"
    fn next_message_body(&mut self) -> ParserResult<Message> {
        let loc = self.tokenizer.lookahead_loc();
        self.tokenizer.next_symbol_expect_eq(SYM_LCURLY)?;

        let dup_check: HashMap<u32, bool> = HashMap::new();
        let mut message = Message::default();
        message.attributes.push(attrib_for_loc(&loc));
        // buffer for attributes for members of this message
        let mut item_attributes = Vec::new();

        while self.tokenizer.lookahead_if_symbol()? != Some(SYM_RCURLY) {
            //let loc = self.tokenizer.lookahead_loc();

            // emptyStatement
            if self.tokenizer.next_symbol_if_eq(SYM_SEMICOLON)? {
                continue;
            }

            if let Some(mut nested_message) = self.next_message_opt()? {
                nested_message.attributes.append(&mut item_attributes);
                message.messages.push(nested_message);
                continue;
            }

            if let Some(mut nested_enum) = self.next_enum_opt()? {
                nested_enum.attributes.append(&mut item_attributes);
                message.enums.push(nested_enum);
                continue;
            }

            if let Some(option) = self.next_option_opt()? {
                message.attributes.push(option);
                continue;
            }

            if let Some(attr) = self.next_attribute_opt()? {
                item_attributes.push(attr);
                continue;
            }
            let mut field = self.next_field()?;
            field.attributes.append(&mut item_attributes);
            if field.number == 0 {
                field.number = message.fields.len() as u32 + 1;
            }
            if dup_check.contains_key(&field.number) {
                return Err(ParserError::DuplicateFieldNumber(field.number));
            }
            message.fields.push(field);
        }

        if !item_attributes.is_empty() {
            return Err(ParserError::DanglingAttributes);
        }
        self.tokenizer.next_symbol_expect_eq(SYM_RCURLY)?;

        Ok(message)
    }

    // message = "message" messageName messageBody
    fn next_message_opt(&mut self) -> ParserResult<Option<Message>> {
        //let loc = self.tokenizer.lookahead_loc();

        if self.tokenizer.next_ident_if_eq("message")? {
            let name = Ident::from_namespace(None, self.tokenizer.next_ident()?);
            let mut message = self.next_message_body()?;
            message.name = name;
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    // Service definition

    // rpc = "rpc" rpcName "(" messageType ")"
    //     "returns" "(" messageType ")"
    //     (( "{" { option | emptyStatement } "}" ) | ";" )
    fn next_rpc_opt(&mut self) -> ParserResult<Option<Method>> {
        let has_fn_returns = |t: &Token| {
            if matches!(t, Token::FnReturns) {
                Some(Token::FnReturns)
            } else {
                None
            }
        };

        if self.tokenizer.next_ident_if_eq("rpc")? {
            let name = self.tokenizer.next_ident()?;

            self.tokenizer.next_symbol_expect_eq(SYM_LPAREN)?;

            let input_type = if self.tokenizer.next_symbol_if_eq(SYM_RPAREN)? {
                // empty args
                None
            } else {
                // non-empty args
                let arg = self.next_field_type()?;
                self.tokenizer.next_symbol_expect_eq(SYM_RPAREN)?;
                Some(arg)
            };
            // Return type
            // if "->" or "returns", get the return type as () or a data type
            // if omitted (method definition ends with ;), return type is also void (None)
            let output_type = if self.tokenizer.next_token_if_map(has_fn_returns)?.is_some()
                || self.tokenizer.next_ident_if_eq("returns")?
            {
                if self.tokenizer.next_symbol_if_eq(SYM_LPAREN)? {
                    self.tokenizer.next_symbol_expect_eq(SYM_RPAREN)?;
                    None
                } else {
                    Some(self.next_field_type()?)
                }
            } else {
                None
            };

            // require semicolon to terminate method definition
            self.tokenizer.next_symbol_expect_eq(SYM_SEMICOLON)?;

            Ok(Some(Method {
                name,
                input_type,
                output_type,
                attributes: Vec::new(),
            }))
        } else {
            Ok(None)
        }
    }

    // proto2:
    // service = "service" serviceName "{" { option | fn | stream | emptyStatement } "}"
    //
    // proto3:
    // service = "service" serviceName "{" { option | fn | emptyStatement } "}"
    fn next_service_opt(&mut self) -> ParserResult<Option<Service>> {
        let loc = self.tokenizer.lookahead_loc();

        if self.tokenizer.next_ident_if_eq("service")? {
            let name = Ident {
                namespace: None,
                name: self.tokenizer.next_ident()?,
            };
            let mut methods = Vec::new();
            let attributes = vec![attrib_for_loc(&loc)];

            let mut item_attributes = Vec::new();
            self.tokenizer.next_symbol_expect_eq(SYM_LCURLY)?;
            while self.tokenizer.lookahead_if_symbol()? != Some(SYM_RCURLY) {
                if let Some(mut method) = self.next_rpc_opt()? {
                    method.attributes.append(&mut item_attributes);
                    methods.push(method);
                    continue;
                }

                if let Some(a) = self.next_attribute_opt()? {
                    item_attributes.push(a);
                    continue;
                }

                if let Some(()) = self.next_empty_statement_opt()? {
                    continue;
                }

                return Err(ParserError::IncorrectInput);
            }
            if !item_attributes.is_empty() {
                return Err(ParserError::DanglingAttributes);
            }
            self.tokenizer.next_symbol_expect_eq(SYM_RCURLY)?;
            Ok(Some(Service {
                name,
                methods,
                attributes,
                ..Default::default()
            }))
        } else {
            Ok(None)
        }
    }

    // Proto file

    // proto = syntax { import | package | option | topLevelDef | emptyStatement }
    // topLevelDef = message | enum | service
    pub fn next_proto(&mut self) -> ParserResult<FileDescriptor> {
        let mut imports = Vec::new();
        let mut package = None;
        let mut messages = Vec::new();
        let mut enums = Vec::new();
        let mut file_attributes = Vec::new();
        let mut services = Vec::new();

        // buffer attributes until we know what they apply to (message, enum, or service)
        let mut inner_attributes = Vec::new();

        while !self.tokenizer.syntax_eof()? {
            if let Some(import) = self.next_import_opt()? {
                if !inner_attributes.is_empty() {
                    return Err(ParserError::DanglingAttributes);
                }
                imports.push(import);
                continue;
            }

            if let Some(next_package) = self.next_package_opt()? {
                if package.is_some() {
                    return Err(ParserError::OnlyOnePackage);
                }
                if !inner_attributes.is_empty() {
                    return Err(ParserError::DanglingAttributes);
                }
                package = Some(next_package);
                continue;
            }

            if let Some(attrib) = self.next_attribute_opt()? {
                inner_attributes.push(attrib);
                continue;
            }

            if let Some(option) = self.next_option_opt()? {
                // can't mix @attribute and option
                // TODO: do we need this restriction?
                if !inner_attributes.is_empty() {
                    return Err(ParserError::DanglingAttributes);
                }
                file_attributes.push(option);
                continue;
            }

            if let Some(mut message) = self.next_message_opt()? {
                message.attributes.append(&mut inner_attributes);
                messages.push(message);
                continue;
            }

            if let Some(mut enumeration) = self.next_enum_opt()? {
                enumeration.attributes.append(&mut inner_attributes);
                enums.push(enumeration);
                continue;
            }

            if let Some(mut service) = self.next_service_opt()? {
                service.attributes.append(&mut inner_attributes);
                services.push(service);
                continue;
            }

            if self.tokenizer.next_symbol_if_eq(SYM_SEMICOLON)? {
                if !inner_attributes.is_empty() {
                    return Err(ParserError::DanglingAttributes);
                }
                continue;
            }

            return Err(ParserError::IncorrectInput);
        }
        if !inner_attributes.is_empty() {
            return Err(ParserError::DanglingAttributes);
        }
        let namespace = match package {
            Some(ns) => ns,
            None => return Err(ParserError::MissingPackage),
        };

        let mut schema = Schema {
            namespace,
            messages,
            enums,
            attributes: file_attributes,
            ..Default::default()
        };
        // compute hash of everything except services:
        // - schema namespace, all custom data types, and attributes
        let base_hash = sha2_hash(vec![&serde_json::to_vec(&schema)?]);
        let b64_config = base64::Config::new(base64::CharacterSet::Standard, false);
        // for each service, hash serialized service + base hash because services depend on types
        // but services don't depend on each other, so they each have a separate signature
        for mut service in services.iter_mut() {
            let serialized = serde_json::to_vec(&service)?;
            let hash = sha2_hash(vec![&serialized, &base_hash]);
            service.schema_id = Some(base64::encode_config(&hash, b64_config));
            service.schema = Some(base64::encode_config(&serialized, b64_config));
        }
        schema.services = services;

        // add parser version
        schema.attributes.push(Attribute {
            key: Ident::from_namespace(None, "midl_parser_version".to_string()),
            values: vec![("_".to_string(), Constant::from(crate::MIDL_PARSER_VERSION))],
        });

        // services,
        Ok(FileDescriptor { imports, schema })
    }
}

/// Compute sha-256 hash of a byte vector. Result is a 32-byte value
fn sha2_hash(data: Vec<&[u8]>) -> SchemaHash {
    let mut hash = sha2::Sha256::new();
    for v in data.iter() {
        hash.update(v)
    }
    hash.finalize()
}

/// Occurrence is only used in parsing. In schema, these are translated to
///  Optional -> Optional
///  Required -> !Optional
///  Repeated -> Array<value>
#[derive(Debug, Clone)]
enum Occurrence {
    Optional,
    Required,
    Repeated,
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Parser) -> ParserResult<R>,
    {
        let mut parser = Parser::new(input);
        let r =
            parse_what(&mut parser).expect(&format!("parse failed at {}", parser.tokenizer.loc()));
        let eof = parser
            .tokenizer
            .syntax_eof()
            .expect(&format!("check eof failed at {}", parser.tokenizer.loc()));
        assert!(eof, "{}", parser.tokenizer.loc());
        r
    }

    fn parse_opt<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Parser) -> ParserResult<Option<R>>,
    {
        let mut parser = Parser::new(input);
        let o =
            parse_what(&mut parser).expect(&format!("parse failed at {}", parser.tokenizer.loc()));
        let r = o.expect(&format!(
            "parser returned none at {}",
            parser.tokenizer.loc()
        ));
        assert!(parser.tokenizer.syntax_eof().unwrap());
        r
    }

    #[test]
    fn test_message() {
        let msg = r#"
        message ReferenceData
    {
        repeated ScenarioInfo  scenarioSet = 1;
        repeated CalculatedObjectInfo calculatedObjectSet = 2;
        repeated RiskFactorList riskFactorListSet = 3;
        repeated RiskMaturityInfo riskMaturitySet = 4;
        repeated IndicatorInfo indicatorSet = 5;
        repeated RiskStrikeInfo riskStrikeSet = 6;
        repeated FreeProjectionList freeProjectionListSet = 7;
        repeated ValidationProperty ValidationSet = 8;
        repeated CalcProperties calcPropertiesSet = 9;
        repeated MaturityInfo maturitySet = 10;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(10, mess.fields.len());
    }

    #[test]
    fn test_enum() {
        let msg = r#"
        enum PairingStatus {
                DEALPAIRED        = 0;
                INVENTORYORPHAN   = 1;
                CALCULATEDORPHAN  = 2;
                CANCELED          = 3;
    }"#;

        let enumeration = parse_opt(msg, |p| p.next_enum_opt());
        assert_eq!(4, enumeration.values.len());
    }

    #[test]
    fn test_ignore() {
        let msg = r#"
        option optimize_for = SPEED;"#;

        parse_opt(msg, |p| p.next_option_opt());
    }

    #[test]
    fn test_import() {
        let msg = r#"package t;
    import "test_import_nested_imported_pb.proto";

    message ContainsImportedNested {
        ContainerForNested.NestedMessage m = 1;
        ContainerForNested.NestedEnum e = 2;
    }
    "#;
        let desc = parse(msg, |p| p.next_proto());

        assert_eq!(
            vec!["test_import_nested_imported_pb.proto"],
            desc.imports.into_iter().map(|i| i.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_nested_message() {
        let msg = r#"message A
    {
        message B {
            repeated int32 a = 1;
            optional string b = 2;
        }
        optional string b = 1;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(1, mess.messages.len());
    }

    #[test]
    fn test_map() {
        let msg = r#"
        message A
    {
        optional map<string, int32> b = 1;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(1, mess.fields.len());
        match mess.fields[0].typ {
            FieldType::Map(ref f) => match &**f {
                &(FieldType::String, FieldType::Int32) => (),
                ref f => panic!("Expecting Map<String, Int32> found {:?}", f),
            },
            ref f => panic!("Expecting map, got {:?}", f),
        }
    }

    #[test]
    fn test_default_value_false() {
        let msg = r#"message Sample {
            @default(value=false)
            bool x = 1;
        }"#;

        let msg = parse_opt(msg, |p| p.next_message_opt());
        let default_val = msg.fields[0].default_value();
        assert_eq!(default_val, Some(Constant::Bool(false)));
    }

    #[test]
    fn test_default_value_true() {
        let msg = r#"
        message Sample {
            @default(value=true)
            bool x = 1;
        }"#;

        let msg = parse_opt(msg, |p| p.next_message_opt());
        let default_val = msg.fields[0].default_value();
        assert_eq!(default_val, Some(Constant::Bool(true)));
    }

    #[test]
    fn test_default_value_int() {
        let msg = r#"message Sample {
            @default(value=17)
            int32 x = 1;
        }"#;

        let msg = parse_opt(msg, |p| p.next_message_opt());
        let default_val = msg.fields[0].default_value();
        assert_eq!(default_val, Some(Constant::U64(17)));
    }

    #[test]
    fn test_default_value_int_neg() {
        let msg = r#"message Sample {
            @default(value= -33)
            int32 x = 1;
        }"#;

        let msg = parse_opt(msg, |p| p.next_message_opt());
        let default_val = msg.fields[0].default_value();
        assert_eq!(default_val, Some(Constant::I64(-33)));
    }

    #[test]
    fn test_default_value_string() {
        let msg = r#"
        message Sample {
            @default(value = "ab\nc d\"g\'h\0\"z");
            optional string x = 1;
        }"#;

        let msg = parse_opt(msg, |p| p.next_message_opt());
        let default_val = msg.fields[0].default_value();
        assert_eq!(
            default_val,
            Some(Constant::String(r#""ab\nc d\"g\'h\0\"z""#.to_string()))
        );
    }

    #[test]
    fn test_incorrect_file_descriptor() {
        let msg = r#"message Foo {
            dfgdg
        }
        "#;

        let err = FileDescriptor::parse(msg).err().expect("err");
        assert_eq!(3, err.line);
    }
}
