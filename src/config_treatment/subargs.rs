use super::include::*;
use std::fs::OpenOptions;

pub struct SubArgs {
    latency_hmt: usize,
    debit_hmt: usize,
    debit_duration: usize,
    fields: Fields,
    variation: Variation,
    result: HashMap<ResultField, Vec<Duration>>,
}

impl SubArgs {
    pub fn new() -> Self {
        SubArgs {
            fields: Fields::new(),
            variation: Variation::new(),
            result: HashMap::new(),
            latency_hmt: 0,
            debit_duration: 1,
            debit_hmt: 0,
        }
    }

    pub fn has_sharing(&self) -> bool {
        self.result
            .keys()
            .any(|k| *k != "reconstruct" && *k != "total_reconstruct")
    }

    pub fn only_once(&mut self, eval: &Evaluation) {
        self.variation.set_count(self.hmt(eval) - 1);
    }

    pub fn get_fields(&self) -> &Fields {
        &self.fields
    }

    pub fn add_result(&mut self, result: &str) -> Result<(), &'static str> {
        if !result_exists(result) {
            return Err("The asked result doesn't exist");
        }
        self.result.insert(result.to_string(), Vec::new());
        Ok(())
    }

    pub fn reset(&mut self) {
        self.variation.reset()
    }

    pub fn set_field_from_str(&mut self, field: &str, val: u16) {
        self.fields.set(TypeField::from(field), val)
    }

    pub fn set_variation_data(&mut self, data: VariationData) {
        self.fields.set(data.field(), data.first());
        self.variation.set_data(data);
    }

    pub fn set_latency_hmt(&mut self, hmt: usize) {
        self.latency_hmt = hmt;
    }

    pub fn set_debit_hmt(&mut self, hmt: usize) {
        self.debit_hmt = hmt;
    }

    pub fn hmt(&self, eval: &Evaluation) -> usize {
        match eval {
            Evaluation::Debit(_) => self.debit_hmt,
            Evaluation::Latency(_) => self.latency_hmt,
        }
    }

    pub fn evolve(
        &mut self,
        result: ResultFields,
        eval: Evaluation,
        recovering: Option<String>,
    ) -> (bool, bool) {
        if result.is_err() {
            println!("error, try to restart");
            return (false, false);
        }
        let (stop, conclusion) = self
            .variation
            .evolve(self.hmt(&eval), &mut self.fields, result);
        if let Some(ref conclusion) = conclusion {
            let fields: &[&str] = match eval {
                Evaluation::Debit(_) => &POSSIBLE_DEBIT_RESULT_FIELD,
                Evaluation::Latency(_) => &POSSIBLE_LATENCY_RESULT_FIELD,
            };
            let hmt = self.hmt(&eval) as u128;
            self.result.iter_mut().for_each(|(k, vec)| {
                if fields.contains(&(k as &str)) {
                    vec.push(conclusion.get_from_str(k) / hmt);
                }
            });
        }
        if stop {
            self.recover(recovering)
        }
        (stop, conclusion.is_some())
    }

    fn recover(&mut self, recover_file: Option<String>) {
        if let Some(recover_file) = recover_file {
            let file_path = format!("../configs/results/{recover_file}");
            let file = OpenOptions::new()
                .append(true)
                .create(true) // Crée le fichier s'il n'existe pas
                .open(file_path)
                .expect("Failed to open file");

            let mut writer = BufWriter::new(file);
            writer
                .write_all(
                    serde_json::to_string_pretty(&self.conclude())
                        .unwrap()
                        .as_bytes(),
                )
                .expect("Failed to write");
        }
        self.variation.reset_full(&mut self.fields);
    }

    pub fn get_field_and_var(&self) -> (Vec<u32>, String, Vec<(String, u32)>) {
        let (vec, field) = self.variation.get_field_and_var(self.fields.n());
        let mut base_state = [
            ("n", self.fields.n()),
            ("t", self.fields.get(TypeField::TDenom)),
            ("nb_byz", self.fields.get(TypeField::NbByz)),
            ("byz_comp", self.fields.get(TypeField::ByzComp)),
        ]
        .iter()
        .map(|(f, v)| (f.to_string(), *v as u32))
        .collect::<Vec<_>>();
        base_state.remove(base_state.iter().position(|(f, _)| *f == field).unwrap());
        (vec, field, base_state)
    }

    pub fn get_result_map(&mut self) -> (JsonValue, JsonValue) {
        let mut latency_result = JsonMap::new();
        let mut debit_result = JsonMap::new();
        self.result.iter().for_each(|(key, val)| {
            let key = key as &str;
            let arr = JsonValue::Array(
                val.iter()
                    .map(|n| JsonValue::Number(Number::from(*n as usize)))
                    .collect(),
            );
            if POSSIBLE_DEBIT_RESULT_FIELD.contains(&key) {
                debit_result.insert(key.to_string(), arr)
            } else {
                latency_result.insert(key.to_string(), arr)
            };
        });
        (
            serde_json::to_value(&debit_result).expect("Failed to convert in json value"),
            serde_json::to_value(&latency_result).expect("Failed to convert in json value"),
        )
    }

    pub fn set_debit_duration(&mut self, d: usize) {
        self.debit_duration = d;
    }

    pub fn debit(&self) -> usize {
        self.debit_duration
    }

    pub fn reconstruct(&self, eval: Evaluation) -> bool {
        match eval {
            Evaluation::Debit(_) => self.result.contains_key("reconstruct"),
            Evaluation::Latency(_) => self.result.contains_key("total_reconstruct"),
        }
    }

    pub fn has_debit(&self) -> bool {
        self.debit_hmt != 0
    }

    pub fn has_latency(&self) -> bool {
        self.latency_hmt != 0
    }

    pub fn conclude(&mut self) -> JsonValue {
        let (debit_map, latency_map) = self.get_result_map();
        let mut map = serde_json::Map::new();
        let (variation, field, base_state) = self.get_field_and_var();
        map.insert("field".to_string(), field.into());
        map.insert("variation".to_string(), variation.into());
        for (f, v) in base_state {
            map.insert(f, v.into());
        }
        let mut obj = JsonMap::new();
        obj.insert("args".to_string(), JsonValue::Object(map));
        if !extract_serde_obj(&debit_map).is_empty() {
            obj.insert("debit".to_string(), debit_map);
        }
        if !extract_serde_obj(&latency_map).is_empty() {
            obj.insert("latency".to_string(), latency_map);
        }
        JsonValue::Object(obj)
    }
}
