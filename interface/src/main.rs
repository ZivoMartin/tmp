use global_lib::*;
use std::env;
// use public_ip;

const BUF_SIZE: usize = 5000;
const SECRET: u128 = std::u128::MAX;

struct Interface {
    args: Args,
    eval: Evaluation,
    interface_ip: String,
    nodes: Vec<String>,
    output_count: u16,
    result: ResultFields,
    reconstruction_time: Duration,
    hmt: usize,
    interrupt: bool,
    timer: Instant,
}

impl Interface {
    async fn new(_public: bool) -> (Interface, TcpListener) {
        // let ip = if public { public_ip::addr().await.expect("Failed to get public ip").to_string() } else { "127.0.0.1".to_string() };
        // let (port, listener) = generate_random_port(&ip).await;
        let listener = TcpListener::bind(INTERFACE_IP)
            .await
            .expect("Failed to bind interface");
        (
            Interface {
                args: Args::default(),
                interface_ip: INTERFACE_IP.to_string(),
                eval: Evaluation::default(),
                output_count: 0,
                nodes: Vec::new(),
                result: ResultFields::new(),
                reconstruction_time: 0,
                hmt: 0,
                interrupt: false,
                timer: Instant::now(),
            },
            listener,
        )
    }

    async fn new_command(interface: Wrapped<Interface>, ip: String, bytes: Bytes<'_>) -> bool {
        match bytes[0].into() {
            InterfaceCode::CONNECT => Self::add_node(interface, ip, &bytes[1..]).await,
            InterfaceCode::OUTPUT => Self::new_output(interface, &bytes[1..]).await,
            InterfaceCode::FROMFILE => Self::load_file(interface, &bytes[1..]).await,
            InterfaceCode::INTERRUPT => Self::interrupt(interface).await,
        }
        false
    }

    async fn interrupt(interface: Wrapped<Interface>) {
        interface.lock().await.interrupt = true;
    }

    async fn load_file(interface: Wrapped<Interface>, bytes: Bytes<'_>) {
        {
            let mut interface = interface.lock().await;
            let path = String::from_utf8_lossy(trim_buf(bytes)).to_string();
            interface.args = match Args::from_file(path) {
                Ok(arg) => arg,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                },
            };
            interface.reset_hmt();
            interface.eval = interface.args.init();
        }
        if interface.lock().await.eval.is_latency() {
            Self::send_share_message(interface).await;
        } else {
            Self::setup_debit(interface).await
        }
    }

    async fn setup_debit(interface: Wrapped<Interface>) {
        {
            let mut interface = interface.lock().await;
            println!("DEBIT COMPUTING");
            interface.reset_hmt();
            interface.result = ResultFields::new();
            interface.timer = Instant::now();
            interface.eval = Evaluation::Debit(Step::Sharing);
        }
        Self::send_share_message(interface).await
    }

    async fn fix_node_number(&mut self) -> bool {
        let n = self.args.n() as usize;
        if self.nodes.len() < n {
            for _ in self.nodes.len()..n {
                let ip = self.interface_ip.clone();
                tokio::spawn(async move {
                    Command::new("../target/release/nodes")
                        .arg(&ip)
                        .status()
                        .await
                        .expect("Failed to create a new node");
                });
            }
        }
        n > self.nodes.len()
    }

    async fn ready_to_share(interface: Wrapped<Interface>) {
        loop {
            let interface = interface.lock().await;
            if interface.nodes.len() >= interface.args.n() as usize {
                break;
            }
        }
    }

    async fn send_share_message(interface: Wrapped<Interface>) {
        let network_changed = interface.lock().await.fix_node_number().await;
        Self::ready_to_share(interface.clone()).await;
        let mut interface = interface.lock().await;
        log(format!(
            "Sharing with: n = {}, t = {}, nb_byz = {}, hmt = {}",
            interface.args.n(),
            interface.args.t(),
            interface.args.nb_byz(),
            interface.hmt
        ));
        interface.output_count = 0;
        let dealer = interface.args.nb_byz();
        let mut msg = [0; BUF_SIZE];
        msg[0] = CommandCode::SETUP.into();
        LittleEndian::write_u16(&mut msg[1..], dealer);
        msg[3] = interface.args.byz_comp().into();
        LittleEndian::write_u16(&mut msg[4..], interface.args.t());
        LittleEndian::write_u16(&mut msg[6..], interface.args.n());
        let mut index = 8;
        if network_changed {
            for addr in &interface.nodes {
                let bytes = addr.bytes();
                msg[index] = bytes.len() as u8;
                index += 1;
                for b in bytes {
                    msg[index] = b;
                    index += 1;
                }
            }
        }
        for i in 0..interface.args.nb_byz() {
            private_message(&interface.nodes[i as usize], &msg).await;
        }
        msg[3] = ByzComp::Honnest.into();
        for node in interface.nodes.iter().skip(interface.args.nb_byz().into()) {
            private_message(node, &msg).await;
        }
        interface.contact_dealer(dealer).await;
    }

    async fn contact_dealer(&self, dealer: u16) {
        let mut deal_msg = [0; 17];
        deal_msg[0] = CommandCode::DEALTHIS.into();
        LittleEndian::write_u128(&mut deal_msg[1..], SECRET);
        private_message(&self.nodes[dealer as usize], &deal_msg).await;
    }

    async fn add_node(interface: Wrapped<Interface>, ip: String, bytes: Bytes<'_>) {
        let port = LittleEndian::read_u16(bytes);
        let ip = extract_ip(&ip) + ":" + &port.to_string();
        let mut interface = interface.lock().await;
        //        println!("new node: {}", ip);
        interface.nodes.push(ip);
    }

    async fn reconstruct(interface: Wrapped<Interface>) {
        let mut interface = interface.lock().await;
        log(format!(
            "Reconstructing with: n = {}, t = {}, nb_byz = {}, hmt: {}",
            interface.args.n(),
            interface.args.t(),
            interface.args.nb_byz(),
            interface.hmt
        ));
        interface.output_count = 0;
        let message = [CommandCode::RECONSTRUCT.into()];
        for addr in &interface.nodes[..interface.args.n() as usize] {
            private_message(addr, &message).await
        }
    }

    async fn new_output(interface: Wrapped<Interface>, bytes: Bytes<'_>) {
        let again = {
            let mut interface = interface.lock().await;
            interface.output_count += 1;
            let result = ResultFields::from_bytes(bytes);
            let reconstruct_time = result.get(TypeResultField::Reconstruction);
            interface.result += result;
            if interface.output_count == interface.args.n() {
                interface.finish();
                true
            } else {
                if interface.is_reconstructing() && interface.output_count == interface.args.t() + 1
                {
                    interface.reconstruction_time = reconstruct_time;
                    for addr in &interface.nodes[..interface.args.n() as usize] {
                        private_message(addr, &[CommandCode::STOP.into()]).await
                    }
                }
                false
            }
        };
        if again {
            let eval: Evaluation = interface.lock().await.eval;
            match eval {
                Evaluation::Debit(_) => Self::process_debit(interface).await,
                Evaluation::Latency(_) => Self::process_latency(interface).await,
            }
        }
    }

    fn is_reconstructing(&self) -> bool {
        self.eval.is_reconstruct()
    }

    fn finish(&mut self) {
        match self.eval {
            Evaluation::Debit(_) => (),
            Evaluation::Latency(_) => {
                self.hmt -= 1;
                if self.hmt == 0 {
                    self.hmt_null()
                } else if self.is_reconstructing() {
                    let reconstruction_time = self.reconstruction_time;
                    self.result
                        .set(TypeResultField::Reconstruction, reconstruction_time)
                }
            },
        }
    }

    fn hmt_null(&mut self) {
        if self.is_reconstructing() {
            let reconstruction_time = self.reconstruction_time;
            self.result
                .set(TypeResultField::Reconstruction, reconstruction_time)
        }
        if !self.is_reconstructing() && self.args.reconstruct(self.eval) {
            self.eval.change_step(Step::Reconstruct);
            self.args.reset();
        } else {
            self.eval.change_step(Step::Sharing);
        }
        self.reset_hmt();
    }

    fn should_evolve(&self) -> bool {
        self.hmt != self.args.hmt(self.eval) || !self.is_reconstructing()
    }

    async fn process_latency(interface: Wrapped<Interface>) {
        let (result, evolve) = {
            let mut interface = interface.lock().await;
            let average_verify =
                interface.result.get(TypeResultField::Verify) / interface.args.n() as u128;
            let average_first_receiv =
                interface.result.get(TypeResultField::FirstReceiv) / interface.args.n() as u128;
            interface
                .result
                .set(TypeResultField::Verify, average_verify);
            interface
                .result
                .set(TypeResultField::FirstReceiv, average_first_receiv);
            let evolve = interface.should_evolve();
            (interface.result.extract(), evolve)
        };

        let eval = interface.lock().await.eval;
        if !evolve {
            Self::again(interface).await
        } else {
            let (eval, has_changed) = interface.lock().await.args.evolve(result, eval);
            if has_changed {
                interface.lock().await.hmt = 1;
            }
            match eval {
                Some(eval) if eval.is_debit() => Self::setup_debit(interface).await,
                Some(_) => Self::again(interface).await,
                None => log("Success !".to_string()),
            }
        }
    }

    async fn process_debit(interface: Wrapped<Interface>) {
        {
            let mut interface = interface.lock().await;
            if interface.timer.elapsed()
                >= std::time::Duration::new(interface.args.debit() as u64, 0)
            {
                log("OVER".to_string());
                interface.hmt -= 1;
                if interface.hmt == 0 {
                    interface.hmt_null();
                }
                let result = interface.result.extract();
                let eval = interface.eval;
                if interface.should_evolve() {
                    match interface.args.evolve(result, eval) {
                        (Some(eval), changed) => {
                            if changed {
                                interface.hmt = 1;
                            }
                            interface.eval = eval
                        },
                        (None, _) => {
                            log("Success !".to_string());
                            return;
                        },
                    }
                }
                interface.timer = Instant::now()
            } else {
                let field = if interface.is_reconstructing() {
                    TypeResultField::DebitReconstruct
                } else {
                    TypeResultField::DebitSharing
                };
                let val = interface.result.get(field) + 1;
                interface.result.set(field, val);
            }
        }
        Self::again(interface).await
    }

    async fn again(interface: Wrapped<Interface>) {
        if interface.lock().await.interrupt {
            interface.lock().await.interrupt = false;
        } else if interface.lock().await.is_reconstructing() {
            Self::reconstruct(interface).await
        } else {
            Self::send_share_message(interface).await
        }
    }

    fn reset_hmt(&mut self) {
        self.hmt = self.args.hmt(self.eval)
    }
}

async fn private_message<'a>(addr: &str, msg: Bytes<'a>) {
    TcpStream::connect(addr)
        .await
        .expect("Invalid node")
        .write_all(msg)
        .await
        .expect("Failed to write")
}

fn handle_args(interface: Wrapped<Interface>) {
    let path = env::args().nth(1);
    tokio::spawn(async move {
        if let Some(path) = path {
            Interface::load_file(interface, path.as_bytes()).await
        }    
    });
}

#[tokio::main]
async fn main() {
    let (interface, listener) = Interface::new(false).await;
    let interface = Arc::new(Mutex::new(interface));
    handle_args(interface.clone());
    loop {
        let (mut socket, ip) = listener.accept().await.unwrap();
        let interface = interface.clone();
        tokio::spawn(async move {
            let mut buf = [0; BUF_SIZE];
            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                if Interface::new_command(interface.clone(), ip.to_string(), &buf[..n]).await {
                    break;
                }
            }
        });
    }
}
