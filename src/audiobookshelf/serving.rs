//! Looking over Audiobookshelf's libraries again, reading one item afresh, and the origins
//! a browser may read it from. What it has no equivalent of is unsupported.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Declared, Declaring, Failure, Fronted, Library, SeriesHeld, Upkeep,
};
use serde::Deserialize;

use super::{an_id, Audiobookshelf, ITEMS, LIBRARIES};

const NO_KINDS: &str = "Audiobookshelf holds neither films nor series";

const NO_PROXIES: &str = "Audiobookshelf holds no list of addresses trusted to name the client";

const NO_RESTART: &str = "Audiobookshelf cannot be asked to start again";

const NO_GUARD: &str = "Audiobookshelf serves no member through a guard of lemonfiber's";

const AUTHORIZE: &str = "/api/authorize";

const SETTINGS: &str = "/api/settings";

const WILDCARD: char = '*';

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settled {
    server_settings: Settings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    allowed_origins: Vec<String>,
}

#[async_trait]
impl Library for Audiobookshelf {
    async fn has_item(&self, _kind: Kind, _term: &str) -> Result<bool, Failure> {
        Err(Self::unsupported(NO_KINDS))
    }

    async fn rescan(&self) -> Result<(), Failure> {
        for library in self.libraries().await? {
            self.done(Method::Post, &format!("{LIBRARIES}/{library}/scan"), None)
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Upkeep for Audiobookshelf {
    async fn series_held(&self, _most: u32) -> Result<Vec<SeriesHeld>, Failure> {
        Err(Self::unsupported(NO_KINDS))
    }

    async fn refresh(&self, id: &str) -> Result<(), Failure> {
        if !an_id(id) {
            return Err(self
                .endpoint
                .refused("that names nothing Audiobookshelf holds"));
        }
        self.done(Method::Post, &format!("{ITEMS}/{id}/scan"), None)
            .await
    }
}

#[async_trait]
impl Fronted for Audiobookshelf {
    async fn known_proxies(&self) -> Result<Vec<String>, Failure> {
        Err(Self::unsupported(NO_PROXIES))
    }

    async fn trust_only(&self, _address: &str) -> Result<(), Failure> {
        Err(Self::unsupported(NO_PROXIES))
    }

    async fn allowed_origins(&self) -> Result<Vec<String>, Failure> {
        let response = self.as_root(Method::Post, AUTHORIZE, None).await?;
        let settled: Settled = self.endpoint.decode(
            &response,
            "the origins Audiobookshelf allows could not be read",
        )?;
        Ok(settled.server_settings.allowed_origins)
    }

    async fn allow_only(&self, origin: &str) -> Result<(), Failure> {
        if origin.trim().is_empty() || origin.contains(WILDCARD) {
            return Err(self
                .endpoint
                .refused("an origin allowed alone names one origin, never a wildcard"));
        }
        let written = serde_json::json!({ "allowedOrigins": [origin] }).to_string();
        let response = self.as_root(Method::Patch, SETTINGS, Some(written)).await?;
        let settled: Settled = self.endpoint.decode(
            &response,
            "the origins Audiobookshelf allows could not be read back",
        )?;
        if settled.server_settings.allowed_origins == [origin] {
            Ok(())
        } else {
            Err(self
                .endpoint
                .refused("Audiobookshelf read back other origins than the one written"))
        }
    }

    async fn restart(&self) -> Result<(), Failure> {
        Err(Self::unsupported(NO_RESTART))
    }
}

#[async_trait]
impl Declaring for Audiobookshelf {
    async fn declared(&self) -> Result<Declared, Failure> {
        Err(Self::unsupported(NO_GUARD))
    }
}
