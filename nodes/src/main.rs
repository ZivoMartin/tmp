use nodes::*;
mod protocols;
use protocols::avss_simpl::*;

pub enum DealerMessage {
    Message(u16, Sign),
}
use std::env;

#[allow(dead_code)]
struct ProbasMaliciousAct {
    honnest: f32,
    random: f32,
    sleeper: f32,
}

#[derive(Debug, Clone)]
pub struct ExternNode {
    pub addr: String,
    pub p_key: PublicKey,
}

impl ExternNode {
    fn new(p_key: PublicKey, addr: String) -> ExternNode {
        ExternNode { addr, p_key }
    }

    fn addr(&self) -> &str {
        &self.addr
    }
}

#[derive(Clone)]
pub struct ShareSet {
    comm: Option<Commitment>,
    set: HashMap<u16, (Share, Proof)>,
}

impl ShareSet {
    fn new() -> ShareSet {
        ShareSet {
            comm: None,
            set: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.set.clear();
        self.comm = None;
    }

    fn len(&self) -> u16 {
        self.set.len() as u16
    }

    fn get(&self, i: u16) -> &(Share, Proof) {
        self.set.get(&i).as_ref().unwrap()
    }

    fn new_share(&mut self, i: u16, share: Share, proof: Proof) {
        self.set.insert(i, (share, proof));
    }

    fn set_comm(&mut self, comm: Commitment) {
        // println!("in");
        self.comm = Some(comm)
    }

    fn get_comm(&self) -> &Commitment {
        self.comm.as_ref().unwrap()
    }
}

#[allow(dead_code)]
pub struct Node {
    t: u16,
    n: u16,
    interface_ip: String,
    connected_node: u16,
    port: u16,
    network: Vec<ExternNode>,
    dealer: u16,
    byz_comp: ByzComp,
    index: u16,
    dealer_sender: Option<Sender<DealerMessage>>,
    keys: KeyPair,
    im_setup: bool,
    shares: ShareSet,
    reconstruct_share_set: ShareSet,
    timer: Instant,
    result: ResultFields,
    im_done: bool,
    stop: bool,
    step: Step,
}

impl Node {
    async fn new(interface_ip: String, port: u16) -> Node {
        Node {
            t: 0,
            n: 0,
            interface_ip,
            timer: Instant::now(),
            connected_node: 0,
            network: Vec::new(),
            dealer: 0,
            index: 0,
            byz_comp: ByzComp::Honnest,
            dealer_sender: None,
            keys: KeyPair::generate(&rcgen::PKCS_ED25519).expect("Failed to construct key"),
            im_setup: false,
            im_done: false,
            shares: ShareSet::new(),
            reconstruct_share_set: ShareSet::new(),
            stop: false,
            port,
            result: ResultFields::new(),
            step: Step::Sharing,
        }
    }

    async fn new_command<'a>(node: Wrapped<Node>, bytes_message: &[u8]) {
        match CommandCode::from(bytes_message[0]) {
            CommandCode::DEALTHIS => Self::setup_deal(node, &bytes_message[1..]).await,
            CommandCode::SHARE => Self::share_receiv(node, &bytes_message[1..]).await,
            CommandCode::ACK => Self::new_sign(node, &bytes_message[1..]).await,
            CommandCode::REST => Self::broadcast_receiv(node, &bytes_message[1..]).await,
            CommandCode::SETUP => Self::setup(node, &bytes_message[1..]).await,
            CommandCode::KEY => Self::new_key(node, &bytes_message[1..]).await,
            CommandCode::RECONSTRUCT => Self::reconstruct(node).await,
            CommandCode::NEWSHARE => Self::new_share(node, &bytes_message[1..]).await,
            CommandCode::STOP => Self::stop_reconstruct(node).await,
        };
    }

    async fn setup(node: Wrapped<Node>, bytes: Bytes<'_>) {
        let mut node = node.lock().await;
        node.im_done = false;
        node.im_setup = false;
        node.step = Step::Sharing;
        node.dealer = LittleEndian::read_u16(bytes);
        node.byz_comp = bytes[2].into();
        node.t = LittleEndian::read_u16(&bytes[3..]);
        let n = LittleEndian::read_u16(&bytes[5..]);
        node.n = n;
        if node.n > node.network.len() as u16 {
            node.connected_node = node.network.len() as u16;
            let (network, _) = read_ip_vec(&bytes[5..]);
            for addr in network.iter().skip(node.network.len()) {
                node.network.push(ExternNode::new(vec![], addr.to_string()));
            }
            if node.connected_node == 0 {
                node.set_index();
            }
            let key = node.encode_key();
            for node in node.network.iter().skip(node.connected_node as usize) {
                private_message(node.addr(), &key).await;
            }
        }
        // if node.im_dealer() {
        //     println!("n: {n}, t: {}", node.t);
        // }
        node.shares.clear();
        node.im_setup = true;
    }

    fn set_index(&mut self) {
        self.index = self
            .network
            .iter()
            .position(|addr| extract_port(addr.addr()) == self.port)
            .unwrap() as u16;
    }

    fn encode_key(&self) -> [u8; 300] {
        let mut buf = [0; 300];
        buf[0] = CommandCode::KEY.into();
        LittleEndian::write_u16(&mut buf[1..], self.index);
        let p_key = self.keys.public_key_der();
        write_bytes(&mut buf[3..], &p_key);
        buf
    }

    fn get_current_network(&self) -> Vec<ExternNode> {
        self.network[0..self.n as usize].to_vec()
    }

    async fn new_key(node: Wrapped<Node>, bytes: Bytes<'_>) {
        loop {
            if node.lock().await.im_setup {
                break;
            }
        }
        let i = LittleEndian::read_u16(bytes);
        let key = trim_buf(&bytes[6..]).to_vec();
        node.lock().await.network[i as usize].p_key = key;
        node.lock().await.connected_node += 1
    }

    async fn im_ready(node: Wrapped<Node>) {
        loop {
            let node = node.lock().await;
            if node.im_setup && node.network.len() <= node.connected_node as usize {
                break;
            }
        }
    }

    async fn setup_deal(node: Wrapped<Node>, bytes: Bytes<'_>) {
        Self::im_ready(node.clone()).await;
        let node_cloned = node.clone();
        let mut node = node.lock().await;
        let s = LittleEndian::read_u128(bytes);
        let dealer_network = node.get_current_network();
        let (sender, receiver) = channel::<DealerMessage>(1000);
        node.dealer_sender = Some(sender);
        let t = node.t;
        tokio::spawn(async move { deal(node_cloned, t, dealer_network, receiver, s).await });
    }

    async fn share_receiv(node: Wrapped<Node>, bytes: Bytes<'_>) {
        Self::im_ready(node.clone()).await;
        node.lock().await.im_setup = false;
        let (proof, mut index) = Proof::read(bytes);
        let (comm, consumed) = Commitment::read(&bytes[index..]);
        index += consumed;
        let (share, _) = Share::read(&bytes[index..]);
        tokio::spawn(async move { first_receiv(node, comm, share, proof).await });
    }

    fn save_share(&mut self, i: u16, share: Share, proof: Proof) {
        self.get_current_set_mut().new_share(i, share, proof);
    }

    fn sign(&self) -> Sign {
        let pkcs8_bytes = self.keys.serialize_der();
        let ed_key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_slice())
            .expect("Failed to parse private key");
        let signature = ed_key_pair.sign(SIGNATURE);
        signature.as_ref().to_vec()
    }

    async fn new_sign(node: Wrapped<Node>, bytes: Bytes<'_>) {
        let i = LittleEndian::read_u16(bytes);
        let sign_size = LittleEndian::read_u32(&bytes[2..]) as usize;
        let sign: Sign = bytes[6..sign_size + 6].to_vec();
        loop {
            if node.lock().await.dealer_sender.is_some() {
                break;
            }
        }
        let _ = node
            .lock()
            .await
            .dealer_sender
            .as_mut()
            .expect("Sender is none")
            .send(DealerMessage::Message(i, sign))
            .await;
    }

    async fn broadcast_receiv(node: Wrapped<Node>, bytes: Bytes<'_>) {
        let (comm, mut index) = Commitment::read(bytes);
        let nb_sign = LittleEndian::read_u32(&bytes[index..]) as usize;
        index += 4;
        let mut signatures: Vec<(u16, Sign)> = Vec::with_capacity(nb_sign);
        for _ in 0..nb_sign {
            let i = LittleEndian::read_u16(&bytes[index..]);
            index += 2;
            let (consumed, sign) = read_vec(trim_buf(&bytes[index..]));
            index += consumed;
            signatures.push((i, sign.to_vec()))
        }
        let nb_share = LittleEndian::read_u32(&bytes[index..]) as usize;
        index += 4;
        let mut missing_shares: Vec<Batch> = Vec::with_capacity(nb_share);
        for _ in 0..nb_share {
            let i = LittleEndian::read_u16(&bytes[index..]);
            index += 2;
            let (proof, consumed) = Proof::read(&bytes[index..]);
            index += consumed;
            let (share, consumed) = Share::read(&bytes[index..]);
            index += consumed;
            missing_shares.push((i, proof, share))
        }
        verify_and_output(node, comm, signatures, missing_shares).await;
    }

    async fn reconstruct(node: Wrapped<Node>) {
        let (network, message): (_, [u8; 300]) = {
            let mut node = node.lock().await;
            node.step = Step::Reconstruct;
            node.reconstruct_share_set = node.shares.clone();
            node.im_done = false;
            node.stop = false;
            node.timer = Instant::now();
            if node.byz_comp == ByzComp::Sleeper {
                return;
            }
            // println!("{}", node.im_setup);
            let mut message = [0; 300];
            message[0] = CommandCode::NEWSHARE.into();
            LittleEndian::write_u16(&mut message[1..], node.index);
            let index = node.my_share().write(&mut message[3..]) + 3;
            node.my_proof().write(&mut message[index..]);
            (node.get_current_network(), message)
        };
        let message = Arc::from(Mutex::from(message.to_vec()));
        for ext_node in network {
            if node.lock().await.stop {
                break;
            }
            let addr = ext_node.addr.clone();
            let message = message.clone();
            tokio::spawn(async move {
                private_message(&addr, &message.lock().await).await;
            });
        }
    }

    async fn new_share(node: Wrapped<Node>, bytes: Bytes<'_>) {
        let output = {
            let mut node = node.lock().await;
            if node.im_done {
                return;
            }
            let i = LittleEndian::read_u16(bytes);
            let (share, index) = Share::read(&bytes[2..]);
            let (proof, _) = Proof::read(&bytes[index + 2..]);
            let mut output = false;
            if !node.get_current_set().set.contains_key(&i)
                && verify(node.get_current_set().get_comm(), i + 1, &share, &proof)
            {
                node.save_share(i, share, proof);
                if node.get_current_set().len() as u16 > 2 * node.t {
                    output = true
                }
            }
            output
        };
        if output {
            node.lock()
                .await
                .output(Step::Reconstruct, ErrorCode::OK)
                .await
        }
    }

    pub async fn stop_reconstruct(node: Wrapped<Node>) {
        node.lock().await.stop = true;
        node.lock()
            .await
            .output(Step::Reconstruct, ErrorCode::OK)
            .await;
    }

    pub async fn output(&mut self, step: Step, code: ErrorCode) {
        if self.im_done {
            return;
        }
        if self.im_dealer() && step == Step::Sharing {
            self.result
                .set(TypeResultField::Total, self.timer.elapsed().as_millis());
        } else if step == Step::Reconstruct {
            self.result.set(
                TypeResultField::Reconstruction,
                self.timer.elapsed().as_millis(),
            );
        }
        let mut msg = [0; RESULT_FIELDS_SIZE + 1];
        msg[0] = InterfaceCode::OUTPUT.into();
        self.im_done = true;
        self.result.extract().to_bytes(&mut msg[1..], code);
        private_message(&self.interface_ip, &msg).await
    }

    pub fn my_share(&self) -> &Share {
        &self.get_current_set().get(self.index).0
    }

    pub fn my_proof(&self) -> &Proof {
        &self.get_current_set().get(self.index).1
    }

    pub fn get_current_set(&self) -> &ShareSet {
        match self.step {
            Step::Sharing => &self.shares,
            Step::Reconstruct => &self.reconstruct_share_set,
        }
    }

    pub fn get_current_set_mut(&mut self) -> &mut ShareSet {
        match self.step {
            Step::Sharing => &mut self.shares,
            Step::Reconstruct => &mut self.reconstruct_share_set,
        }
    }

    pub fn im_dealer(&self) -> bool {
        self.index == self.dealer
    }
}

async fn connect(interface_ip: String, port: u16) {
    let mut buf = [0; 3];
    buf[0] = InterfaceCode::CONNECT.into();
    LittleEndian::write_u16(&mut buf[1..], port);
    private_message(&interface_ip, &buf).await
}

async fn private_message<'a>(addr: &str, msg: Bytes<'a>) {
    TcpStream::connect(addr)
        .await
        .unwrap_or_else(|_| panic!("Invalid node: {addr}"))
        .write_all(msg)
        .await
        .expect("Failed to write")
}

#[tokio::main]
async fn main() {
    begin().await;
}

async fn listen_with(listener: TcpListener, node: Wrapped<Node>) {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let node = node.clone();
        tokio::spawn(async move {
            let mut buf = [0; 20_000];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                Node::new_command(node.clone(), &buf).await;
            }
        });
    }
}

async fn begin() {
    let interface_ip = env::args().nth(1).unwrap();
    let (port, listener) = generate_random_port("127.0.0.1").await;
    let node = Arc::new(Mutex::new(Node::new(interface_ip.clone(), port).await));
    tokio::spawn(async move { connect(interface_ip, port).await });
    listen_with(listener, node).await;
}
