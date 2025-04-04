use crate::config::constants::defaults;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use rsa::pkcs8::DecodePublicKey;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};
use time::OffsetDateTime;

use crate::logger::logger::{log_error, log_info, log_warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseInfo {
    machine_id: String,
    pub(crate) expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct License {
    info: LicenseInfo,
    signature: String,
}

pub struct LicenseValidator {
    machine_id: String,
    license_dir: PathBuf,
    xor_key: Vec<u8>,
    protected_public: Vec<u8>,
    protected_encryption: Vec<u8>,
}

impl LicenseValidator {
    pub fn new(
        xor_key: Vec<u8>,
        protected_public: Vec<u8>,
        protected_encryption: Vec<u8>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let machine_id = Self::get_machine_id()?;
        let local_appdata = env::var("LOCALAPPDATA")?;
        let license_dir = PathBuf::from(local_appdata).join(defaults::RAC_DIR);

        if !license_dir.exists() {
            fs::create_dir_all(&license_dir)?;
            log_info("Created license directory", "LicenseValidator::new");
        }

        log_info(
            &format!("Initialized LicenseValidator with machine ID: {}", machine_id),
            "LicenseValidator::new",
        );

        Ok(Self {
            machine_id,
            license_dir,
            xor_key,
            protected_public,
            protected_encryption,
        })
    }

    pub fn get_current_machine_id(&self) -> &str {
        &self.machine_id
    }

    pub fn get_license_dir(&self) -> String {
        self.license_dir.to_string_lossy().replace("\\\\", "\\")
    }

    pub fn get_license_info(&self) -> Result<LicenseInfo, Box<dyn std::error::Error>> {
        let license_path = self
            .license_dir
            .join(self.machine_id.to_string() + ".license");
        let encrypted_data = fs::read(&license_path)?;
        let license_data = self.decrypt_license_data(&encrypted_data)?;
        let license: License = serde_json::from_str(&license_data)?;
        Ok(license.info)
    }

    fn get_machine_id() -> Result<String, Box<dyn std::error::Error>> {
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("wmic")
                .args(["csproduct", "get", "UUID"])
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            let uuid = stdout
                .lines()
                .nth(1)
                .ok_or("Failed to get UUID")?
                .trim()
                .to_string();
            Ok(uuid)
        }
    }

    fn decrypt_license_data(&self, encrypted_data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        if encrypted_data.len() < 12 {
            log_error("Invalid encrypted data length", "decrypt_license_data");
            return Err("Invalid encrypted data length".into());
        }

        match self.decrypt_license_data_internal(encrypted_data) {
            Ok(data) => {
                log_info("License data decrypted successfully", "decrypt_license_data");
                Ok(data)
            }
            Err(e) => {
                log_error(&format!("License decryption failed: {}", e), "decrypt_license_data");
                Err(e)
            }
        }
    }

    fn decrypt_license_data_internal(
        &self,
        encrypted_data: &[u8],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let xored_encryption_key: Vec<u8> = self
            .protected_encryption
            .iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ self.xor_key[i % self.xor_key.len()])
            .collect();
        let decoded_key = general_purpose::STANDARD.decode(&xored_encryption_key)?;
        let key = Key::<Aes256Gcm>::from_slice(&decoded_key);
        let cipher = Aes256Gcm::new(key);

        let nonce = Nonce::from_slice(&encrypted_data[..12]);
        let ciphertext = &encrypted_data[12..];

        let decrypted = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;

        String::from_utf8(decrypted).map_err(|e| format!("Invalid UTF-8: {}", e).into())
    }

    pub fn validate_license(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let license_path = self
            .license_dir
            .join(self.machine_id.to_string() + ".license");

        if !license_path.exists() {
            log_error("License file not found", "validate_license");
            return Err("License file not found. Please contact your administrator.".into());
        }

        log_info("Starting license validation", "validate_license");

        let encrypted_data = fs::read(&license_path)?;
        let license_data = self.decrypt_license_data(&encrypted_data)?;
        let license: License = serde_json::from_str(&license_data)?;

        if license.info.machine_id != self.machine_id {
            log_warn("Machine ID mismatch detected", "validate_license");
            return Ok(false);
        }

        let now = OffsetDateTime::now_utc().unix_timestamp();
        if now > license.info.expires_at {
            log_warn("License has expired", "validate_license");
            return Ok(false);
        }

        match self.verify_signature(&license) {
            Ok(true) => {
                log_info("License validation successful", "validate_license");
                Ok(true)
            }
            Ok(false) => {
                log_warn("Invalid license signature", "validate_license");
                Ok(false)
            }
            Err(e) => {
                log_error(&format!("Signature verification error: {}", e), "validate_license");
                Err(e)
            }
        }
    }

    fn verify_signature(&self, license: &License) -> Result<bool, Box<dyn std::error::Error>> {
        let public_key_bytes = &self.protected_public;

        let xored_public_key: Vec<u8> = public_key_bytes
            .iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ self.xor_key[i % self.xor_key.len()])
            .collect();
        let public_key_str = String::from_utf8_lossy(&xored_public_key);

        let public_key = RsaPublicKey::from_public_key_pem(public_key_str.as_ref())?;
        let info_bytes = serde_json::to_vec(&license.info)?;

        let mut hasher = Sha256::new();
        hasher.update(&info_bytes);
        let hash = hasher.finalize();

        let signature_bytes = general_purpose::STANDARD.decode(&license.signature)?;

        Ok(public_key
            .verify(
                rsa::Pkcs1v15Sign::new::<Sha256>(),
                &hash,
                &signature_bytes,
            )
            .is_ok())
    }
}