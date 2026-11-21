use crate::dto::{SignedCatalogItem, SignedJob};
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("Invalid base64 encoding for {field}: {detail}")]
    InvalidBase64 { field: &'static str, detail: String },

    #[error("Invalid Ed25519 public key length or curve point (expected 32 raw bytes)")]
    InvalidPublicKey,

    #[error("Invalid Ed25519 signature length (expected 64 raw bytes)")]
    InvalidSignatureFormat,

    #[error("Ed25519 signature verification failed")]
    SignatureVerificationFailed,

    #[error("Signed job has expired (expires_at={expires_at}, now={now})")]
    JobExpired { expires_at: i64, now: i64 },

    #[error("Device ID mismatch in signed payload (expected={expected}, actual={actual})")]
    DeviceIdMismatch { expected: String, actual: String },

    #[error("SHA-256 hash mismatch (expected={expected}, actual={actual})")]
    Sha256Mismatch { expected: String, actual: String },

    #[error("I/O error while hashing file: {0}")]
    Io(String),
}

pub fn decode_public_key_b64(public_key_b64: &str) -> Result<VerifyingKey, CryptoError> {
    let raw = BASE64_STD
        .decode(public_key_b64.trim())
        .map_err(|e| CryptoError::InvalidBase64 {
            field: "server_public_key_b64",
            detail: e.to_string(),
        })?;
    let key_bytes: [u8; 32] = raw
        .try_into()
        .map_err(|_| CryptoError::InvalidPublicKey)?;
    VerifyingKey::from_bytes(&key_bytes).map_err(|_| CryptoError::InvalidPublicKey)
}

pub fn decode_signature_b64(signature_b64: &str) -> Result<Signature, CryptoError> {
    let raw = BASE64_STD
        .decode(signature_b64.trim())
        .map_err(|e| CryptoError::InvalidBase64 {
            field: "signature_b64",
            detail: e.to_string(),
        })?;
    let sig_bytes: [u8; 64] = raw
        .try_into()
        .map_err(|_| CryptoError::InvalidSignatureFormat)?;
    Ok(Signature::from_bytes(&sig_bytes))
}

/// Verifies a raw canonical UTF-8 string against the base64-encoded Ed25519 server public key.
pub fn verify_canonical_ed25519(
    server_public_key_b64: &str,
    canonical_utf8: &str,
    signature_b64: &str,
) -> Result<(), CryptoError> {
    let verifying_key = decode_public_key_b64(server_public_key_b64)?;
    let signature = decode_signature_b64(signature_b64)?;
    verifying_key
        .verify_strict(canonical_utf8.as_bytes(), &signature)
        .map_err(|_| CryptoError::SignatureVerificationFailed)
}

/// Verifies a `SignedJob` against expiry, optional expected `device_id`, and Ed25519 signature.
pub fn verify_signed_job(
    job: &SignedJob,
    server_public_key_b64: &str,
    expected_device_id: Option<&str>,
    now_unix: i64,
) -> Result<(), CryptoError> {
    if job.expires_at_unix < now_unix {
        return Err(CryptoError::JobExpired {
            expires_at: job.expires_at_unix,
            now: now_unix,
        });
    }

    if let Some(expected) = expected_device_id {
        if !expected.is_empty() && job.device_id != expected {
            return Err(CryptoError::DeviceIdMismatch {
                expected: expected.to_string(),
                actual: job.device_id.clone(),
            });
        }
    }

    let canonical = job.canonical_string();
    verify_canonical_ed25519(server_public_key_b64, &canonical, &job.signature_b64)
}

/// Verifies a `SignedCatalogItem` against optional expected `device_id` and Ed25519 signature.
pub fn verify_signed_catalog_item(
    item: &SignedCatalogItem,
    server_public_key_b64: &str,
    expected_device_id: Option<&str>,
) -> Result<(), CryptoError> {
    if let Some(expected) = expected_device_id {
        if !expected.is_empty() && item.device_id != expected {
            return Err(CryptoError::DeviceIdMismatch {
                expected: expected.to_string(),
                actual: item.device_id.clone(),
            });
        }
    }

    let canonical = item.canonical_string();
    verify_canonical_ed25519(server_public_key_b64, &canonical, &item.signature_b64)
}

/// Computes lowercase hex SHA-256 of an in-memory byte slice.
pub fn compute_bytes_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Streams a file in 64 KiB chunks and returns its lowercase hex SHA-256 digest.
pub fn compute_file_sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let read_bytes = file.read(&mut buf)?;
        if read_bytes == 0 {
            break;
        }
        hasher.update(&buf[..read_bytes]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verifies that `path`'s SHA-256 digest matches `expected_sha256` (case-insensitive hex).
pub fn verify_file_sha256(path: &Path, expected_sha256: &str) -> Result<(), CryptoError> {
    let expected = expected_sha256.trim().to_ascii_lowercase();
    let actual = compute_file_sha256(path).map_err(|e| CryptoError::Io(e.to_string()))?;
    if actual != expected {
        return Err(CryptoError::Sha256Mismatch { expected, actual });
    }
    Ok(())
}

/// Helper Ed25519 keypair for deterministic cross-language & integration tests.
pub struct TestSigningKeypair {
    signing_key: SigningKey,
}

impl TestSigningKeypair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        Self {
            signing_key: SigningKey::generate(&mut csprng),
        }
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    pub fn public_key_b64(&self) -> String {
        BASE64_STD.encode(self.signing_key.verifying_key().to_bytes())
    }

    pub fn sign_canonical(&self, canonical_utf8: &str) -> String {
        let sig = self.signing_key.sign(canonical_utf8.as_bytes());
        BASE64_STD.encode(sig.to_bytes())
    }

    pub fn sign_job(&self, job: &mut SignedJob) {
        let canonical = job.canonical_string();
        job.signature_b64 = self.sign_canonical(&canonical);
    }

    pub fn sign_catalog_item(&self, item: &mut SignedCatalogItem) {
        let canonical = item.canonical_string();
        item.signature_b64 = self.sign_canonical(&canonical);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cross_language_deterministic_seed_vector() {
        // Deterministic 32-byte seed [0x42; 32] matching Python `Ed25519PrivateKey.from_private_bytes(b"\x42" * 32)`
        let kp = TestSigningKeypair::from_seed([0x42u8; 32]);
        let pub_b64 = kp.public_key_b64();
        assert_eq!(pub_b64.len(), 44);

        let mut job = SignedJob {
            job_id: "job-001".to_string(),
            device_id: "dev-123".to_string(),
            job_type: "software".to_string(),
            action: "install".to_string(),
            target_id: "pkg-7zip".to_string(),
            version: "24.08".to_string(),
            sha256: Some(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            ),
            command: Some("/qn /norestart".to_string()),
            expires_at_unix: 2000000000,
            signature_b64: String::new(),
            download_url: Some("/api/v1/agent/packages/pkg-7zip/download".to_string()),
            update_ids: vec![],
            security_only: false,
            package_meta: None,
            detection_rule: None,
            blocked_processes: vec![],
            return_codes: HashMap::new(),
        };

        assert_eq!(
            job.canonical_string(),
            "v1|job|job-001|dev-123|software|install|pkg-7zip|24.08|e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855|/qn /norestart|2000000000"
        );

        kp.sign_job(&mut job);
        assert_eq!(job.signature_b64.len(), 88);

        // Valid verification succeeds
        assert!(verify_signed_job(&job, &pub_b64, Some("dev-123"), 1999999999).is_ok());

        // Expired job rejected deterministically
        assert_eq!(
            verify_signed_job(&job, &pub_b64, Some("dev-123"), 2000000001),
            Err(CryptoError::JobExpired {
                expires_at: 2000000000,
                now: 2000000001
            })
        );

        // Wrong device_id rejected deterministically
        assert_eq!(
            verify_signed_job(&job, &pub_b64, Some("dev-other"), 1900000000),
            Err(CryptoError::DeviceIdMismatch {
                expected: "dev-other".to_string(),
                actual: "dev-123".to_string()
            })
        );

        // Tampered command rejected deterministically
        let mut tampered = job.clone();
        tampered.command = Some("cmd.exe /c calc.exe".to_string());
        assert_eq!(
            verify_signed_job(&tampered, &pub_b64, Some("dev-123"), 1900000000),
            Err(CryptoError::SignatureVerificationFailed)
        );
    }

    #[test]
    fn test_signed_catalog_item_verification_and_tamper_rejection() {
        let kp = TestSigningKeypair::generate();
        let pub_b64 = kp.public_key_b64();

        let mut item = SignedCatalogItem {
            package_id: "pkg-firefox".to_string(),
            device_id: "dev-123".to_string(),
            name: "Mozilla Firefox".to_string(),
            vendor: "Mozilla".to_string(),
            description: "Browser".to_string(),
            category: "Browser".to_string(),
            icon_url: String::new(),
            os_family: "windows".to_string(),
            source_type: "winget".to_string(),
            package_ref: "Mozilla.Firefox".to_string(),
            version: "132.0".to_string(),
            sha256: None,
            file_size: 0,
            download_url: None,
            install_cmd: None,
            uninstall_cmd: None,
            allow_uninstall: false,
            installed_version: None,
            status: "available".to_string(),
            blocked_processes: vec!["firefox.exe".to_string()],
            detection_rule: None,
            return_codes: HashMap::new(),
            signature_b64: String::new(),
        };

        assert_eq!(
            item.canonical_string(),
            "v1|catalog|pkg-firefox|dev-123|winget|Mozilla.Firefox|132.0||||0"
        );
        kp.sign_catalog_item(&mut item);

        assert!(verify_signed_catalog_item(&item, &pub_b64, Some("dev-123")).is_ok());

        // Flipping allow_uninstall from false -> true invalidates Ed25519 signature!
        let mut tampered = item.clone();
        tampered.allow_uninstall = true;
        assert_eq!(
            verify_signed_catalog_item(&tampered, &pub_b64, Some("dev-123")),
            Err(CryptoError::SignatureVerificationFailed)
        );
    }

    #[test]
    fn test_rfc8032_test_vector_1_compatibility() {
        // RFC 8032 Section 7.1 TEST 1 (empty message) - identical to Python cryptography
        let secret_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let expected_pub_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let expected_sig_hex = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

        let seed: [u8; 32] = hex::decode(secret_hex).unwrap().try_into().unwrap();
        let kp = TestSigningKeypair::from_seed(seed);
        let pub_b64 = BASE64_STD.encode(hex::decode(expected_pub_hex).unwrap());
        assert_eq!(kp.public_key_b64(), pub_b64);

        let sig_b64 = kp.sign_canonical("");
        assert_eq!(
            sig_b64,
            BASE64_STD.encode(hex::decode(expected_sig_hex).unwrap())
        );
        assert!(verify_canonical_ed25519(&pub_b64, "", &sig_b64).is_ok());
    }

    #[test]
    fn test_file_sha256_verification() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"NS8-SoftwareManager test payload").unwrap();
        tmp.flush().unwrap();

        let digest = compute_file_sha256(tmp.path()).unwrap();
        assert_eq!(digest.len(), 64);
        assert!(verify_file_sha256(tmp.path(), &digest.to_uppercase()).is_ok());
        assert!(matches!(
            verify_file_sha256(
                tmp.path(),
                "0000000000000000000000000000000000000000000000000000000000000000"
            ),
            Err(CryptoError::Sha256Mismatch { .. })
        ));
    }
}
