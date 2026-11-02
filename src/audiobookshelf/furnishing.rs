//! A library for each media type the stack files that Audiobookshelf takes, added where
//! its folder exists and no library reads it, and none Audiobookshelf already holds
//! renamed, re-pointed or removed.

use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, FiledAt, Furnished, Furnishing};

use super::{Audiobookshelf, Libraries, LibraryHeld, LIBRARIES};

const FILESYSTEM: &str = "/api/filesystem";

const NOT_A_FOLDER: u16 = 400;

/// How Audiobookshelf makes a library of one of the stack's media types: its title, the
/// media type Audiobookshelf names it by and the metadata provider it matches items with.
struct Kind {
    media_type: &'static str,
    title: &'static str,
    library_type: &'static str,
    provider: &'static str,
}

const TAKEN: [Kind; 2] = [
    Kind {
        media_type: "audiobooks",
        title: "Audiobooks",
        library_type: "book",
        provider: "google",
    },
    Kind {
        media_type: "podcasts",
        title: "Podcasts",
        library_type: "podcast",
        provider: "itunes",
    },
];

impl LibraryHeld {
    fn reads(&self, folder: &str) -> bool {
        self.folders
            .iter()
            .any(|held| held.full_path.trim_end_matches('/') == folder.trim_end_matches('/'))
    }
}

impl Audiobookshelf {
    /// Whether `folder` exists inside Audiobookshelf's container.
    async fn exists(&self, folder: &str) -> Result<bool, Failure> {
        let asked = form_encoded(&[("path", folder)]);
        let response = self
            .as_root(Method::Get, &format!("{FILESYSTEM}?{asked}"), None)
            .await?;
        if response.status == NOT_A_FOLDER {
            return Ok(false);
        }
        self.endpoint.expect_success(&response).map(|()| true)
    }

    /// The library of `kind` Audiobookshelf made reading `folder`, none where it refused
    /// the folder.
    async fn added(&self, kind: &Kind, folder: &str) -> Result<Option<LibraryHeld>, Failure> {
        let asked = serde_json::json!({
            "name": kind.title,
            "folders": [{ "fullPath": folder }],
            "mediaType": kind.library_type,
            "provider": kind.provider,
        })
        .to_string();
        let response = self.as_root(Method::Post, LIBRARIES, Some(asked)).await?;
        if response.status == NOT_A_FOLDER {
            return Ok(None);
        }
        self.endpoint
            .decode(
                &response,
                "the library Audiobookshelf made could not be read",
            )
            .map(Some)
    }

    /// Whether a library of `held` now reads `one`, adding one where its folder exists and
    /// none of `held` reads it or carries the title it would be added under.
    async fn furnished(&self, held: &mut Libraries, one: &FiledAt) -> Result<bool, Failure> {
        let Some(kind) = TAKEN.iter().find(|kind| kind.media_type == one.media_type) else {
            return Ok(false);
        };
        if held
            .libraries
            .iter()
            .any(|library| library.reads(&one.folder))
        {
            return Ok(true);
        }
        if held
            .libraries
            .iter()
            .any(|library| library.name == kind.title)
            || !self.exists(&one.folder).await?
        {
            return Ok(false);
        }
        let Some(added) = self.added(kind, &one.folder).await? else {
            return Ok(false);
        };
        let reads = added.reads(&one.folder);
        held.libraries.push(added);
        Ok(reads)
    }
}

#[async_trait]
impl Furnishing for Audiobookshelf {
    async fn furnish(&self, filed: &[FiledAt]) -> Result<Vec<Furnished>, Failure> {
        let mut held = self.shelved().await?;
        let mut furnished = Vec::with_capacity(filed.len());
        for one in filed {
            furnished.push(Furnished {
                media_type: one.media_type.clone(),
                held: self.furnished(&mut held, one).await?,
            });
        }
        Ok(furnished)
    }
}
