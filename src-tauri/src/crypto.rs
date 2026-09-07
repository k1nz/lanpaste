use std::fs;
use std::path::{Path, PathBuf};

use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ED25519};
use sha2::{Digest, Sha256};

const KEY_FILE: &str = "identity.p8";
const CERT_FILE: &str = "identity.crt";
const ID_FILE: &str = "instance_id";

#[derive(Clone)]
pub struct Identity {
    pub instance_id: String,
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub fingerprint: String,
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

impl Identity {
    pub fn load_or_create(dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| format!("identity dir: {e}"))?;
        let key_path = dir.join(KEY_FILE);
        let cert_path = dir.join(CERT_FILE);
        let id_path = dir.join(ID_FILE);

        if key_path.exists() && cert_path.exists() && id_path.exists() {
            return Self::load(&key_path, &cert_path, &id_path);
        }
        Self::create(&key_path, &cert_path, &id_path)
    }

    fn load(key_path: &Path, cert_path: &Path, id_path: &Path) -> Result<Self, String> {
        let key_der = fs::read(key_path).map_err(|e| format!("read identity key: {e}"))?;
        let cert_der = fs::read(cert_path).map_err(|e| format!("read identity cert: {e}"))?;
        let instance_id = fs::read_to_string(id_path)
            .map_err(|e| format!("read instance id: {e}"))?
            .trim()
            .to_string();
        let signing_key = SigningKey::from_pkcs8_der(&key_der)
            .map_err(|e| format!("parse identity key: {e}"))?;
        Ok(Self::from_parts(instance_id, signing_key, cert_der, key_der))
    }

    fn create(key_path: &Path, cert_path: &Path, id_path: &Path) -> Result<Self, String> {
        let instance_id = uuid::Uuid::new_v4().to_string();
        let rcgen_key =
            KeyPair::generate_for(&PKCS_ED25519).map_err(|e| format!("tls key: {e}"))?;
        let key_der = rcgen_key.serialize_der();
        let signing_key =
            SigningKey::from_pkcs8_der(&key_der).map_err(|e| format!("dalek key: {e}"))?;

        let mut params = CertificateParams::new(vec![
            "lanpaste.local".to_string(),
            instance_id.clone(),
        ])
        .map_err(|e| format!("cert params: {e}"))?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("LanPaste {instance_id}"));
        params.distinguished_name = dn;
        let cert = params
            .self_signed(&rcgen_key)
            .map_err(|e| format!("self-sign: {e}"))?;
        let cert_der = cert.der().to_vec();

        fs::write(key_path, &key_der).map_err(|e| format!("write key: {e}"))?;
        fs::write(cert_path, &cert_der).map_err(|e| format!("write cert: {e}"))?;
        fs::write(id_path, &instance_id).map_err(|e| format!("write id: {e}"))?;

        Ok(Self::from_parts(instance_id, signing_key, cert_der, key_der))
    }

    fn from_parts(
        instance_id: String,
        signing_key: SigningKey,
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
    ) -> Self {
        let verifying_key = signing_key.verifying_key();
        let fingerprint = fingerprint_of(&verifying_key);
        Self {
            instance_id,
            signing_key,
            verifying_key,
            fingerprint,
            cert_der,
            key_der,
        }
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.as_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> String {
        hex::encode(self.signing_key.sign(message).to_bytes())
    }

    pub fn sign_request(&self, ts: u64, body: &[u8]) -> String {
        self.sign(&request_message(&self.instance_id, ts, body))
    }
}

pub fn fingerprint_of(key: &VerifyingKey) -> String {
    let digest = Sha256::digest(key.as_bytes());
    hex::encode(digest)
}

pub fn fingerprint_from_hex_pubkey(hex_key: &str) -> Result<String, String> {
    let bytes = hex::decode(hex_key).map_err(|e| format!("pubkey hex: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "pubkey must be 32 bytes".to_string())?;
    let vk = VerifyingKey::from_bytes(&arr).map_err(|e| format!("pubkey: {e}"))?;
    Ok(fingerprint_of(&vk))
}

pub fn request_message(instance_id: &str, ts: u64, body: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(instance_id.as_bytes());
    hasher.update(b"|");
    hasher.update(ts.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(Sha256::digest(body));
    hasher.finalize().to_vec()
}

pub fn verify_request(
    public_key_hex: &str,
    instance_id: &str,
    ts: u64,
    body: &[u8],
    sig_hex: &str,
) -> Result<(), String> {
    let pk_bytes = hex::decode(public_key_hex).map_err(|_| "device_removed".to_string())?;
    let arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| "device_removed".to_string())?;
    let vk = VerifyingKey::from_bytes(&arr).map_err(|_| "device_removed".to_string())?;
    let sig_bytes = hex::decode(sig_hex).map_err(|_| "device_removed".to_string())?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "device_removed".to_string())?;
    let sig = Signature::from_bytes(&sig_arr);
    vk.verify(&request_message(instance_id, ts, body), &sig)
        .map_err(|_| "device_removed".to_string())
}

pub fn parse_verifying_key(hex_key: &str) -> Result<VerifyingKey, String> {
    let bytes = hex::decode(hex_key).map_err(|e| format!("pubkey: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "pubkey must be 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&arr).map_err(|e| format!("pubkey: {e}"))
}

/// Generate a 6-digit pairing token.
pub fn generate_pair_token() -> String {
    use rand::Rng;
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{n:06}")
}

#[allow(dead_code)]
pub fn identity_paths(dir: &PathBuf) -> (PathBuf, PathBuf, PathBuf) {
    (
        dir.join(KEY_FILE),
        dir.join(CERT_FILE),
        dir.join(ID_FILE),
    )
}
