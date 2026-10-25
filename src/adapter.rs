//! What this adapter says it is, and the kit that serves its contracts.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::{media, owner};

use crate::audiobookshelf::Audiobookshelf;

/// Where Audiobookshelf answers, by its service id and the port it listens on.
pub const ADDRESS: &str = "http://audiobookshelf:80";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Audiobookshelf";

/// The Audiobookshelf release this adapter's recordings were taken from.
pub const RELEASE: &str = "2.37.1";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:581d68b2a6fc7ebf58d81c878a9f387cbbc0d88ac9d37b298b9cee10168af85b";

/// What this adapter says it is: the contracts it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![
            lemonfiber_contract::spoken(owner::claim::CAPABILITY, owner::claim::MAJOR),
            lemonfiber_contract::spoken(media::serve::CAPABILITY, media::serve::MAJOR),
        ],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contracts, served from `audiobookshelf` under `key`, ready while Audiobookshelf
/// answers.
#[must_use]
pub fn kit(key: String, audiobookshelf: Arc<Audiobookshelf>) -> Kit {
    let asked = Arc::clone(&audiobookshelf);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(owner::claim::served(Arc::clone(&audiobookshelf)))
        .serving(media::serve::served(audiobookshelf))
}

#[cfg(test)]
mod tests;
