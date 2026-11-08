use aws_lc_rs::encoding::AsDer;
use aws_lc_rs::rsa::{KeyPair, KeySize};
use aws_lc_rs::signature::{KeyPair as _, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

/// A Google Cloud service account key for tests: a fresh RSA key in a JSON key file of the
/// form Google Cloud downloads, and the public key that verifies what it signs.
pub struct ServiceAccountKey {
    pub json: String,
    pub public_key: Vec<u8>,
    pub client_email: String,
    pub project_id: String,
    pub private_key_id: String,
}

impl ServiceAccountKey {
    pub fn new(client_email: &str, project_id: &str) -> ServiceAccountKey {
        let pair = KeyPair::generate(KeySize::Rsa2048).expect("RSA key");
        let der = pair.as_der().expect("PKCS #8");
        let body = STANDARD.encode(der.as_ref());
        let lines: Vec<&str> = body
            .as_bytes()
            .chunks(64)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            lines.join("\n")
        );
        let private_key_id = "0123456789abcdef0123456789abcdef0123cafe".to_owned();
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "type": "service_account",
            "project_id": project_id,
            "private_key_id": private_key_id,
            "private_key": pem,
            "client_email": client_email,
            "client_id": "100000000000000000001",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "universe_domain": "googleapis.com"
        }))
        .expect("JSON key");
        ServiceAccountKey {
            json,
            public_key: pair.public_key().as_ref().to_vec(),
            client_email: client_email.to_owned(),
            project_id: project_id.to_owned(),
            private_key_id,
        }
    }

    /// The header and claims of an RS256 JWT this key signed, or `None` when the signature
    /// doesn't verify.
    pub fn verify(&self, jwt: &str) -> Option<(serde_json::Value, serde_json::Value)> {
        let mut parts = jwt.split('.');
        let (header, claims, signature) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, &self.public_key)
            .verify(format!("{header}.{claims}").as_bytes(), &signature)
            .ok()?;
        let decode = |part: &str| -> Option<serde_json::Value> {
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).ok()?).ok()
        };
        Some((decode(header)?, decode(claims)?))
    }
}
