// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::types::*;
use crate::{enum_from_inner, Location};
pub use crypto::keys::slip10::{Chain, ChainCode};
use crypto::{
    ciphers::{
        aes::Aes256Gcm,
        chacha::XChaCha20Poly1305,
        traits::{consts::Unsigned, Aead, Tag},
    },
    hashes::{
        blake2b::Blake2b256,
        sha::{Sha256, Sha384, Sha512, SHA256, SHA256_LEN, SHA384, SHA384_LEN, SHA512, SHA512_LEN},
        Digest,
    },
    keys::{
        bip39,
        pbkdf::{PBKDF2_HMAC_SHA256, PBKDF2_HMAC_SHA384, PBKDF2_HMAC_SHA512},
        slip10, x25519,
    },
    macs::hmac::{HMAC_SHA256, HMAC_SHA384, HMAC_SHA512},
    signatures::ed25519,
    utils::rand::fill,
};
use engine::{runtime::GuardedVec, vault::RecordHint};
use serde::{Deserialize, Serialize};
use std::convert::{From, Into, TryFrom};
use stronghold_utils::GuardDebug;

/// Enum that wraps all cryptographic procedures that are supported by Stronghold.
///  
/// A procedure performs a (cryptographic) operation on a secret in the vault and/
/// or generates a new secret.
///
/// **Note**: For all procedures that write output to the vault, the [`PersistSecret`]
/// trait is implement. **A secret is only permanently stored in the vault, if
/// explicitly specified via [`PersistSecret::write_secret`]. Analogous for procedures with
/// non-secret output, the [`PersistOutput`] is implemented and [`PersistOutput::store_output`]
/// has to be called if the procedure's output should be returned to the user.
#[derive(Clone, GuardDebug, Serialize, Deserialize)]
pub enum PrimitiveProcedure {
    CopyRecord(CopyRecord),
    Slip10Generate(Slip10Generate),
    Slip10Derive(Slip10Derive),
    BIP39Generate(BIP39Generate),
    BIP39Recover(BIP39Recover),
    PublicKey(PublicKey),
    GenerateKey(GenerateKey),
    Ed25519Sign(Ed25519Sign),
    X25519DiffieHellman(X25519DiffieHellman),
    Hash(Hash),
    Hmac(Hmac),
    Hkdf(Hkdf),
    Pbkdf2Hmac(Pbkdf2Hmac),
    AeadEncrypt(AeadEncrypt),
    AeadDecrypt(AeadDecrypt),
}

impl ProcedureStep for PrimitiveProcedure {
    type Output = ProcedureIo;
    fn execute<R: Runner>(self, runner: &mut R) -> Result<Self::Output, ProcedureError> {
        use PrimitiveProcedure::*;
        match self {
            CopyRecord(proc) => proc.execute(runner).map(|o| o.into()),
            Slip10Generate(proc) => proc.execute(runner).map(|o| o.into()),
            Slip10Derive(proc) => proc.execute(runner).map(|o| o.into()),
            BIP39Generate(proc) => proc.execute(runner).map(|o| o.into()),
            BIP39Recover(proc) => proc.execute(runner).map(|o| o.into()),
            GenerateKey(proc) => proc.execute(runner).map(|o| o.into()),
            PublicKey(proc) => proc.execute(runner).map(|o| o.into()),
            Ed25519Sign(proc) => proc.execute(runner).map(|o| o.into()),
            X25519DiffieHellman(proc) => proc.execute(runner).map(|o| o.into()),
            Hash(proc) => proc.execute(runner).map(|o| o.into()),
            Hmac(proc) => proc.execute(runner).map(|o| o.into()),
            Hkdf(proc) => proc.execute(runner).map(|o| o.into()),
            Pbkdf2Hmac(proc) => proc.execute(runner).map(|o| o.into()),
            AeadEncrypt(proc) => proc.execute(runner).map(|o| o.into()),
            AeadDecrypt(proc) => proc.execute(runner).map(|o| o.into()),
        }
    }
}

impl PrimitiveProcedure {
    pub fn output(&self) -> Option<Location> {
        match self {
            PrimitiveProcedure::CopyRecord(CopyRecord { output, .. })
            | PrimitiveProcedure::Slip10Generate(Slip10Generate { output, .. })
            | PrimitiveProcedure::Slip10Derive(Slip10Derive { output, .. })
            | PrimitiveProcedure::BIP39Generate(BIP39Generate { output, .. })
            | PrimitiveProcedure::BIP39Recover(BIP39Recover { output, .. })
            | PrimitiveProcedure::GenerateKey(GenerateKey { output, .. })
            | PrimitiveProcedure::X25519DiffieHellman(X25519DiffieHellman { shared_key: output, .. })
            | PrimitiveProcedure::Hkdf(Hkdf { okm: output, .. })
            | PrimitiveProcedure::Pbkdf2Hmac(Pbkdf2Hmac { output, .. }) => Some(output.clone()),
            _ => None,
        }
    }
}

// === implement From Traits from inner types to wrapper enums

enum_from_inner!(PrimitiveProcedure::CopyRecord from CopyRecord);
enum_from_inner!(PrimitiveProcedure::Slip10Generate from Slip10Generate);
enum_from_inner!(PrimitiveProcedure::Slip10Derive from Slip10Derive);
enum_from_inner!(PrimitiveProcedure::BIP39Generate from BIP39Generate);
enum_from_inner!(PrimitiveProcedure::BIP39Recover from BIP39Recover);
enum_from_inner!(PrimitiveProcedure::GenerateKey from GenerateKey);
enum_from_inner!(PrimitiveProcedure::PublicKey from PublicKey);
enum_from_inner!(PrimitiveProcedure::Ed25519Sign from Ed25519Sign);
enum_from_inner!(PrimitiveProcedure::X25519DiffieHellman from X25519DiffieHellman);
enum_from_inner!(PrimitiveProcedure::Hash from Hash);
enum_from_inner!(PrimitiveProcedure::Hmac from Hmac);
enum_from_inner!(PrimitiveProcedure::Hkdf from Hkdf);
enum_from_inner!(PrimitiveProcedure::Pbkdf2Hmac from Pbkdf2Hmac);
enum_from_inner!(PrimitiveProcedure::AeadEncrypt from AeadEncrypt);
enum_from_inner!(PrimitiveProcedure::AeadDecrypt from AeadDecrypt);

// ==========================
// Helper Procedure
// ==========================

/// Copy the content of a record from one location to another.
///
/// Note: This does not remove the old record. Users that would like to move the record instead
/// of just copying it, should run `Stronghold::delete_data` on the old location **after** this
/// procedure was executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyRecord {
    pub input: Location,

    pub output: Location,

    pub hint: RecordHint,
}

impl DeriveSecret for CopyRecord {
    type Output = ();

    fn derive(self, guard: GuardedVec<u8>) -> Result<Products<()>, FatalProcedureError> {
        let products = Products {
            secret: (*guard.borrow()).to_vec(),
            output: (),
        };
        Ok(products)
    }

    fn source(&self) -> &Location {
        &self.input
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

// ==========================
// Procedures for Cryptographic Primitives
// ==========================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MnemonicLanguage {
    English,
    Japanese,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AeadAlg {
    Aes256Gcm,
    XChaCha20Poly1305,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyType {
    Ed25519,
    X25519,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashType {
    Blake2b,
    Sha2(Sha2Hash),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Sha2Hash {
    Sha256,
    Sha384,
    Sha512,
}

/// Generate a BIP39 seed and its corresponding mnemonic sentence (optionally protected by a
/// passphrase). Store the seed and return the mnemonic sentence as data output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BIP39Generate {
    pub passphrase: Option<String>,

    pub language: MnemonicLanguage,

    pub output: Location,

    pub hint: RecordHint,
}

impl GenerateSecret for BIP39Generate {
    type Output = String;

    fn generate(self) -> Result<Products<Self::Output>, FatalProcedureError> {
        let mut entropy = [0u8; 32];
        fill(&mut entropy)?;

        let wordlist = match self.language {
            MnemonicLanguage::English => bip39::wordlist::ENGLISH,
            MnemonicLanguage::Japanese => bip39::wordlist::JAPANESE,
        };

        let mnemonic = bip39::wordlist::encode(&entropy, &wordlist).unwrap();

        let mut seed = [0u8; 64];
        let passphrase = self.passphrase.unwrap_or_else(|| "".into());
        bip39::mnemonic_to_seed(&mnemonic, &passphrase, &mut seed);

        Ok(Products {
            secret: seed.to_vec(),
            output: mnemonic,
        })
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

/// Use a BIP39 mnemonic sentence (optionally protected by a passphrase) to create or recover
/// a BIP39 seed and store it in the `output` location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BIP39Recover {
    pub passphrase: Option<String>,

    pub mnemonic: String,

    pub output: Location,

    pub hint: RecordHint,
}

impl GenerateSecret for BIP39Recover {
    type Output = ();

    fn generate(self) -> Result<Products<Self::Output>, FatalProcedureError> {
        let mut seed = [0u8; 64];
        let passphrase = self.passphrase.unwrap_or_else(|| "".into());
        bip39::mnemonic_to_seed(&self.mnemonic, &passphrase, &mut seed);
        Ok(Products {
            secret: seed.to_vec(),
            output: (),
        })
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

/// Generate a raw SLIP10 seed of the specified size (in bytes, defaults to 64 bytes/512 bits) and store it in
/// the `output` location
///
/// Note that this does not generate a BIP39 mnemonic sentence and it's not possible to
/// generate one: use `BIP39Generate` if a mnemonic sentence will be required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slip10Generate {
    pub size_bytes: Option<usize>,

    pub output: Location,

    pub hint: RecordHint,
}

impl GenerateSecret for Slip10Generate {
    type Output = ();

    fn generate(self) -> Result<Products<Self::Output>, FatalProcedureError> {
        let size_bytes = self.size_bytes.unwrap_or(64);
        let mut seed = vec![0u8; size_bytes];
        fill(&mut seed)?;
        Ok(Products {
            secret: seed,
            output: (),
        })
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

#[derive(GuardDebug, Clone, Serialize, Deserialize)]
pub enum SLIP10DeriveInput {
    /// Note that BIP39 seeds are allowed to be used as SLIP10 seeds
    Seed(Location),
    Key(Location),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Slip10ParentType {
    Seed,
    Key,
}

/// Derive a SLIP10 child key from a seed or a parent key, store it in output location and
/// return the corresponding chain code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slip10Derive {
    pub chain: Chain,

    pub parent_ty: Slip10ParentType,

    pub input: Location,

    pub output: Location,

    pub hint: RecordHint,
}

impl DeriveSecret for Slip10Derive {
    type Output = ChainCode;

    fn derive(self, guard: GuardedVec<u8>) -> Result<Products<ChainCode>, FatalProcedureError> {
        let dk = match self.parent_ty {
            Slip10ParentType::Key => {
                slip10::Key::try_from(&*guard.borrow()).and_then(|parent| parent.derive(&self.chain))
            }
            Slip10ParentType::Seed => {
                slip10::Seed::from_bytes(&guard.borrow()).derive(slip10::Curve::Ed25519, &self.chain)
            }
        }?;
        Ok(Products {
            secret: dk.into(),
            output: dk.chain_code(),
        })
    }

    fn source(&self) -> &Location {
        &self.input
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

fn x25519_secret_key(guard: GuardedVec<u8>) -> Result<x25519::SecretKey, crypto::Error> {
    let raw = guard.borrow();
    let raw = (*raw).to_vec();
    if raw.len() != x25519::SECRET_KEY_LENGTH {
        let e = crypto::Error::BufferSize {
            has: raw.len(),
            needs: x25519::SECRET_KEY_LENGTH,
            name: "data buffer",
        };
        return Err(e);
    }
    x25519::SecretKey::try_from_slice(&raw)
}

fn ed25519_secret_key(guard: GuardedVec<u8>) -> Result<ed25519::SecretKey, crypto::Error> {
    let raw = guard.borrow();
    let mut raw = (*raw).to_vec();
    if raw.len() < ed25519::SECRET_KEY_LENGTH {
        let e = crypto::Error::BufferSize {
            has: raw.len(),
            needs: ed25519::SECRET_KEY_LENGTH,
            name: "data buffer",
        };
        return Err(e);
    }
    raw.truncate(ed25519::SECRET_KEY_LENGTH);
    let mut bs = [0; ed25519::SECRET_KEY_LENGTH];
    bs.copy_from_slice(&raw);

    Ok(ed25519::SecretKey::from_bytes(bs))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateKey {
    pub ty: KeyType,

    pub output: Location,

    pub hint: RecordHint,
}

impl GenerateSecret for GenerateKey {
    type Output = ();

    fn generate(self) -> Result<Products<Self::Output>, FatalProcedureError> {
        let secret = match self.ty {
            KeyType::Ed25519 => ed25519::SecretKey::generate().map(|sk| sk.to_bytes().to_vec())?,
            KeyType::X25519 => x25519::SecretKey::generate().map(|sk| sk.to_bytes().to_vec())?,
        };
        Ok(Products { secret, output: () })
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

/// Derive an Ed25519 public key from the corresponding private key stored at the specified
/// location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub ty: KeyType,

    pub private_key: Location,
}

impl UseSecret for PublicKey {
    type Output = Vec<u8>;

    fn use_secret(self, guard: GuardedVec<u8>) -> Result<Self::Output, FatalProcedureError> {
        match self.ty {
            KeyType::Ed25519 => {
                let sk = ed25519_secret_key(guard)?;
                Ok(sk.public_key().to_bytes().to_vec())
            }
            KeyType::X25519 => {
                let sk = x25519_secret_key(guard)?;
                Ok(sk.public_key().to_bytes().to_vec())
            }
        }
    }

    fn source(&self) -> &Location {
        &self.private_key
    }
}

/// Use the specified Ed25519 compatible key to sign the given message
///
/// Compatible keys are any record that contain the desired key material in the first 32 bytes,
/// in particular SLIP10 keys are compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ed25519Sign {
    pub msg: Vec<u8>,

    pub private_key: Location,
}

impl UseSecret for Ed25519Sign {
    type Output = [u8; ed25519::SIGNATURE_LENGTH];

    fn use_secret(self, guard: GuardedVec<u8>) -> Result<Self::Output, FatalProcedureError> {
        let sk = ed25519_secret_key(guard)?;
        let sig = sk.sign(&self.msg);
        Ok(sig.to_bytes())
    }

    fn source(&self) -> &Location {
        &self.private_key
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X25519DiffieHellman {
    pub public_key: [u8; x25519::PUBLIC_KEY_LENGTH],

    pub private_key: Location,

    pub shared_key: Location,

    pub hint: RecordHint,
}

impl DeriveSecret for X25519DiffieHellman {
    type Output = ();

    fn derive(self, guard: GuardedVec<u8>) -> Result<Products<()>, FatalProcedureError> {
        let sk = x25519_secret_key(guard)?;
        let public = x25519::PublicKey::from_bytes(self.public_key);
        let shared_key = sk.diffie_hellman(&public);

        Ok(Products {
            secret: shared_key.to_bytes().to_vec(),
            output: (),
        })
    }

    fn source(&self) -> &Location {
        &self.private_key
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.shared_key, self.hint)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hash {
    pub ty: HashType,

    pub msg: Vec<u8>,
}

impl ProcessData for Hash {
    type Output = Vec<u8>;

    fn process(self) -> Result<Self::Output, FatalProcedureError> {
        match self.ty {
            HashType::Blake2b => {
                let mut digest = [0; <Blake2b256 as Digest>::OutputSize::USIZE];
                digest.copy_from_slice(&Blake2b256::digest(&self.msg));
                Ok(digest.to_vec())
            }
            HashType::Sha2(Sha2Hash::Sha256) => {
                let mut digest = [0; SHA256_LEN];
                SHA256(&self.msg, &mut digest);
                Ok(digest.to_vec())
            }
            HashType::Sha2(Sha2Hash::Sha384) => {
                let mut digest = [0; SHA384_LEN];
                SHA384(&self.msg, &mut digest);
                Ok(digest.to_vec())
            }
            HashType::Sha2(Sha2Hash::Sha512) => {
                let mut digest = [0; SHA512_LEN];
                SHA512(&self.msg, &mut digest);
                Ok(digest.to_vec())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hmac {
    pub ty: Sha2Hash,

    pub msg: Vec<u8>,

    pub key: Location,
}

impl UseSecret for Hmac {
    type Output = Vec<u8>;

    fn use_secret(self, guard: GuardedVec<u8>) -> Result<Self::Output, FatalProcedureError> {
        match self.ty {
            Sha2Hash::Sha256 => {
                let mut mac = [0; SHA256_LEN];
                HMAC_SHA256(&self.msg, &*guard.borrow(), &mut mac);
                Ok(mac.to_vec())
            }
            Sha2Hash::Sha384 => {
                let mut mac = [0; SHA384_LEN];
                HMAC_SHA384(&self.msg, &*guard.borrow(), &mut mac);
                Ok(mac.to_vec())
            }
            Sha2Hash::Sha512 => {
                let mut mac = [0; SHA512_LEN];
                HMAC_SHA512(&self.msg, &*guard.borrow(), &mut mac);
                Ok(mac.to_vec())
            }
        }
    }

    fn source(&self) -> &Location {
        &self.key
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hkdf {
    pub ty: Sha2Hash,

    pub salt: Vec<u8>,

    pub label: Vec<u8>,

    pub ikm: Location,

    pub okm: Location,

    pub hint: RecordHint,
}

impl DeriveSecret for Hkdf {
    type Output = ();

    fn derive(self, guard: GuardedVec<u8>) -> Result<Products<()>, FatalProcedureError> {
        let secret = match self.ty {
            Sha2Hash::Sha256 => {
                let mut okm = [0; SHA256_LEN];
                hkdf::Hkdf::<Sha256>::new(Some(&self.salt), &*guard.borrow())
                    .expand(&self.label, &mut okm)
                    .expect("okm is the correct length");
                okm.to_vec()
            }
            Sha2Hash::Sha384 => {
                let mut okm = [0; SHA384_LEN];
                hkdf::Hkdf::<Sha384>::new(Some(&self.salt), &*guard.borrow())
                    .expand(&self.label, &mut okm)
                    .expect("okm is the correct length");
                okm.to_vec()
            }
            Sha2Hash::Sha512 => {
                let mut okm = [0; SHA512_LEN];
                hkdf::Hkdf::<Sha512>::new(Some(&self.salt), &*guard.borrow())
                    .expand(&self.label, &mut okm)
                    .expect("okm is the correct length");
                okm.to_vec()
            }
        };
        Ok(Products { secret, output: () })
    }

    fn source(&self) -> &Location {
        &self.ikm
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.okm, self.hint)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pbkdf2Hmac {
    pub ty: Sha2Hash,

    pub password: Vec<u8>,

    pub salt: Vec<u8>,

    pub count: u32,

    pub output: Location,

    pub hint: RecordHint,
}

impl GenerateSecret for Pbkdf2Hmac {
    type Output = ();

    fn generate(self) -> Result<Products<Self::Output>, FatalProcedureError> {
        let secret;
        match self.ty {
            Sha2Hash::Sha256 => {
                let mut buffer = [0; SHA256_LEN];
                PBKDF2_HMAC_SHA256(&self.password, &self.salt, self.count as usize, &mut buffer)?;
                secret = buffer.to_vec()
            }
            Sha2Hash::Sha384 => {
                let mut buffer = [0; SHA384_LEN];
                PBKDF2_HMAC_SHA384(&self.password, &self.salt, self.count as usize, &mut buffer)?;
                secret = buffer.to_vec()
            }
            Sha2Hash::Sha512 => {
                let mut buffer = [0; SHA512_LEN];
                PBKDF2_HMAC_SHA512(&self.password, &self.salt, self.count as usize, &mut buffer)?;
                secret = buffer.to_vec()
            }
        }
        Ok(Products { secret, output: () })
    }

    fn target(&self) -> (&Location, RecordHint) {
        (&self.output, self.hint)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeadEncrypt {
    pub alg: AeadAlg,

    pub associated_data: Vec<u8>,

    pub plaintext: Vec<u8>,

    // **Note**: The nonce is required to have length [`Aes256Gcm::NONCE_LENGTH`] /
    /// [`XChaCha20Poly1305::NONCE_LENGTH`], (depending on the [`AeadAlg`])
    pub nonce: Vec<u8>,

    pub key: Location,
}

impl UseSecret for AeadEncrypt {
    type Output = Vec<u8>;

    fn use_secret(self, guard: GuardedVec<u8>) -> Result<Self::Output, FatalProcedureError> {
        let mut ctx = vec![0; self.plaintext.len()];

        let f = match self.alg {
            AeadAlg::Aes256Gcm => Aes256Gcm::try_encrypt,
            AeadAlg::XChaCha20Poly1305 => XChaCha20Poly1305::try_encrypt,
        };
        let mut t = match self.alg {
            AeadAlg::Aes256Gcm => Tag::<Aes256Gcm>::default(),
            AeadAlg::XChaCha20Poly1305 => Tag::<XChaCha20Poly1305>::default(),
        };
        f(
            &*guard.borrow(),
            &self.nonce,
            &self.associated_data,
            &self.plaintext,
            &mut ctx,
            &mut t,
        )?;
        let mut output = Vec::with_capacity(t.len() + ctx.len());
        output.extend(t);
        output.extend(ctx);
        Ok(output)
    }

    fn source(&self) -> &Location {
        &self.key
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeadDecrypt {
    pub alg: AeadAlg,

    pub associated_data: Vec<u8>,

    pub ciphertext: Vec<u8>,

    pub tag: Vec<u8>,

    pub nonce: Vec<u8>,

    pub key: Location,
}

impl UseSecret for AeadDecrypt {
    type Output = Vec<u8>;

    fn use_secret(self, guard: GuardedVec<u8>) -> Result<Self::Output, FatalProcedureError> {
        let mut ptx = vec![0; self.ciphertext.len()];

        let f = match self.alg {
            AeadAlg::Aes256Gcm => Aes256Gcm::try_decrypt,
            AeadAlg::XChaCha20Poly1305 => XChaCha20Poly1305::try_decrypt,
        };
        f(
            &*guard.borrow(),
            &self.nonce,
            &self.associated_data,
            &mut ptx,
            &self.ciphertext,
            &self.tag,
        )?;
        Ok(ptx)
    }

    fn source(&self) -> &Location {
        &self.key
    }
}
