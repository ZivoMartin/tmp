use super::include::*;

#[derive(PartialEq)]
pub struct VariationData {
    field: TypeField,
    variation: Vec<u16>,
}

impl VariationData {
    pub fn empty() -> VariationData {
        VariationData {
            field: TypeField::N,
            variation: Vec::new(),
        }
    }

    pub fn field(&self) -> TypeField {
        self.field
    }

    pub fn first(&self) -> u16 {
        self.variation[0]
    }

    pub fn from_serde_str(key: &str, vec: &[JsonValue]) -> Result<VariationData, &'static str> {
        let mut variation = Vec::new();
        for (i, n) in vec.iter().enumerate() {
            match n {
                JsonValue::Number(n) => variation.push(n.as_u64().unwrap() as u16),
                JsonValue::String(s) if s == ".." => {
                    let previous = if i == 0 {
                        return Err("Don't put the '..' at the beggining");
                    } else {
                        variation[i - 1]
                    };
                    let next = if vec.len() == i {
                        return Err("Don't put the '..' at the end");
                    } else {
                        &vec[i + 1]
                    };
                    for i in previous + 1..serde_n_to_u16(next) {
                        variation.push(i)
                    }
                },
                _ => return Err("Invalid variation"),
            }
        }
        Ok(VariationData {
            field: TypeField::from(key),
            variation,
        })
    }
}

pub struct Variation {
    data: VariationData,
    variation_index: usize,
    end: usize,
    variation_count: usize,
    conclusion: ResultFields,
}

impl Default for Variation {
    fn default() -> Self {
        Self::new()
    }
}

impl Variation {
    pub fn new() -> Variation {
        Variation {
            data: VariationData::empty(),
            end: 0,
            variation_index: 0,
            variation_count: 0,
            conclusion: ResultFields::new(),
        }
    }

    pub fn set_count(&mut self, n: usize) {
        self.variation_count = n
    }

    pub fn set_data(&mut self, data: VariationData) {
        self.end = data.variation.len();
        self.data = data;
    }

    pub fn reset(&mut self) {
        self.variation_count = 0;
    }

    pub fn reset_full(&mut self, fields: &mut Fields) {
        self.reset();
        self.variation_index = 0;
        self.actualise_data(fields);
    }

    pub fn evolve(
        &mut self,
        hmt: usize,
        args: &mut Fields,
        result: ResultFields,
    ) -> (bool, Option<ResultFields>) {
        self.variation_count += 1;
        self.conclusion += result;
        if self.variation_count == hmt {
            self.variation_count = 0;
            self.variation_index += 1;
            let conclusion = self.conclusion.extract();
            self.actualise_data(args);
            return (self.variation_index == self.end, Some(conclusion));
        }
        (false, None)
    }

    fn actualise_data(&self, args: &mut Fields) {
        if self.variation_index < self.end {
            args.set(self.data.field, self.data.variation[self.variation_index]);
        }
    }

    pub fn get_field_and_var(&self, n: u16) -> (Vec<u32>, String) {
        (
            if self.data.field == TypeField::TDenom {
                self.data
                    .variation
                    .iter()
                    .map(|t| ((n - 1) / *t) as u32)
                    .collect()
            } else {
                self.data.variation.iter().map(|v| *v as u32).collect()
            },
            self.data.field.to_string(),
        )
    }
}
