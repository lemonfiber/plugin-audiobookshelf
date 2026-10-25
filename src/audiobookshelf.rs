//! Audiobookshelf, the audiobook server, asked through its own API as the root account
//! lemonfiber minted for this adapter.

use std::sync::{Arc, Mutex};

use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_adapter_kit::endpoint::Endpoint;
use lemonfiber_ports::http::{Fetched, Http, Method, Request, Response, JSON};
use lemonfiber_ports::service::Failure;
use serde::Deserialize;

mod keys;
mod owning;
mod screening;
mod serving;

/// What every failure is blamed on.
pub const SERVICE: &str = "audiobookshelf";

const HEALTH: &str = "/healthcheck";

const LOGIN: &str = "/login";

const LIBRARIES: &str = "/api/libraries";

const ME: &str = "/api/me";

const ITEMS: &str = "/api/items";

const RETURN_TOKENS: &str = "x-return-tokens";

const AUTHORIZATION: &str = "Authorization";

const CONTENT_TYPE: &str = "Content-Type";

const SESSION_REFUSED: u16 = 401;

const ABSENT: u16 = 404;

const NO_ROOT: &str = "this adapter holds no root account for Audiobookshelf";

/// A client for one Audiobookshelf, signed in as the root account `username` and
/// `password` supply, holding the session it opened until Audiobookshelf refuses it.
pub struct Audiobookshelf {
    http: Arc<dyn Http>,
    endpoint: Endpoint,
    username: Arc<dyn Holding>,
    password: Arc<dyn Holding>,
    session: Mutex<Option<String>>,
}

#[derive(Deserialize)]
struct Login {
    user: Account,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    access_token: String,
}

#[derive(Deserialize)]
struct Libraries {
    libraries: Vec<LibraryHeld>,
}

#[derive(Deserialize)]
struct LibraryHeld {
    id: String,
}

impl Audiobookshelf {
    /// The Audiobookshelf reached at `base` over `http`, signed in as the root account
    /// `username` and `password` supply.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        username: Arc<dyn Holding>,
        password: Arc<dyn Holding>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(Arc::clone(&http), base, SERVICE),
            http,
            username,
            password,
            session: Mutex::new(None),
        }
    }

    /// Whether Audiobookshelf says it is healthy, which it says to anybody.
    pub async fn answers(&self) -> bool {
        self.endpoint
            .send(&self.request(Method::Get, HEALTH, None, None))
            .await
            .is_ok_and(|response| response.is_success())
    }

    fn root(&self) -> Result<serde_json::Value, Failure> {
        let held = |holding: &Arc<dyn Holding>| {
            holding
                .key()
                .map_err(|why| self.endpoint.refused(&format!("{NO_ROOT}: {why}")))
        };
        Ok(serde_json::json!({
            "username": held(&self.username)?,
            "password": held(&self.password)?,
        }))
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
        bearer: Option<&str>,
    ) -> Request {
        let mut headers = Vec::new();
        if let Some(token) = bearer {
            headers.push((AUTHORIZATION.to_owned(), format!("Bearer {token}")));
        }
        if body.is_some() {
            headers.push((CONTENT_TYPE.to_owned(), JSON.to_owned()));
        }
        Request {
            method,
            url: self.endpoint.url(path),
            headers,
            body,
            pinned: None,
        }
    }

    async fn opened(&self) -> Result<String, Failure> {
        let mut asking = self.request(Method::Post, LOGIN, Some(self.root()?.to_string()), None);
        asking
            .headers
            .push((RETURN_TOKENS.to_owned(), "true".to_owned()));
        let response = self.endpoint.send(&asking).await?;
        let login: Login = self.endpoint.decode(
            &response,
            "the session Audiobookshelf opened could not be read",
        )?;
        if login.user.access_token.is_empty() {
            return Err(self
                .endpoint
                .refused("Audiobookshelf opened a session with no token"));
        }
        if let Ok(mut held) = self.session.lock() {
            held.replace(login.user.access_token.clone());
        }
        Ok(login.user.access_token)
    }

    async fn session(&self) -> Result<String, Failure> {
        let held = self.session.lock().ok().and_then(|held| held.clone());
        match held {
            Some(token) => Ok(token),
            None => self.opened().await,
        }
    }

    async fn as_root(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, Failure> {
        let token = self.session().await?;
        let response = self
            .endpoint
            .send(&self.request(method, path, body.clone(), Some(&token)))
            .await?;
        if response.status != SESSION_REFUSED {
            return Ok(response);
        }
        let token = self.opened().await?;
        self.endpoint
            .send(&self.request(method, path, body, Some(&token)))
            .await
    }

    async fn fetched_as_root(&self, path: &str, most: usize) -> Result<Fetched, Failure> {
        let token = self.session().await?;
        let fetched = self.fetched(path, &token, most).await?;
        if fetched.status != SESSION_REFUSED {
            return Ok(fetched);
        }
        let token = self.opened().await?;
        self.fetched(path, &token, most).await
    }

    async fn fetched(&self, path: &str, token: &str, most: usize) -> Result<Fetched, Failure> {
        self.http
            .fetch(&self.request(Method::Get, path, None, Some(token)), most)
            .await
            .map_err(|_| Failure::Unavailable {
                service: SERVICE.to_owned(),
            })
    }

    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        what: &str,
    ) -> Result<T, Failure> {
        let response = self.as_root(Method::Get, path, None).await?;
        self.endpoint.decode(&response, what)
    }

    async fn done(&self, method: Method, path: &str, body: Option<String>) -> Result<(), Failure> {
        let response = self.as_root(method, path, body).await?;
        self.endpoint.expect_success(&response)
    }

    async fn libraries(&self) -> Result<Vec<String>, Failure> {
        let held: Libraries = self
            .read(
                LIBRARIES,
                "the libraries Audiobookshelf holds could not be read",
            )
            .await?;
        Ok(held
            .libraries
            .into_iter()
            .map(|library| library.id)
            .filter(|id| an_id(id))
            .collect())
    }

    fn unsupported(detail: &str) -> Failure {
        Failure::Unsupported {
            service: SERVICE.to_owned(),
            detail: detail.to_owned(),
        }
    }
}

/// Whether `id` is spelled as the identifiers Audiobookshelf assigns are.
fn an_id(id: &str) -> bool {
    (32..=36).contains(&id.len())
        && id
            .bytes()
            .all(|mark| mark.is_ascii_hexdigit() || mark == b'-')
}

#[cfg(test)]
mod tests;
