pub use byteorder::{ByteOrder, LittleEndian};
pub use rand::Rng;
pub use std::collections::HashMap;
pub use std::process::exit;
pub use std::sync::Arc;
pub use std::time::Instant;
pub use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub use tokio::net::{TcpListener, TcpStream};
pub use tokio::process::Command;
pub use tokio::sync::mpsc::{channel, Receiver, Sender};
pub use tokio::sync::Mutex;
mod config_treatment;
pub use config_treatment::include::*;

pub const BASE_CAPACITY: usize = 2000;
pub type Bytes<'a> = &'a [u8];
pub type Wrapped<T> = Arc<Mutex<T>>;
pub static INTERFACE_IP: &str = "127.0.0.1:18800";
pub static UI_IP: &str = "127.0.0.1:18801";

#[macro_export]
macro_rules! as_number {
    ($t:ty, enum $enum_name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum $enum_name {
            $($variant),*
        }

        impl From<$t> for $enum_name {
            fn from(value: $t) -> Self {
                match value {
                    $(x if x == $enum_name::$variant as $t => $enum_name::$variant),*,
                    _ => panic!("Invalid value for enum"),
                }
            }
        }

        impl From<$enum_name> for $t {
            fn from(variant: $enum_name) -> Self {
                variant as $t
            }
        }
    };
}

as_number!(
    u8,
    enum CommandCode {
        DEALTHIS,
        SHARE,
        ACK,
        REST,
        SETUP,
        KEY,
        RECONSTRUCT,
        NEWSHARE,
        STOP,
    }
);

as_number!(
    u8,
    enum InterfaceCode {
        CONNECT,
        INTERRUPT,
        OUTPUT,
        FROMFILE,
    }
);

as_number!(
    u8,
    enum ByzComp {
        Honnest,
        Sleeper,
    }
);

impl ByzComp {
    pub fn to_u16(&self) -> u16 {
        Into::<u8>::into(*self) as u16
    }
}

as_number!(
    u8,
    enum ErrorCode {
        OK,
        UnvalidSigns,
        UnvalidShares,
        IncoherentBatch,
        MissingShare,
    }
);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Evaluation {
    Debit(Step),
    Latency(Step),
}

impl Default for Evaluation {
    fn default() -> Self {
        Evaluation::Latency(Step::Sharing)
    }
}

impl Evaluation {
    pub fn is_latency(&self) -> bool {
        matches!(self, Evaluation::Latency(_))
    }

    pub fn is_debit(&self) -> bool {
        matches!(self, Evaluation::Debit(_))
    }

    pub fn is_reconstruct(&self) -> bool {
        let step = match self {
            Evaluation::Debit(s) => s,
            Evaluation::Latency(s) => s,
        };
        *step == Step::Reconstruct
    }

    pub fn change_step(&mut self, step: Step) {
        let new_self = match self {
            Evaluation::Debit(_) => Evaluation::Debit(step),
            Evaluation::Latency(_) => Evaluation::Latency(step),
        };
        *self = new_self;
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Step {
    Sharing,
    Reconstruct,
}

pub fn trim_buf(buf: &[u8]) -> &[u8] {
    if let Some(pos) = buf.iter().rposition(|&x| x != 0) {
        &buf[..=pos]
    } else {
        &[]
    }
}

pub fn extract_port(addr: &str) -> u16 {
    addr.split(':')
        .nth(1)
        .unwrap_or_else(|| panic!("Invalid ip: {addr}"))
        .parse::<u16>()
        .expect("Failed to parse port in number")
}

pub fn extract_ip(addr: &str) -> String {
    addr.split(':')
        .next()
        .expect("Invalid address ip")
        .to_string()
}

pub fn write_bytes(bytes: &mut [u8], vec: &[u8]) -> u32 {
    let mut res = 4;
    LittleEndian::write_u32(bytes, vec.len() as u32);
    for (i, b) in vec.iter().enumerate() {
        bytes[i + 4] = *b;
        res += 1
    }
    res
}

pub fn write_vec(bytes: &mut Vec<u8>, index: usize, vec: &[u8]) -> u32 {
    if (bytes.len() as f32 * 0.8) as usize <= index + vec.len() + 4 {
        bytes.append(&mut vec![0; BASE_CAPACITY])
    }
    write_bytes(&mut bytes[index..], vec)
}

pub fn read_vec(bytes: Bytes<'_>) -> (usize, Bytes<'_>) {
    let n = LittleEndian::read_u32(bytes) as usize;
    (4 + n, &bytes[4..4 + n])
}

pub fn read_vec_u16(bytes: Bytes<'_>) -> (usize, Vec<u16>) {
    let n = LittleEndian::read_u32(bytes) as usize;
    let mut res = Vec::<u16>::with_capacity(n);
    for i in 0..n {
        res.push(LittleEndian::read_u16(&bytes[4 + i * 2..]));
    }
    (4 + n * 2, res)
}

pub fn read_ip_vec(bytes: Bytes<'_>) -> (Vec<String>, usize) {
    let n = LittleEndian::read_u16(bytes) as usize;
    let mut consumed = 2;
    (
        (0..n)
            .map(|_| {
                let ip_size = bytes[consumed] as usize;
                consumed += 1 + ip_size;
                String::from_utf8_lossy(&bytes[consumed - ip_size..consumed]).to_string()
            })
            .collect::<Vec<String>>(),
        consumed,
    )
}

pub async fn generate_random_port(ip: &str) -> (u16, TcpListener) {
    let mut rng = rand::thread_rng();
    loop {
        let port = rng.gen_range(1024..60_000);
        if let Ok(listener) = TcpListener::bind(&format!("{ip}:{port}")).await {
            break (port, listener);
        }
    }
}

pub fn log(log: String) {
    println!("{log}");
    // tokio::spawn(async move {
    //     TcpStream::connect(UI_IP)
    //         .await
    //         .expect("Failed to contact ui")
    //         .write_all(log.as_bytes())
    //         .await
    //         .expect("Failed to write")
    // });
}
//format: rustfmt src/lib.rs src/config_treatment/args.rs src/config_treatment/gnu.rs src/config_treatment/result_fields.rs src/config_treatment/variations.rs src/config_treatment/fields.rs src/config_treatment/subargs.rs  interface/src/main.rs nodes/src/main.rs nodes/src/protocols/avss_simpl.rs nodes/src/crypto/mod.rs nodes/src/crypto/ark_custom.rs ui/src/settings_scene.rs ui/src/widgets.rs ui/src/graph_scene.rs ui/src/config_edit.rs
