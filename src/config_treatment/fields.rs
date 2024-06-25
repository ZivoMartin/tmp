use super::include::*;

as_number!(
    usize,
    enum TypeField {
        N,
        TDenom,
        NbByz,
        ByzComp,
        T,
    }
);

pub static STATIC_TYPE_FIELD: [&str; 4] = ["n", "t", "nb_byz", "byz_comp"];

impl Display for TypeField {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtErr> {
        write!(
            f,
            "{}",
            String::from(STATIC_TYPE_FIELD[Into::<usize>::into(*self)])
        )
    }
}

impl From<&str> for TypeField {
    fn from(s: &str) -> TypeField {
        STATIC_TYPE_FIELD
            .iter()
            .position(|elt| elt == &s)
            .unwrap_or_else(|| panic!("unvalid string: {s}"))
            .into()
    }
}

pub struct Fields {
    fields: Vec<u16>,
}

impl Fields {
    pub fn new() -> Self {
        Fields { fields: vec![0; 4] }
    }

    pub fn get(&self, field: TypeField) -> u16 {
        self.fields[Into::<usize>::into(field)]
    }

    pub fn set(&mut self, field: TypeField, val: u16) {
        self.fields[Into::<usize>::into(field)] = val;
    }

    pub fn n(&self) -> u16 {
        self.get(TypeField::N)
    }

    pub fn t(&self) -> u16 {
        ((self.n() - 1) as f32 * (self.get(TypeField::TDenom) as f32 / 100.0)) as u16
    }
}

impl Default for Fields {
    fn default() -> Self {
        Fields {
            fields: vec![61, 20, 0, ByzComp::Sleeper.to_u16()],
        }
    }
}
