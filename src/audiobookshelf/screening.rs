//! What Audiobookshelf holds and plays, read as its root account reads it. A member of the
//! household is somebody `identity.source` names, and Audiobookshelf speaks for none of
//! them, so everything asked for one member is unsupported.

use std::collections::HashMap;

use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Failure, Holds, HowFar, Image, Item, ItemDetail, ItemProgress, Medium, Picture, Playback,
    Screening, PICTURE_MOST,
};
use serde::Deserialize;

use super::{an_id, Audiobookshelf, ABSENT, ITEMS, LIBRARIES};

const NO_MEMBERS: &str =
    "Audiobookshelf speaks for none of the household's accounts, so nothing is asked for one";

const NO_DEVICES: &str = "Audiobookshelf opens and ends no device's session for lemonfiber";

const BOOK: &str = "book";

const ONLINE: &str = "/api/users/online";

const USERS: &str = "/api/users";

const BY_TITLE: &str = "media.metadata.title";

const PICTURE_PREFIX: &str = "image/";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Held {
    id: String,
    #[serde(default)]
    media_type: String,
    media: Media,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    cover_path: Option<String>,
    #[serde(default)]
    num_tracks: u32,
    #[serde(default)]
    duration: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    published_year: Option<String>,
    #[serde(default)]
    published_date: Option<String>,
    #[serde(default)]
    description_plain: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
}

#[derive(Deserialize)]
struct Page {
    results: Vec<Held>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Online {
    open_sessions: Vec<Session>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    user_id: String,
    #[serde(default)]
    display_title: String,
    #[serde(default)]
    episode_id: Option<String>,
    #[serde(default)]
    media_metadata: Option<Metadata>,
    #[serde(default)]
    device_info: Option<Device>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default)]
    client_name: Option<String>,
}

#[derive(Deserialize)]
struct Users {
    users: Vec<User>,
}

#[derive(Deserialize)]
struct User {
    id: String,
    username: String,
}

impl Held {
    fn item(&self) -> Item {
        let metadata = &self.media.metadata;
        Item {
            id: self.id.clone(),
            title: metadata.title.clone().unwrap_or_default(),
            year: metadata
                .published_year
                .as_deref()
                .and_then(|year| year.trim().parse().ok()),
            medium: Medium::Other,
            holds: Holds {
                poster: self.media.cover_path.is_some(),
                backdrop: false,
                plays: self.media_type == BOOK && self.media.num_tracks > 0,
            },
        }
    }

    fn detail(self) -> ItemDetail {
        let item = self.item();
        let metadata = self.media.metadata;
        ItemDetail {
            item,
            overview: metadata
                .description_plain
                .or(metadata.description)
                .filter(|said| !said.trim().is_empty()),
            minutes: self.media.duration.and_then(whole_minutes),
            genres: metadata.genres,
            certificate: None,
            released: metadata.published_date.filter(|date| a_date(date)),
            seasons: Vec::new(),
        }
    }
}

/// `seconds` in whole minutes, nothing where it is not a length.
pub(super) fn whole_minutes(seconds: f64) -> Option<u32> {
    format!("{:.0}", seconds / 60.0).parse().ok()
}

/// Whether `said` is a calendar date written as `YYYY-MM-DD`.
pub(super) fn a_date(said: &str) -> bool {
    said.len() == 10
        && said.bytes().enumerate().all(|(at, mark)| match at {
            4 | 7 => mark == b'-',
            _ => mark.is_ascii_digit(),
        })
}

impl Audiobookshelf {
    async fn titled(&self, id: &str) -> Result<Option<Held>, Failure> {
        if !an_id(id) {
            return Ok(None);
        }
        let response = self
            .as_root(Method::Get, &format!("{ITEMS}/{id}?expanded=1"), None)
            .await?;
        if response.status == ABSENT {
            return Ok(None);
        }
        self.endpoint
            .decode(&response, "the item Audiobookshelf holds could not be read")
            .map(Some)
    }

    async fn poster(&self, id: &str) -> Result<Option<Image>, Failure> {
        if self.titled(id).await?.is_none() {
            return Ok(None);
        }
        let fetched = self
            .fetched_as_root(&format!("{ITEMS}/{id}/cover"), PICTURE_MOST)
            .await?;
        if fetched.status == ABSENT {
            return Ok(None);
        }
        if !fetched.is_success() {
            return Err(self
                .endpoint
                .refused("Audiobookshelf would not answer the cover"));
        }
        let Some(media_type) = fetched
            .header("content-type")
            .filter(|kind| kind.starts_with(PICTURE_PREFIX))
            .map(str::to_owned)
        else {
            return Ok(None);
        };
        Ok(fetched.bytes.map(|bytes| Image { media_type, bytes }))
    }

    async fn held(&self, most: u32) -> Result<Vec<Item>, Failure> {
        let mut items = Vec::new();
        for library in self.libraries().await? {
            let left = usize::try_from(most)
                .unwrap_or(usize::MAX)
                .saturating_sub(items.len());
            if left == 0 {
                break;
            }
            let asked = form_encoded(&[("limit", &left.to_string()), ("sort", BY_TITLE)]);
            let page: Page = self
                .read(
                    &format!("{LIBRARIES}/{library}/items?{asked}"),
                    "what one library holds could not be read",
                )
                .await?;
            items.extend(page.results.iter().take(left).map(Held::item));
        }
        Ok(items)
    }

    async fn played(&self) -> Result<Vec<Playback>, Failure> {
        let online: Online = self
            .read(ONLINE, "what Audiobookshelf is playing could not be read")
            .await?;
        if online.open_sessions.is_empty() {
            return Ok(Vec::new());
        }
        let users: Users = self
            .read(USERS, "the accounts Audiobookshelf holds could not be read")
            .await?;
        let named: HashMap<String, String> = users
            .users
            .into_iter()
            .map(|user| (user.id, user.username))
            .collect();
        Ok(online
            .open_sessions
            .into_iter()
            .map(|session| playback(session, &named))
            .collect())
    }
}

fn playback(session: Session, named: &HashMap<String, String>) -> Playback {
    let episode = session.episode_id.is_some();
    let device = session
        .device_info
        .and_then(|device| device.device_name.or(device.client_name))
        .unwrap_or_default();
    Playback {
        member: named.get(&session.user_id).cloned().unwrap_or_default(),
        member_id: session.user_id,
        title: session.display_title,
        series: session
            .media_metadata
            .filter(|_| episode)
            .and_then(|podcast| podcast.title),
        season: None,
        episode: None,
        medium: if episode {
            Medium::Episode
        } else {
            Medium::Other
        },
        paused: false,
        device,
    }
}

#[async_trait]
impl Screening for Audiobookshelf {
    async fn signed_in(&self, _member: &str, _device: &str) -> Result<Option<String>, Failure> {
        Err(Self::unsupported(NO_MEMBERS))
    }

    async fn title(&self, member: Option<&str>, id: &str) -> Result<Option<ItemDetail>, Failure> {
        if member.is_some() {
            return Err(Self::unsupported(NO_MEMBERS));
        }
        Ok(self.titled(id).await?.map(Held::detail))
    }

    async fn part_way(&self, _member: &str, _most: u32) -> Result<Vec<ItemProgress>, Failure> {
        Err(Self::unsupported(NO_MEMBERS))
    }

    async fn progressed(&self, _member: &str, _id: &str, _how_far: &HowFar) -> Result<(), Failure> {
        Err(Self::unsupported(NO_MEMBERS))
    }

    async fn sign_out(&self, _device: &str) -> Result<(), Failure> {
        Err(Self::unsupported(NO_DEVICES))
    }

    async fn picture(
        &self,
        member: Option<&str>,
        id: &str,
        which: Picture,
    ) -> Result<Option<Image>, Failure> {
        if member.is_some() {
            return Err(Self::unsupported(NO_MEMBERS));
        }
        match which {
            Picture::Poster => self.poster(id).await,
            Picture::Backdrop => Ok(None),
        }
    }

    async fn holdings(&self, member: Option<&str>, most: u32) -> Result<Vec<Item>, Failure> {
        if member.is_some() {
            return Err(Self::unsupported(NO_MEMBERS));
        }
        self.held(most).await
    }

    async fn playing(&self, member: Option<&str>) -> Result<Vec<Playback>, Failure> {
        if member.is_some() {
            return Err(Self::unsupported(NO_MEMBERS));
        }
        self.played().await
    }
}
