/// Output formats

const FLOAT_NAN: &str = "nan";
const FLOAT_INF: &str = "inf";

/// Format float number, with optional nan, and +/- inf
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        FLOAT_NAN.to_string()
    } else if f.is_infinite() {
        if f >= 0.0 {
            FLOAT_INF.to_string()
        } else {
            format!("-{}", FLOAT_INF)
        }
    } else {
        // TODO: make sure doesn't lose precision
        format!("{}", f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format_float() {
        assert_eq!("10", format_float(10.0));
    }
}
