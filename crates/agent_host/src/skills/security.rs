use anyhow::{Context, Result};
use async_trait::async_trait;
use keyring::Entry;
use std::sync::Arc;
use totp_rs::{Algorithm, TOTP};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::skills::{Skill, SkillInput, SkillContext};
use crate::skills::common::CommonInfrastructure;
use shared::skill::{Mode, PermissionLevel, SkillOutput};

/// Skill for managing 2FA Security
pub struct SecuritySkill {
    infra: Arc<CommonInfrastructure>,
}

impl SecuritySkill {
    pub fn new(infra: Arc<CommonInfrastructure>) -> Self {
        Self { infra }
    }

    /// Generate a new TOTP secret and QR code
    async fn setup_2fa(&self, user: &str) -> Result<SkillOutput> {
        use rand::RngCore;
        let mut secret_bytes = [0u8; 20];
        rand::thread_rng().fill_bytes(&mut secret_bytes);
        
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes.to_vec(),
            Some("LittleHelper".to_string()),
            user.to_string(),
        ).unwrap();

        // 1. Save to Keyring
        let entry = Entry::new("little-helper-2fa", user)?;
        let secret_str = BASE64.encode(&secret_bytes);
        entry.set_password(&secret_str).context("Failed to save secret to keyring")?;

        // 2. Generate QR Code
        let code_str = totp.get_qr_base64().map_err(|e| anyhow::anyhow!("QR Gen Error: {}", e))?; 
        
        // It returns a base64 encoded string of the PNG image
        let image_data = BASE64.decode(&code_str)?;
        let qr_path = self.infra.data_dir.join("2fa_qr.png");
        std::fs::write(&qr_path, image_data)?;
        
        Ok(SkillOutput::text("2FA Setup Initiated.\nScan the QR code below with Google Authenticator.")
            .with_file(shared::skill::FileResult {
                path: qr_path.clone(),
                action: shared::skill::FileAction::Created,
                preview: Some(format!(r#"<preview type="image" path="{}">Scan Valid QR Code</preview>"#, qr_path.display())),
            }))
    }

    /// Verify a code
    async fn verify_2fa(&self, user: &str, code: &str) -> Result<SkillOutput> {
         let entry = Entry::new("little-helper-2fa", user)?;
         let secret_str = entry.get_password().context("No 2FA secret found. Please run setup first.")?;
         let secret_bytes = BASE64.decode(secret_str).context("Invalid secret in keyring")?;

         let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes,
            Some("LittleHelper".to_string()),
            user.to_string(),
        ).unwrap();

        if totp.check_current(code).unwrap_or(false) {
            self.infra.security_context.authenticate();
            Ok(SkillOutput::text("✅ 2FA Code Verified! Session checks passed."))
        } else {
            Ok(SkillOutput::error("❌ Invalid 2FA Code. Please try again."))
        }
    }
}

#[async_trait]
impl Skill for SecuritySkill {
    fn id(&self) -> &'static str {
        "security"
    }

    fn name(&self) -> &'static str {
        "Security Manager"
    }

    fn description(&self) -> &'static str {
        "Manage 2FA setup and verification."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix, Mode::Data, Mode::Build, Mode::Research, Mode::Find]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let action = input.params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        // User could be passed in params or defaulted to "user"
        let user = input.params.get("user").and_then(|v| v.as_str()).unwrap_or("default_user");

        match action {
            "setup_2fa" => self.setup_2fa(user).await,
            "verify_2fa" => {
                let code = input.params.get("code").and_then(|v| v.as_str()).context("Missing code")?;
                self.verify_2fa(user, code).await
            }
            _ => Ok(SkillOutput::text("Unknown action. Use setup_2fa or verify_2fa.")),
        }
    }
}
