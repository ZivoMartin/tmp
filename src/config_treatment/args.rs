use crate::*;

use super::include::*;

pub const ARGS_SIZE: usize = 23;

use super::result_fields::*;
use super::subargs::*;
use super::variations::*;

#[derive(Default)]
pub struct Args {
    args: Vec<SubArgs>,
    current_arg: usize,
    output: String,
    recovering: Option<String>,
}

impl Args {
    pub fn init(&mut self) -> Evaluation {
        self.current_arg = self
            .args
            .iter()
            .position(|s| s.has_latency())
            .unwrap_or_else(|| {
                self.args
                    .iter()
                    .position(|s| s.has_debit())
                    .expect("Please don't start empty config")
            });
        if self.current_arg().has_latency() {
            Evaluation::Latency(Step::Sharing)
        } else {
            Evaluation::Debit(Step::Sharing)
        }
    }

    pub fn debit(&self) -> usize {
        self.current_arg().debit()
    }

    pub fn get_fields(&self) -> &Fields {
        self.current_arg().get_fields()
    }

    fn current_arg(&self) -> &SubArgs {
        &self.args[self.current_arg]
    }

    fn current_arg_mut(&mut self) -> &mut SubArgs {
        &mut self.args[self.current_arg]
    }

    pub fn evolve(&mut self, result: ResultFields, eval: Evaluation) -> (Option<Evaluation>, bool) {
        let recovering = self.recovering.clone();
        let (finished, evolved) = self.current_arg_mut().evolve(result, eval, recovering);
        if finished {
            self.current_arg += self.args[self.current_arg + 1..]
                .iter()
                .position(match eval {
                    Evaluation::Debit(_) => |subarg: &SubArgs| subarg.has_debit(),
                    Evaluation::Latency(_) => |subarg: &SubArgs| subarg.has_latency(),
                })
                .unwrap_or(self.args.len())
                + 1;
            if self.current_arg >= self.args.len() {
                if eval.is_latency() {
                    if let Some(i) = self.args.iter().position(|s| s.has_debit()) {
                        self.current_arg = i;
                        return (Some(Evaluation::Debit(Step::Sharing)), true);
                    }
                }
                self.conclude();
                return (None, true);
            }
        }
        let share_once = if (evolved || finished) && !self.current_arg().has_sharing() {
            self.current_arg_mut().only_once(&eval);
            true
        } else {
            false
        };
        (Some(eval), share_once)
    }

    pub fn conclude(&mut self) {
        if !self.output.is_empty() {
            let vec = JsonValue::Array(
                self.args
                    .iter_mut()
                    .map(SubArgs::conclude)
                    .collect::<Vec<JsonValue>>(),
            );
            if !self.output.ends_with(".json") {
                self.output += ".json";
            }
            let file = File::create(format!("../configs/results/{}", self.output))
                .expect("Failed to create file");
            let mut writer = BufWriter::new(file);
            writer
                .write_all(serde_json::to_string_pretty(&vec).unwrap().as_bytes())
                .expect("Failed to write");
            writer.flush().expect("Failed to flush");
        }
    }

    pub fn reset(&mut self) {
        self.current_arg_mut().reset()
    }

    pub fn n(&self) -> u16 {
        self.get_fields().n()
    }

    pub fn t(&self) -> u16 {
        self.get_fields().t()
    }

    pub fn nb_byz(&self) -> u16 {
        self.get_fields().get(TypeField::NbByz)
    }

    pub fn byz_comp(&self) -> ByzComp {
        (self.get_fields().get(TypeField::ByzComp) as u8).into()
    }

    pub fn hmt(&self, eval: Evaluation) -> usize {
        self.current_arg().hmt(&eval)
    }

    fn handle_args(res: &mut Args, args: &JsonValue) -> Result<(), &'static str> {
        let args = extract_serde_obj(args);
        for (key, value) in args.iter() {
            let key = key as &str;
            match key {
                "output" => res.output = extract_serde_string(value).to_string(),
                "recovering_file" => res.recovering = Some(extract_serde_string(value).to_string()),
                _ => return Err("Unvalid field in args"),
            }
        }
        Ok(())
    }

    fn handle_setup(setup: &JsonMap, subarg: &mut SubArgs) -> Result<(), &'static str> {
        for (key, value) in setup {
            match value {
                JsonValue::Number(n) => subarg.set_field_from_str(key, n.as_u64().unwrap() as u16),
                JsonValue::Array(arr) => {
                    subarg.set_variation_data(VariationData::from_serde_str(key, arr)?)
                },
                _ => return Err("Invalid arg for the setup"),
            }
        }
        Ok(())
    }

    fn handle_debit(val: &JsonMap, subarg: &mut SubArgs) -> Result<(), &'static str> {
        for (key, value) in val.iter() {
            let key = key as &str;
            match key {
                "hmt" => subarg.set_debit_hmt(serde_n_to_usize(value)),
                "duration" => subarg.set_debit_duration(serde_n_to_usize(value)),
                _ if POSSIBLE_DEBIT_RESULT_FIELD.contains(&key) => {
                    if *value == JsonValue::Bool(true) {
                        subarg.add_result(key)?
                    }
                },
                _ => return Err("Invalid key for the debit"),
            }
        }
        Ok(())
    }

    fn handle_latency(val: &JsonMap, args: &mut SubArgs) -> Result<(), &'static str> {
        for (key, value) in val.iter() {
            match key as &str {
                "hmt" => args.set_latency_hmt(serde_n_to_usize(value)),
                "steps" => {
                    let steps: Vec<&String> = extract_serde_arr(value)
                        .iter()
                        .map(extract_serde_string)
                        .collect();
                    for step in steps {
                        args.add_result(step)?
                    }
                },
                _ => return Err("Invalid key for the latency"),
            }
        }
        Ok(())
    }

    pub fn from_file(path: String) -> Result<Args, &'static str> {
        let mut res = Args::default();
        let content = read_to_string(path).expect("Path invalid");
        let value: JsonValue = from_str(&content).expect("The given json file is invalid");
        let json_args = extract_serde_arr(&value);
        Self::handle_args(&mut res, &json_args[0])?;
        for sim in json_args.iter().skip(1).map(extract_serde_obj) {
            let mut subarg = SubArgs::new();
            for (key, val) in sim.iter() {
                let val = extract_serde_obj(val);
                (match key as &str {
                    "debit" => Self::handle_debit,
                    "latency" => Self::handle_latency,
                    "setup" => Self::handle_setup,
                    _ => return Err("Invalid key"),
                })(val, &mut subarg)?
            }
            res.args.push(subarg);
        }
        Ok(res)
    }

    pub fn reconstruct(&self, eval: Evaluation) -> bool {
        self.current_arg().reconstruct(eval)
    }
}
