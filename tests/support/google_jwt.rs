use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand::rngs::OsRng;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;
use time::OffsetDateTime;
use tokio::net::TcpListener;

pub const TEST_CLIENT_ID: &str = "test-client-id.apps.googleusercontent.com";
const TEST_KID: &str = "test-google-key";

#[derive(Clone, Serialize)]
pub struct GoogleTokenClaims {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
}

impl GoogleTokenClaims {
    pub fn valid(email: &str) -> Self {
        Self {
            sub: format!("subject-{email}"),
            email: email.into(),
            email_verified: true,
            name: Some("Test User".into()),
            iss: "https://accounts.google.com".into(),
            aud: TEST_CLIENT_ID.into(),
            exp: (OffsetDateTime::now_utc() + time::Duration::minutes(5)).unix_timestamp(),
        }
    }
}

struct SigningMaterial {
    encoding_key: EncodingKey,
    jwks_json: String,
}

fn generate_signing_material() -> SigningMaterial {
    let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("generate RSA test key");
    let private_pem = private_key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("encode RSA private key");
    let public_key = RsaPublicKey::from(&private_key);
    let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
    let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
    let jwks_json = serde_json::json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": TEST_KID,
            "n": n,
            "e": e
        }]
    })
    .to_string();
    SigningMaterial {
        encoding_key: EncodingKey::from_rsa_pem(private_pem.as_bytes())
            .expect("load RSA encoding key"),
        jwks_json,
    }
}

fn trusted_signing_material() -> &'static SigningMaterial {
    static MATERIAL: OnceLock<SigningMaterial> = OnceLock::new();
    MATERIAL.get_or_init(generate_signing_material)
}

fn untrusted_signing_material() -> &'static SigningMaterial {
    static MATERIAL: OnceLock<SigningMaterial> = OnceLock::new();
    MATERIAL.get_or_init(generate_signing_material)
}

pub struct GoogleJwtFixture {
    pub jwks_url: String,
    hits: Arc<AtomicUsize>,
    _server: tokio::task::JoinHandle<()>,
}

impl GoogleJwtFixture {
    pub async fn spawn() -> Self {
        let body = trusted_signing_material().jwks_json.clone();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_handler = Arc::clone(&hits);
        let app = Router::new().route(
            "/certs",
            get(move || {
                let body = body.clone();
                let hits = Arc::clone(&hits_for_handler);
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    ([("content-type", "application/json")], body)
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind JWT fixture");
        let address = listener.local_addr().expect("JWT fixture address");
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            jwks_url: format!("http://{address}/certs"),
            hits,
            _server: server,
        }
    }

    pub fn issue(&self, email: &str) -> String {
        self.issue_claims(&GoogleTokenClaims::valid(email))
    }

    pub fn issue_claims(&self, claims: &GoogleTokenClaims) -> String {
        issue_with(trusted_signing_material(), claims)
    }

    pub fn issue_with_wrong_key(&self, email: &str) -> String {
        issue_with(
            untrusted_signing_material(),
            &GoogleTokenClaims::valid(email),
        )
    }

    pub fn hit_count(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

fn issue_with(material: &SigningMaterial, claims: &GoogleTokenClaims) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KID.into());
    encode(&header, claims, &material.encoding_key).expect("sign Google-format test token")
}
