use crate::*;
pub mod kzg10;
pub use kzg10::*;

pub use rcgen::KeyPair;
pub use ring::signature::{Ed25519KeyPair, KeyPair as _, Signature, UnparsedPublicKey, ED25519};
pub static SIGNATURE: &[u8; 19] = b"SIGNATURE OF A NODE";

// pub(crate) use ark_bls12_377::{FrConfig, Bls12_377, Fr};
pub(crate) use ark_bls12_381::{FrConfig, Bls12_381};
pub(crate) use ark_ec::pairing::Pairing;
pub(crate) use ark_ff::{Fp, MontBackend};
pub(crate) use ark_poly::{univariate::DensePolynomial, DenseUVPolynomial};
pub(crate) use ark_poly_commit::Polynomial as ArkPolynomial;
pub(crate) use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress};
pub(crate) use ark_std::test_rng;

pub const BIGINT_SIZE: usize = 4;
pub(crate) type F = Fp<MontBackend<FrConfig, BIGINT_SIZE>, BIGINT_SIZE>;

pub(crate) type E = Bls12_381;
pub(crate) type P = DensePolynomial<F>;


pub type Sign = Vec<u8>;
pub type PublicKey = Vec<u8>;
pub type Witness = i32;
pub type Batch = (u16, Proof, Share);
pub(crate) type ArkCommitment = kzg10::Commitment<E>;
pub(crate) type ArkShare = F;
pub(crate) type ArkProof = kzg10::Proof<E>;

pub fn compute_proof_and_shares(
    n: u16,
    degree_bound: u32,
    secret: u128,
) -> (Commitment, Vec<(Proof, Share)>) {
    let n = n as usize;
    let degree = degree_bound as usize;
    let rng = &mut test_rng();
    let pp = KZG10::<E, P>::setup(degree, false, rng).unwrap();
    let (ck, vk) = KZG10::<E, P>::trim(&pp, degree).unwrap();
    let mut p = P::rand(degree, rng);
    p[0] = Fp::from(secret);
    let hiding_bound = Some(1);
    let (comm, rand) = KZG10::<E, P>::commit(&ck, &p, hiding_bound, Some(rng)).unwrap();
    let output = (1..n + 1)
        .map(|i| {
            let point = Fp::from(i as u16);
            let share = p.evaluate(&point);
            let proof = KZG10::open(&ck, &p, point, &rand).unwrap();
            (Proof { proof }, Share { share })
        })
        .collect::<Vec<_>>();
    (
        Commitment {
            comm,
            vkey: vk,
            degree_bound,
        },
        output,
    )
}
use ark_ff::Field;
use ark_std::Zero;
pub fn interpolate(shares: &HashMap<u16, (Share, Proof)>) -> i128 {
    let points: Vec<(ArkShare, ArkShare)> = shares
        .iter()
        .map(|(i, (s, _))| (F::from(i + 1), s.share))
        .collect();
    let mut result = ArkShare::zero();
    for (i, &(xi, yi)) in points.iter().enumerate() {
        let mut term = yi;
        for (j, &(xj, _)) in points.iter().enumerate() {
            if i != j {
                term *= -xj;
                term *= (xi - xj).inverse().unwrap();
            }
        }
        result += term;
    }
    let mut bytes = [0u8; 32];
    result
        .serialize_uncompressed(&mut bytes[..])
        .expect("Failed to write fr to bytes");
    i128::from_le_bytes(bytes[0..16].try_into().expect("Failed to parse in i128"))
}

#[derive(Clone)]
pub struct Proof {
    proof: ArkProof,
}

impl Proof {
    pub fn write(&self, buf: &mut [u8]) -> usize {
        self.proof
            .serialize_compressed(buf)
            .expect("Failed to serialize");
        self.proof.serialized_size(Compress::Yes)
    }

    pub fn read(buf: &[u8]) -> (Self, usize) {
        let res = Proof {
            proof: ArkProof::deserialize_compressed(buf).expect("Failed to deserialize"),
        };
        let size = res.proof.serialized_size(Compress::Yes);
        (res, size)
    }
}

#[derive(Clone)]
pub struct Share {
    pub share: ArkShare,
}

impl Share {
    pub fn write(&self, buf: &mut [u8]) -> usize {
        self.share
            .serialize_compressed(buf)
            .expect("Failed to serialize");
        self.share.serialized_size(Compress::Yes)
    }

    pub fn read(buf: &[u8]) -> (Self, usize) {
        let res = Share {
            share: ArkShare::deserialize_compressed(buf).expect("Failed to deserialize"),
        };
        let size = res.share.serialized_size(Compress::Yes);
        (res, size)
    }
}

#[derive(Clone)]
pub struct Commitment {
    comm: ArkCommitment,
    degree_bound: u32,
    vkey: VerifierKey<E>,
}

impl Commitment {
    pub fn write(&self, buf: &mut [u8]) -> usize {
        self.vkey
            .serialize_compressed(&mut buf[..])
            .expect("Failed to serialize");
        let vkey_size = self.vkey.serialized_size(Compress::Yes);
        self.comm
            .serialize_compressed(&mut buf[vkey_size..])
            .expect("Failed to serialize");
        let comm_size = self.comm.serialized_size(Compress::Yes);
        LittleEndian::write_u32(&mut buf[vkey_size + comm_size..], self.degree_bound);
        comm_size + vkey_size + 4
    }

    pub fn read(buf: &[u8]) -> (Commitment, usize) {
        let vkey = VerifierKey::<E>::deserialize_compressed(buf).expect("Failed to deserialize");
        let vkey_size = vkey.serialized_size(Compress::Yes);
        let comm = ArkCommitment::deserialize_compressed(&buf[vkey_size..])
            .expect("Failed to deserialize");
        let comm_size = comm.serialized_size(Compress::Yes);
        let degree_bound = LittleEndian::read_u32(&buf[vkey_size + comm_size..]);
        let res = Commitment {
            comm,
            vkey,
            degree_bound,
        };
        (res, comm_size + vkey_size + 4)
    }
}

pub fn deg_check(comm: &Commitment, deg: usize) -> bool {
    comm.degree_bound == deg as u32
}

pub fn is_valid_sign(p_keys: &PublicKey, sign: &Sign) -> bool {
    let raw_public_key = &p_keys[12..];
    let public_key = UnparsedPublicKey::new(&ED25519, raw_public_key);
    public_key.verify(SIGNATURE, sign).is_ok()
}

pub fn verify(comm: &Commitment, index: u16, share: &Share, proof: &Proof) -> bool {
    KZG10::<E, P>::check(&comm.vkey, &comm.comm, index.into(), share.share, &proof.proof).unwrap()
}

pub fn batch_verify(comm: &Commitment, batchs: &Vec<Batch>) -> bool {
    let points: &Vec<<E as Pairing>::ScalarField> = &batchs.iter().map(|(i, _, _)| (*i+1).into()).collect();
    let proofs: &Vec<ArkProof> = &batchs.iter().map(|(_, p, _)| p.proof.clone()).collect();
    let shares: &Vec<ArkShare> = &batchs.iter().map(|(_, _, s)| s.share.clone()).collect();
    KZG10::<E, P>::batch_check(&comm.vkey, &comm.comm, points, shares, proofs, &mut test_rng()).unwrap()
}
