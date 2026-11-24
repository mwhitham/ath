//! Checking an OAuth access token from the configured OpenID Connect provider (D260).
//!
//! The host checks the issuer, the signature against the provider's published keys,
//! the expiry, the audience, and the client. It accepts only signature algorithms
//! with a public key, so a shared secret can never sign a token for it. The person is
//! identified by issuer and subject, never by email address.

use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::config::SignInConfig;
use crate::error::{ApiError, ApiResult};

const ACCEPTED: &[Algorithm] = &[
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::EdDSA,
];

/// How long published keys are trusted before they are read again.
const KEY_LIFETIME: Duration = Duration::from_secs(600);
/// The shortest gap between two reads caused by an unknown key ID.
const REFRESH_GAP: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub issuer: String,
    pub subject: String,
    pub client_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

pub struct Verifier {
    config: SignInConfig,
    keys: RwLock<Option<(Instant, JwkSet)>>,
}

impl Verifier {
    pub fn new(config: SignInConfig) -> Verifier {
        Verifier { config, keys: RwLock::new(None) }
    }

    pub fn issuer(&self) -> &str {
        &self.config.issuer
    }

    pub async fn verify(&self, token: &str) -> ApiResult<Verified> {
        let header = decode_header(token).map_err(|_| ApiError::sign_in_required("The access token cannot be read"))?;
        if !ACCEPTED.contains(&header.alg) {
            return Err(ApiError::sign_in_required("The access token uses a signature this host does not accept"));
        }
        let key = self.key(header.kid.as_deref()).await?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "sub", "aud"]);
        validation.leeway = 30;
        let data = decode::<Claims>(token, &key, &validation)
            .map_err(|error| ApiError::sign_in_required(format!("The access token was refused: {error}")))?;
        let claims = data.claims;
        let client_id = claims
            .azp
            .or(claims.client_id)
            .ok_or_else(|| ApiError::sign_in_required("The access token does not name the app it was issued to"))?;
        if !self.config.allowed_clients.is_empty() && !self.config.allowed_clients.contains(&client_id) {
            return Err(ApiError::sign_in_required("This host does not accept tokens issued to this app"));
        }
        Ok(Verified { issuer: claims.iss, subject: claims.sub, client_id, name: claims.name })
    }

    async fn key(&self, kid: Option<&str>) -> ApiResult<DecodingKey> {
        if let Some(key) = self.cached_key(kid, KEY_LIFETIME).await? {
            return Ok(key);
        }
        let fresh_enough = matches!(&*self.keys.read().await, Some((read_at, _)) if read_at.elapsed() < REFRESH_GAP);
        if !fresh_enough {
            let set = self.fetch().await?;
            *self.keys.write().await = Some((Instant::now(), set));
        }
        self.cached_key(kid, Duration::MAX)
            .await?
            .ok_or_else(|| ApiError::sign_in_required("The access token was signed with a key this host does not know"))
    }

    async fn cached_key(&self, kid: Option<&str>, lifetime: Duration) -> ApiResult<Option<DecodingKey>> {
        let keys = self.keys.read().await;
        let Some((read_at, set)) = &*keys else {
            return Ok(None);
        };
        if read_at.elapsed() > lifetime {
            return Ok(None);
        }
        let jwk = match kid {
            Some(kid) => set.find(kid),
            None if set.keys.len() == 1 => set.keys.first(),
            None => None,
        };
        match jwk {
            Some(jwk) => DecodingKey::from_jwk(jwk).map(Some).map_err(|error| ApiError::internal("signing key", error)),
            None => Ok(None),
        }
    }

    async fn fetch(&self) -> ApiResult<JwkSet> {
        let issuer = self.config.issuer.trim_end_matches('/').to_string();
        let configured = self.config.jwks_uri.clone();
        tokio::task::spawn_blocking(move || -> Result<JwkSet, String> {
            let jwks_uri = match configured {
                Some(uri) => uri,
                None => {
                    let discovery: serde_json::Value = ureq::get(&format!("{issuer}/.well-known/openid-configuration"))
                        .timeout(Duration::from_secs(10))
                        .call()
                        .map_err(|error| error.to_string())?
                        .into_json()
                        .map_err(|error| error.to_string())?;
                    discovery["jwks_uri"].as_str().ok_or("the discovery document names no jwks_uri")?.to_string()
                }
            };
            ureq::get(&jwks_uri)
                .timeout(Duration::from_secs(10))
                .call()
                .map_err(|error| error.to_string())?
                .into_json::<JwkSet>()
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| ApiError::internal("reading signing keys", error))?
        .map_err(|error| ApiError::internal("reading signing keys", error))
    }
}
