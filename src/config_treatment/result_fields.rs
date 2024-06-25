pub type Duration = u128; // in ms
pub type ResultField = String;
use crate::as_number;
use crate::ErrorCode;
use byteorder::{ByteOrder, LittleEndian};
use std::ops::AddAssign;

pub static POSSIBLE_LATENCY_RESULT_FIELD: [&str; 7] = [
    "verify",
    "dealing",
    "first_receiv",
    "broadcasting",
    "messages_computing",
    "total_sharing",
    "total_reconstruct",
];

pub static POSSIBLE_DEBIT_RESULT_FIELD: [&str; 2] = ["sharing", "reconstruct"];

const NB_FIELD: usize = 9;
pub const RESULT_FIELDS_SIZE: usize = NB_FIELD * 16 + 1;

pub fn result_exists(res: &str) -> bool {
    POSSIBLE_DEBIT_RESULT_FIELD.contains(&res) || POSSIBLE_LATENCY_RESULT_FIELD.contains(&res)
}

as_number!(
    usize,
    enum TypeResultField {
        Verify,
        Dealing,
        FirstReceiv,
        BroadCasting,
        MessagesComputing,
        Total,
        Reconstruction,
        DebitSharing,
        DebitReconstruct,
    }
);

#[derive(Clone, Debug)]
pub struct ResultFields {
    results: Vec<Duration>,
    code: ErrorCode,
}

impl Default for ResultFields {
    fn default() -> Self {
        ResultFields {
            results: Vec::new(),
            code: ErrorCode::OK,
        }
    }
}

impl AddAssign for ResultFields {
    fn add_assign(&mut self, other: ResultFields) {
        self.results
            .iter_mut()
            .zip(other.results.iter())
            .for_each(|(my, his)| {
                *my = *my + *his;
            })
    }
}

impl ResultFields {
    pub fn new() -> Self {
        ResultFields {
            code: ErrorCode::OK,
            results: vec![0; NB_FIELD],
        }
    }

    pub fn is_err(&self) -> bool {
        self.code != ErrorCode::OK
    }

    pub fn get(&self, i: TypeResultField) -> Duration {
        self.results[Into::<usize>::into(i)]
    }

    pub fn set(&mut self, i: TypeResultField, v: Duration) {
        self.results[Into::<usize>::into(i)] = v;
    }

    pub fn extract(&mut self) -> ResultFields {
        let res = self.clone();
        *self = ResultFields::new();
        res
    }

    pub fn to_bytes(&self, bytes: &mut [u8], code: ErrorCode) {
        bytes[0] = code.into();
        let bytes = &mut bytes[1..];
        self.results
            .iter()
            .enumerate()
            .for_each(|(i, r)| LittleEndian::write_u128(&mut bytes[i * 16..], *r));
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let code: ErrorCode = bytes[0].into();
        let bytes = &bytes[1..];
        ResultFields {
            code,
            results: (0..NB_FIELD)
                .map(|i| LittleEndian::read_u128(&bytes[i * 16..]))
                .collect(),
        }
    }

    pub fn get_from_str(&self, field: &str) -> Duration {
        self.results[POSSIBLE_LATENCY_RESULT_FIELD
            .iter()
            .chain(POSSIBLE_DEBIT_RESULT_FIELD.iter())
            .position(|f| *f == field)
            .unwrap()]
    }
}
