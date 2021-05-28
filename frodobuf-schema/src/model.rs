//! Frodobuf Schema model
use serde::{Deserialize, Serialize};
use std::fmt;

/// Separator in hierarchical names, e.g., "package.service"
pub const IDENT_PATH_DELIMITER: &str = ".";
/// name for attributes that store source code location (file, col)
pub const ATTRIBUTE_ID_SOURCE: &str = "_source";
/// name for attribute that stores a line of documentation
pub const ATTRIBUTE_ID_DOC: &str = "doc";
/// name for attribute that was a protobuf "option"
pub const ATTRIBUTE_ID_OPTION: &str = "option";
/// name for anonymous/unnamed attribute (value only)
pub const ATTRIBUTE_UNNAMED: &str = "_";

/// iterator over a set of attributes, for a field, method, sevice, or schema
pub struct Attributes<'a> {
    base: std::slice::Iter<'a, Attribute>,
}

/// Implementation of iterator over attributes
impl<'a> Iterator for Attributes<'a> {
    type Item = &'a Attribute;
    fn next(&mut self) -> Option<&'a Attribute> {
        self.base.next()
    }
}

/// An attribute is a key, plus a set of (name,value) pairs
/// If value os omitted, the name is stored with a value of 'true'
///
/// Examples
///   @deprecated                         // attrib with key only
///   @precision(scale = 2, round="down") // eg, how to display with 2 digits after decimal point
///   @serialize(flatten)                 // shorthand for @serialize(flatten = true)
///
/// There is also (currently) a syntax that can take a literal value only
///   @doc("This is my method")
///   @max(95)
///   @min(5)
/// For these, the name is recorded as ATTRIBUTE_UNNAMED ("_")
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    /// key of the attribute, like a "namespace" for the name=value pairs
    pub key: Ident,
    /// set of name=value pairs associated with the key
    pub values: Vec<(String, Constant)>,
}

impl Attribute {
    /// Construct an attribute with a key only `@key`
    pub fn new_key_only(key: &str) -> Attribute {
        Attribute {
            key: Ident::from(key),
            values: Vec::new(),
        }
    }

    /// Construct an attribute with a single value `@key(value)`
    pub fn new_single_value<C: Into<Constant>>(key: &str, value: C) -> Attribute {
        Attribute {
            key: Ident::from(key),
            values: vec![("_".to_string(), value.into())],
        }
    }

    /// Construct an attribute with a single name=value pair `@key(name=value)`
    pub fn new_single_kv<S: Into<String>, C: Into<Constant>>(
        key: &str,
        name: S,
        value: C,
    ) -> Attribute {
        Attribute {
            key: Ident::from(key),
            values: vec![(name.into(), value.into())],
        }
    }

    /// Iterate through all attributes
    pub fn iter(&self) -> impl std::iter::Iterator<Item = &(String, Constant)> {
        self.values.iter()
    }

    /// Returns the first value for key 'name', or None if not found
    pub fn get(&self, name: &str) -> Option<&Constant> {
        self.iter()
            .find_map(|opt| if opt.0 == name { Some(&opt.1) } else { None })
    }
}

/// trait for schema items that have attributes
pub trait HasAttributes {
    /// returns an iterator over the item's attributes
    fn attributes(&'_ self) -> Attributes<'_>;

    /// Returns an attribute by name, or None if it is not found
    fn get_attribute(&self, key: &str) -> Option<&Attribute> {
        self.attributes().find(|a| a.key == key)
    }
}

/// Identifier
/// For services, there is a namespace path corresponding to a 'package' hierarchy
/// For messages, the 'name' is the message name
/// TODO: for message, should 'namespace' be the service/trait term, or the global service/trait path?
/// "Global" name is "namespace::name"
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ident {
    /// optional namespace for identifier, as a list of nested packages or modules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>, // '::' separated namespace path
    /// The "leaf" or simple name of the identifier
    pub name: String,
}

impl Ident {
    /// Construct an identifier with no namespace
    pub fn new(s: &str) -> Self {
        if let Some((ns, id)) = s.rsplit_once(IDENT_PATH_DELIMITER) {
            Ident {
                namespace: Some(ns.to_string()),
                name: id.to_string(),
            }
        } else {
            Ident {
                namespace: None,
                name: s.to_string(),
            }
        }
    }

    /// Construct Identifier with simple namespace
    pub fn from_namespace(namespace: Option<String>, name: String) -> Self {
        Ident { namespace, name }
    }
}

/// Conversion from &String to Ident
impl From<String> for Ident {
    fn from(s: String) -> Ident {
        Ident::new(&s)
    }
}

/// Conversion from &str to Ident
impl From<&str> for Ident {
    fn from(s: &str) -> Ident {
        Ident::new(s)
    }
}

/// Implement <Ident> == <String> comparisons
impl PartialEq<String> for Ident {
    fn eq(&self, other: &String) -> bool {
        &self.to_string() == other
    }
}

/// Implement <Ident> == <String> comparisons
impl PartialEq<Ident> for String {
    fn eq(&self, other: &Ident) -> bool {
        &other.to_string() == self
    }
}

/// Implement <Ident> == <String> comparisons
impl PartialEq<Ident> for &str {
    fn eq(&self, other: &Ident) -> bool {
        &other.to_string() == self
    }
}

/// Implement <Ident> == <&str> comparisons
impl PartialEq<str> for Ident {
    fn eq(&self, other: &str) -> bool {
        self.to_string().as_str() == other
    }
}

/// Implement <Ident> == <String> comparisons
impl PartialEq<Ident> for str {
    fn eq(&self, other: &Ident) -> bool {
        *other == self
    }
}

/// Implement <Ident> == <&str> comparisons
impl PartialEq<&str> for Ident {
    fn eq(&self, other: &&str) -> bool {
        self.to_string().as_str() == *other
    }
}

/// Range of integer field numbers
pub type FieldNumberRange = std::ops::RangeInclusive<u32>;

impl fmt::Display for Ident {
    /// Displays an identifier with its full path
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.display())
    }
}

impl Ident {
    /// Return fully qualified identifier
    pub fn display(&self) -> String {
        match self.namespace.as_ref() {
            Some(ns) => format!("{}{}{}", ns, IDENT_PATH_DELIMITER, &self.name),
            None => self.name.clone(),
        }
    }
}

//impl std::string::ToString for Ident {
//    fn to_string(&self) -> String {
//        self.display()
//    }
//}

/// Field type - for object fields and method parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    /// 8-bit signed int
    Int8,

    /// 32-bit signed int
    Int32,

    /// 64-bit signed int
    Int64,

    /// 8-bit unsigned int
    Uint8,

    /// 32-bit unsigned int
    Uint32,

    /// 64-bit unsigned int
    Uint64,

    /// bool
    Bool,

    /// UTF-8-encoded String (including 7-bit US ASCII, which is a subset of UTF-8)
    String,

    /// arbitrary sequence of bytes.
    Bytes,

    /// 32-bit float
    Float32,

    /// 64-bit float (in syntax, also called 'double')
    Float64,

    /// RFC-3339-encoded date/time
    Datetime,

    /// Map (key-type, val-type). Supported key types: string, int, or bytes
    Map(Box<(FieldType, FieldType)>),

    /// Array/Vec ("repeated" in protobuf)
    Array(Box<FieldType>),

    /// A custom datatype
    /// parameter is path
    ObjectOrEnum(Ident),
}

impl FieldType {
    /// Returns true if type is one of the signed or unsigned integer types
    pub fn is_integer(&self) -> bool {
        matches!(
            *self,
            FieldType::Uint8
                | FieldType::Int8
                | FieldType::Uint32
                | FieldType::Int32
                | FieldType::Uint64
                | FieldType::Int64
        )
    }
}

/// A message Field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// Field name
    pub name: String,
    /// whether field is optional (true) or required (false)
    pub optional: bool,
    /// Field type
    pub typ: FieldType,
    /// Tag number
    pub number: u32,
    /// Field attributes
    pub attributes: Vec<Attribute>,
}

impl Field {
    /// returns the default value for a field based on its data type
    pub fn default_value(&self) -> Option<Constant> {
        // if a default is declared, return that
        if let Some(attr) = self.get_attribute("default") {
            attr.get("value").cloned()
        } else {
            None
        }
    }
}

impl HasAttributes for Field {
    fn attributes<'a>(&self) -> Attributes {
        let atr = &self.attributes;
        Attributes {
            base: atr.iter(), // self.attributes.iter(),
        }
    }
}

/// A Frodobuf message
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message name
    pub name: Ident,

    /// Message fields
    pub fields: Vec<Field>,

    /// Nested messages
    pub messages: Vec<Message>,

    /// Nested enums
    pub enums: Vec<Enumeration>,

    /// Attributes
    pub attributes: Vec<Attribute>,
    // Extension field numbers
    //pub extension_ranges: Vec<FieldNumberRange>,
    // Extensions
    //pub extensions: Vec<Extension>,
}

impl Message {
    /** Find a field by name. */
    pub fn get_field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

impl<'a> HasAttributes for Message {
    fn attributes(&'_ self) -> Attributes<'_> {
        Attributes {
            base: self.attributes.iter(),
        }
    }
}

/// A Frodobuf enumeration value - name (a symbol) and an Int32 value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    /// enum value name
    pub name: String,
    /// enum value number
    pub number: i32,
    /// enum value attributes
    pub attributes: Vec<Attribute>,
}

/// A Frodobuf enum - not to be confused with a rust enum (which is more like a protobuf oneof)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enumeration {
    /// enum name
    pub name: String,
    /// enum values
    pub values: Vec<EnumValue>,
    /// enum attributes
    pub attributes: Vec<Attribute>,
}

impl<'a> HasAttributes for Enumeration {
    fn attributes(&'_ self) -> Attributes<'_> {
        Attributes {
            base: self.attributes.iter(),
        }
    }
}

/// Service method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    /// Method name
    pub name: String,
    /// Input type, or None if the function takes no params
    pub input_type: Option<FieldType>,
    /// Output type, or None if the function return void
    pub output_type: Option<FieldType>,

    /*
    /// If this method is client streaming
    pub client_streaming: bool,
    /// If this method is server streaming
    pub server_streaming: bool,
     */
    /// Method attributes
    pub attributes: Vec<Attribute>,
}

impl<'a> HasAttributes for Method {
    fn attributes(&'_ self) -> Attributes<'_> {
        Attributes {
            base: self.attributes.iter(),
        }
    }
}

/// Service definition
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Service name
    pub name: Ident,

    /// methods in this service
    pub methods: Vec<Method>,

    /// Optional attributes for the service
    pub attributes: Vec<Attribute>,

    /// Serialized json schema
    pub schema: Option<String>,

    /// 256-bit hash, base64-encoded
    pub schema_id: Option<String>,
}

impl<'a> HasAttributes for Service {
    fn attributes(&'_ self) -> Attributes<'_> {
        Attributes {
            base: self.attributes.iter(),
        }
    }
}

/// constant = fullIdent | ( [ "-" | "+" ] intLit ) | ( [ "-" | "+" ] floatLit ) |
//                 strLit | boolLit
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constant {
    /// unsigned 64-bit integer
    U64(u64),
    /// signed 64-bit integer
    I64(i64),
    /// 64-bit floating point number
    F64(f64),
    /// boolean value
    Bool(bool),
    /// Identifier - must be previously defined as a constant
    Ident(Ident),
    /// literal string, as in `"Hello"`
    String(String),
    /// Sequence of raw bytes
    Bytes(Vec<u8>),
}
impl fmt::Display for Constant {
    /// format constant value for printing
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::U64(v) => write!(f, "{}", v),
            Constant::I64(v) => write!(f, "{}", v),
            Constant::F64(v) => write!(f, "{}", crate::format::format_float(*v)),
            Constant::Bool(v) => write!(f, "{}", v),
            Constant::Ident(v) => write!(f, "{}", v),
            Constant::String(v) => write!(f, "{}", v),
            Constant::Bytes(_) => write!(f, "<bytes>"),
        }
    }
}

impl Constant {
    /// format constant value for printing
    pub fn format(&self) -> String {
        match *self {
            Constant::U64(u) => u.to_string(),
            Constant::I64(i) => i.to_string(),
            Constant::F64(f) => crate::format::format_float(f),
            Constant::Bool(b) => b.to_string(),
            Constant::Ident(ref i) => format!("{}", i),
            // TODO: this needs escaping if used in code generation
            Constant::String(ref s) => s.to_string(),
            Constant::Bytes(_) => "<bytes>".to_string(),
        }
    }
    /// Returns Some(s) if value is a string constant, otherwise None
    pub fn as_string(&self) -> Option<&str> {
        match &self {
            Constant::String(s) => Some(crate::format::unquote(s.as_str())),
            _ => None,
        }
    }
}

impl From<String> for Constant {
    fn from(s: String) -> Constant {
        Constant::String(s)
    }
}

impl From<&str> for Constant {
    fn from(s: &str) -> Constant {
        Constant::String(s.to_string())
    }
}

impl From<Vec<u8>> for Constant {
    fn from(v: Vec<u8>) -> Constant {
        Constant::Bytes(v)
    }
}

impl From<u64> for Constant {
    fn from(val: u64) -> Constant {
        Constant::U64(val)
    }
}

impl From<u32> for Constant {
    fn from(val: u32) -> Constant {
        Constant::U64(val as u64)
    }
}

impl From<i64> for Constant {
    fn from(val: i64) -> Constant {
        Constant::I64(val)
    }
}

impl From<i32> for Constant {
    fn from(val: i32) -> Constant {
        Constant::I64(val as i64)
    }
}

/// A Schema definition read from a file
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Package
    pub namespace: Ident, // from "package"

    /// Top level messages
    pub messages: Vec<Message>,

    /// Enums
    pub enums: Vec<Enumeration>,

    /// Services
    pub services: Vec<Service>,

    /// Schema attributes
    pub attributes: Vec<Attribute>,
}

impl<'a> HasAttributes for Schema {
    fn attributes(&'_ self) -> Attributes<'_> {
        Attributes {
            base: self.attributes.iter(),
        }
    }
}
