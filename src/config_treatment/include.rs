pub use super::args::*;
pub use super::fields::*;
pub use super::gnu::*;
pub use super::result_fields::*;
pub use super::variations::*;

pub use serde_json::{from_str, Map, Value as JsonValue};
pub type JsonMap = Map<String, JsonValue>;
pub use crate::as_number;
pub use crate::ByzComp;
pub use crate::Evaluation;
pub use serde_json::Number;
pub use std::collections::HashMap;
pub use std::default::Default;
pub use std::fmt::{Display, Error as FmtErr, Formatter};
pub use std::fs::read_to_string;
pub use std::fs::File;
pub use std::io::{BufWriter, Write};

pub fn extract_serde_obj(v: &JsonValue) -> &JsonMap {
    match v {
        JsonValue::Object(m) => m,
        _ => panic!("Failed to extract Object"),
    }
}

pub fn extract_serde_string(v: &JsonValue) -> &String {
    match v {
        JsonValue::String(s) => s,
        _ => panic!("Failed to extract String"),
    }
}

pub fn extract_serde_arr(v: &JsonValue) -> &Vec<JsonValue> {
    match v {
        JsonValue::Array(arr) => arr,
        _ => panic!("Failed to extract Array"),
    }
}

pub fn serde_n_to_u16(n: &JsonValue) -> u16 {
    if let JsonValue::Number(n) = n {
        n.as_u64().unwrap() as u16
    } else {
        panic!("Given array is not fully a number array")
    }
}

pub fn serde_n_to_usize(n: &JsonValue) -> usize {
    if let JsonValue::Number(n) = n {
        n.as_u64().unwrap() as usize
    } else {
        panic!("Given array is not fully a number array")
    }
}
