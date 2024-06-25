use crate::*;

pub const MESSAGE_SIZE: usize = 1500;

pub async fn broadcast<'a>(network: &Vec<ExternNode>, msg: Bytes<'a>) {
    let message = Arc::from(Mutex::from(msg.to_vec()));
    for node in network {
        let addr = node.addr.clone();
        let message = message.clone();
        tokio::spawn(async move {
            private_message(&addr, &message.lock().await).await;
        });
    }
}

pub async fn deal(
    node: Wrapped<Node>,
    t: u16,
    network: Vec<ExternNode>,
    mut receiver: Receiver<DealerMessage>,
    secret: u128,
) {
    let start = Instant::now();
    node.lock().await.timer = Instant::now();
    let n = network.len() as u16;
    // println!("n = {n}, t = {t}");
    let (comm, mut output) = compute_proof_and_shares(n, 2 * t as u32, secret);
    let mut shares = HashMap::<u16, Batch>::new();
    // println!("DEALING: Computing messages...");
    let mut comm_parsed: [u8; MESSAGE_SIZE] = [0; MESSAGE_SIZE];
    let comm_size = comm.write(&mut comm_parsed);
    let comm_parsed = &comm_parsed[..comm_size];
    let messages = (0..n)
        .rev()
        .map(|i| {
            let (proof, share) = output.pop().unwrap();
            let mut buf = [0; MESSAGE_SIZE * 2];
            buf[0] = CommandCode::SHARE.into();
            let mut index = proof.write(&mut buf[1..]) + 1;
            for b in comm_parsed {
                buf[index] = *b;
                index += 1;
            }
            share.write(&mut buf[index..]);
            shares.insert(i, (i, proof, share));
            (i as usize, buf)
        })
        .collect::<Vec<(usize, [u8; MESSAGE_SIZE * 2])>>();
    node.lock().await.result.set(
        TypeResultField::MessagesComputing,
        start.elapsed().as_millis(),
    );
    for (i, msg) in &messages {
        private_message(network[*i].addr(), msg).await;
    }
    let mut signatures = Vec::<(u16, Sign)>::new();
    let deux_t_plus_un = 2 * t + 1;
    loop {
        match receiver.recv().await {
            Some(m) => match m {
                DealerMessage::Message(i, sign)
                    if is_valid_sign(&network[i as usize].p_key, &sign) =>
                {
                    shares.remove(&i);
                    signatures.push((i, sign));
                    if signatures.len() == deux_t_plus_un as usize {
                        break;
                    }
                },
                _ => (),
            },
            None => panic!("error during receiving phase"),
        }
    }    
    let broadcast_timer = Instant::now();
    let missing_shares = shares
        .values()
        .map(|(i, p, s)| (*i, p.clone(), s.clone()))
        .collect::<Vec<Batch>>();
    let mut buf = vec![0; BASE_CAPACITY];
    let mut index = 1;
    buf[0] = CommandCode::REST.into();
    index += comm.write(&mut buf[index..]);
    LittleEndian::write_u32(&mut buf[index..], signatures.len() as u32);
    index += 4;
    for (i, sign) in signatures.iter() {
        LittleEndian::write_u16(&mut buf[index..], *i);
        index += 2;
        index += write_vec(&mut buf, index, sign) as usize;
    }
    LittleEndian::write_u32(&mut buf[index..], missing_shares.len() as u32);
    index += 4;
    for (i, proof, share) in missing_shares.iter() {
        LittleEndian::write_u16(&mut buf[index..], *i);
        index += 2;
        index += proof.write(&mut buf[index..]);
        index += share.write(&mut buf[index..]);
        if (buf.len() as f32 * 0.8) as usize <= index {
            buf.append(&mut vec![0; BASE_CAPACITY])
        }
    }
    node.lock()
        .await
        .result
        .set(TypeResultField::Dealing, start.elapsed().as_millis());
    node.lock().await.result.set(
        TypeResultField::BroadCasting,
        broadcast_timer.elapsed().as_millis(),
    );
    broadcast(&network, trim_buf(&buf)).await;
    // println!("------------------------------------------------------------------------------------------------------------------");
    // println!("DEALING TIME: {:?}", start.elapsed());
    // println!("------------------------------------------------------------------------------------------------------------------");
}

pub async fn verify_and_output(
    node: Wrapped<Node>,
    comm: Commitment,
    signatures: Vec<(u16, Sign)>,
    missing_shares: Vec<Batch>,
) {
    let mut node = node.lock().await;
    let start = Instant::now();
    let mut shares_set: Vec<bool> = vec![false; node.n as usize];
    if signatures.len() as u16 != node.t*2 + 1 {
        node.output(Step::Sharing, ErrorCode::UnvalidSigns).await;
        println!("ERROR 1");
        return;
    }
    for (i, sign) in signatures {
        if !is_valid_sign(&node.network[i as usize].p_key, &sign) || shares_set[i as usize] {
            node.output(Step::Sharing, ErrorCode::UnvalidSigns).await;
            return;
        }
        shares_set[i as usize] = true;
    }
    if !batch_verify(&comm, &missing_shares) {
        println!("ERROR 2");
        node.output(Step::Sharing, ErrorCode::UnvalidShares).await;
        return;
    }
    for (i, p, s) in missing_shares {
        if shares_set[i as usize] {
            eprintln!("ERROR: 3");
            node.output(Step::Sharing, ErrorCode::IncoherentBatch).await;
            return;
        }
        shares_set[i as usize] = true;
        node.save_share(i, s, p);
    }
    if shares_set.contains(&false) {
        eprintln!("ERROR: 4");
        node.output(Step::Sharing, ErrorCode::MissingShare).await;
        return;
    }
    node.get_current_set_mut().set_comm(comm);
    // println!("output of {}: {:?}, duration: {:?}", node.index, node.my_share().share, start.elapsed());
    node.result
        .set(TypeResultField::Verify, start.elapsed().as_millis());
    node.output(Step::Sharing, ErrorCode::OK).await;
}

pub async fn first_receiv(node: Wrapped<Node>, comm: Commitment, share: Share, proof: Proof) {
    let start = Instant::now();
    let mut node = node.lock().await;
    if node.byz_comp == ByzComp::Sleeper {
        return;
    }
    if deg_check(&comm, 2 * node.t as usize) && verify(&comm, node.index + 1, &share, &proof) {
        let sign = node.sign();
        let mut buf = [0; BASE_CAPACITY];
        buf[0] = CommandCode::ACK.into();
        LittleEndian::write_u16(&mut buf[1..], node.index);
        write_bytes(&mut buf[3..], &sign);
        private_message(node.network[node.dealer as usize].addr(), &buf).await;
        let i = node.index;
        node.save_share(i, share, proof);
        // println!(
        //     "Node {} fisnished to sign, duration: {:?}",
        //     node.index,
        //     start.elapsed()
        // );
    } else {
        println!("Node {}: I received invalid share.", node.index);
    }
    node.result
        .set(TypeResultField::FirstReceiv, start.elapsed().as_millis());
}
