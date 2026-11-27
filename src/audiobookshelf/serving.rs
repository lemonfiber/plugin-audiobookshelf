//! Looking over Audiobookshelf's libraries again, reading one item afresh, and the origins
//! a browser may read it from. What it has no equivalent of is unsupported.

use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{
    Allowing, Applied, Channel, Declared, Declaring, Encrypting, Failure, Fronted, Heard, Library,
    Listing, Recounting, SeriesHeld, Tuned, Tuning, Upkeep,
};
use serde::Deserialize;

use super::{an_id, Audiobookshelf, ITEMS, LIBRARIES};

const NO_KINDS: &str = "Audiobookshelf holds neither films nor series";

const NO_PROXIES: &str = "Audiobookshelf holds no list of addresses trusted to name the client";

const NO_RESTART: &str = "Audiobookshelf cannot be asked to start again";

const NO_GUARD: &str = "Audiobookshelf serves no member through a guard of lemonfiber's";

const NO_TLS: &str = "Audiobookshelf serves no internal TLS port";

const AUTHORIZE: &str = "/api/authorize";

const SETTINGS: &str = "/api/settings";

const WILDCARD: char = '*';

const NO_EVENTS: &str = "Audiobookshelf tells nobody what it adds or what goes wrong";

const NO_LIVE_TV: &str = "Audiobookshelf has no live TV";

#[derive(Deserialize)]
struct Found {
    #[serde(default)]
    book: Vec<Hit>,
    #[serde(default)]
    podcast: Vec<Hit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hit {
    library_item: Titled,
}

#[derive(Deserialize)]
struct Titled {
    media: Media,
}

#[derive(Deserialize)]
struct Media {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(default)]
    title: Option<String>,
}

impl Hit {
    fn titled(&self, needle: &str) -> bool {
        self.library_item
            .media
            .metadata
            .title
            .as_deref()
            .is_some_and(|title| title.to_lowercase().contains(needle))
    }
}

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
    async fn has_item(&self, kind: TitleKind, term: &str) -> Result<bool, Failure> {
        let podcasts = match kind {
            TitleKind::Audiobooks | TitleKind::Books => false,
            TitleKind::Podcasts => true,
            TitleKind::Tv | TitleKind::Movies | TitleKind::Music => return Ok(false),
        };
        let needle = term.to_lowercase();
        let asked = form_encoded(&[("q", term)]);
        let unread = format!(
            "the {} Audiobookshelf holds could not be read",
            kind.media_type()
        );
        for library in self.libraries().await? {
            let found: Found = self
                .read(&format!("{LIBRARIES}/{library}/search?{asked}"), &unread)
                .await?;
            let hits = if podcasts { found.podcast } else { found.book };
            if hits.iter().any(|hit| hit.titled(&needle)) {
                return Ok(true);
            }
        }
        Ok(false)
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
        self.allowing(&[origin.to_owned()]).await
    }

    async fn restart(&self) -> Result<(), Failure> {
        Err(Self::unsupported(NO_RESTART))
    }
}

impl Audiobookshelf {
    async fn allowing(&self, origins: &[String]) -> Result<(), Failure> {
        if origins.is_empty()
            || origins
                .iter()
                .any(|origin| origin.trim().is_empty() || origin.contains(WILDCARD))
        {
            return Err(self
                .endpoint
                .refused("each origin allowed names one origin, never a wildcard"));
        }
        let written = serde_json::json!({ "allowedOrigins": origins }).to_string();
        let response = self.as_root(Method::Patch, SETTINGS, Some(written)).await?;
        let settled: Settled = self.endpoint.decode(
            &response,
            "the origins Audiobookshelf allows could not be read back",
        )?;
        if settled.server_settings.allowed_origins == origins {
            Ok(())
        } else {
            Err(self
                .endpoint
                .refused("Audiobookshelf read back other origins than those written"))
        }
    }
}

#[async_trait]
impl Allowing for Audiobookshelf {
    async fn allow_origins(&self, origins: &[String]) -> Result<(), Failure> {
        self.allowing(origins).await
    }
}

#[async_trait]
impl Recounting for Audiobookshelf {
    async fn events(
        &self,
        _address: Option<&str>,
        _pin: Option<&str>,
        _key: Option<&str>,
        _every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        Err(Self::unsupported(NO_EVENTS))
    }
}

#[async_trait]
impl Tuning for Audiobookshelf {
    async fn channels(&self, _member: &str) -> Result<Vec<Channel>, Failure> {
        Err(Self::unsupported(NO_LIVE_TV))
    }

    async fn guide(&self, _member: &str, _hours: u32) -> Result<Vec<Listing>, Failure> {
        Err(Self::unsupported(NO_LIVE_TV))
    }

    async fn tune(&self, _member: &str, _channel: &str) -> Result<Option<Tuned>, Failure> {
        Err(Self::unsupported(NO_LIVE_TV))
    }

    async fn untune(&self, _closes_by: &str) -> Result<(), Failure> {
        Err(Self::unsupported(NO_LIVE_TV))
    }
}

#[async_trait]
impl Declaring for Audiobookshelf {
    async fn declared(&self) -> Result<Declared, Failure> {
        Err(Self::unsupported(NO_GUARD))
    }
}

#[async_trait]
impl Encrypting for Audiobookshelf {
    async fn serve_tls(&self, _path: &str, _pin: &str) -> Result<Applied, Failure> {
        Err(Self::unsupported(NO_TLS))
    }
}
