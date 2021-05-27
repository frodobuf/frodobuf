//use std::f64;

#[derive(Debug)]
pub enum ProtobufFloatParseError {
    EmptyString,
    CannotParseFloat,
}

pub type ProtobufFloatParseResult<T> = Result<T, ProtobufFloatParseError>;

pub const PROTOBUF_NAN: &str = "nan";
pub const PROTOBUF_INF: &str = "inf";

/// Parse float from `.proto` format
pub fn parse_protobuf_float(s: &str) -> ProtobufFloatParseResult<f64> {
    if s.is_empty() {
        return Err(ProtobufFloatParseError::EmptyString);
    }
    if s == PROTOBUF_NAN {
        return Ok(f64::NAN);
    }
    if s == PROTOBUF_INF || s == format!("+{}", PROTOBUF_INF) {
        return Ok(f64::INFINITY);
    }
    if s == format!("-{}", PROTOBUF_INF) {
        return Ok(f64::NEG_INFINITY);
    }
    match s.parse() {
        Ok(f) => Ok(f),
        Err(_) => Err(ProtobufFloatParseError::CannotParseFloat),
    }
}
