//! The API keys Audiobookshelf holds for the stack's services, each filed under an app's
//! name and made for the root account. Audiobookshelf shows a key once, when it is minted,
//! so the keys filed under a name are unsupported.

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use lemonfiber_adapter_kit::credential::spelled_as_a_key;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{AppKeys, Dated, Failure};
use serde::Deserialize;

use super::{an_id, Audiobookshelf, ME};

const KEYS: &str = "/api/api-keys";

const SHOWN_ONCE: &str = "Audiobookshelf shows a key only once, when it is minted";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Keys {
    api_keys: Vec<Key>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Key {
    id: String,
    name: String,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    last_used_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Minted {
    api_key: MintedKey,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MintedKey {
    id: String,
    api_key: String,
}

#[derive(Deserialize)]
struct Me {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    key_id: String,
    name: String,
}

impl Audiobookshelf {
    async fn listed(&self, app: &str) -> Result<Vec<Key>, Failure> {
        let keys: Keys = self
            .read(KEYS, "the keys Audiobookshelf holds could not be read")
            .await?;
        let mut filed: Vec<Key> = keys
            .api_keys
            .into_iter()
            .filter(|key| key.name == app)
            .collect();
        filed.sort_by(|one, other| one.created_at.cmp(&other.created_at));
        Ok(filed)
    }

    fn a_key(&self, key: &str) -> Result<(), Failure> {
        if spelled_as_a_key(key) {
            Ok(())
        } else {
            Err(self.endpoint.refused("that is not spelled as a key is"))
        }
    }
}

/// The key and name a key Audiobookshelf minted says it is, read off its claims.
fn claimed(key: &str) -> Option<Claims> {
    let mut parts = key.split('.');
    let claims = parts.nth(1)?;
    let read = URL_SAFE_NO_PAD.decode(claims.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&read).ok()
}

#[async_trait]
impl AppKeys for Audiobookshelf {
    async fn filed_as(&self, _app: &str) -> Result<Vec<String>, Failure> {
        Err(Self::unsupported(SHOWN_ONCE))
    }

    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        Ok(self
            .listed(app)
            .await?
            .into_iter()
            .map(|key| Dated {
                created: key.created_at,
                last_used: key.last_used_at,
            })
            .collect())
    }

    async fn mint(&self, app: &str) -> Result<String, Failure> {
        let me: Me = self
            .read(
                ME,
                "the account this adapter signed in as could not be read",
            )
            .await?;
        let before: Vec<String> = self
            .listed(app)
            .await?
            .into_iter()
            .map(|key| key.id)
            .collect();
        let asked =
            serde_json::json!({ "name": app, "userId": me.id, "isActive": true }).to_string();
        let response = self.as_root(Method::Post, KEYS, Some(asked)).await?;
        let minted: Minted = self
            .endpoint
            .decode(&response, "the key Audiobookshelf minted could not be read")?;
        let new: Vec<String> = self
            .listed(app)
            .await?
            .into_iter()
            .map(|key| key.id)
            .filter(|id| !before.contains(id))
            .collect();
        if new != [minted.api_key.id.as_str()] || minted.api_key.api_key.is_empty() {
            return Err(self.endpoint.refused(&format!(
                "a key was minted as {app} and the key list does not show it as the one new key"
            )));
        }
        Ok(minted.api_key.api_key)
    }

    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        self.a_key(key)?;
        let response = self
            .endpoint
            .send(&self.request(Method::Get, ME, None, Some(key)))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        self.a_key(key)?;
        let claims = claimed(key)
            .filter(|claims| an_id(&claims.key_id))
            .ok_or_else(|| {
                self.endpoint
                    .refused("that is not a key Audiobookshelf minted")
            })?;
        let held = self
            .listed(&claims.name)
            .await?
            .into_iter()
            .any(|filed| filed.id == claims.key_id);
        if !held {
            return Err(self
                .endpoint
                .refused("Audiobookshelf holds no key that one names"));
        }
        self.done(Method::Delete, &format!("{KEYS}/{}", claims.key_id), None)
            .await
    }
}
