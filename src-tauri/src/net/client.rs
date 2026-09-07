use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use rustls_pki_types::PrivateKeyDer;

use crate::crypto::Identity;
use crate::store::StoredDevice;

#[derive(Debug)]
struct PinVerifier {
    pin: Option<Vec<u8>>,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        if let Some(pin) = &self.pin {
            if end_entity.as_ref() != pin.as_slice() {
                return Err(TlsError::General("trust_broken".into()));
            }
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn client_config(pin: Option<Vec<u8>>) -> Result<ClientConfig, String> {
    let verifier = Arc::new(PinVerifier { pin });
    let cfg = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(cfg)
}

pub fn http_client(pin: Option<Vec<u8>>) -> Result<reqwest::Client, String> {
    let cfg = client_config(pin)?;
    reqwest::Client::builder()
        .use_preconfigured_tls(cfg)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

pub fn pin_from_device(d: &StoredDevice) -> Result<Vec<u8>, String> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &d.cert_der)
        .map_err(|e| format!("cert: {e}"))
}

pub fn signed_headers(identity: &Identity, body: &[u8]) -> Vec<(String, String)> {
    let ts = crate::store::now_ms() as u64;
    let sig = identity.sign_request(ts, body);
    vec![
        ("x-lanpaste-instance".into(), identity.instance_id.clone()),
        ("x-lanpaste-ts".into(), ts.to_string()),
        ("x-lanpaste-sig".into(), sig),
    ]
}

pub fn host_url(host: &str, port: u16, path: &str) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    format!("https://{host}:{port}{path}")
}

pub fn rustls_server_config(identity: &Identity) -> Result<Arc<rustls::ServerConfig>, String> {
    let cert = CertificateDer::from(identity.cert_der.clone());
    let key = PrivateKeyDer::try_from(identity.key_der.clone())
        .map_err(|e| format!("tls key: {e}"))?;
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(|e| format!("tls server: {e}"))?;
    Ok(Arc::new(cfg))
}

pub fn is_pin_mismatch(err: &str) -> bool {
    err.contains("trust_broken") || err.contains("certificate pin")
}
