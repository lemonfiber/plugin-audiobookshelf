//! Who holds Audiobookshelf's root account, and claiming it: the root lemonfiber minted,
//! made by Audiobookshelf's first-run setup while nobody has made one.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Owner, Ownership};
use serde::Deserialize;

use super::{Audiobookshelf, ME};

const STATUS: &str = "/status";

const INIT: &str = "/init";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    is_init: bool,
}

impl Audiobookshelf {
    async fn initialised(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, STATUS, None, None))
            .await?;
        let status: Status = self.endpoint.decode(
            &response,
            "whether Audiobookshelf has its root account could not be read",
        )?;
        Ok(status.is_init)
    }
}

#[async_trait]
impl Ownership for Audiobookshelf {
    async fn owner(&self) -> Result<Owner, Failure> {
        if !self.initialised().await? {
            return Ok(Owner::Open);
        }
        let answered = self
            .as_root(Method::Get, ME, None)
            .await
            .and_then(|response| self.endpoint.expect_success(&response));
        match answered {
            Ok(()) => Ok(Owner::Ours),
            Err(Failure::Unauthorised { .. }) => Ok(Owner::Theirs),
            Err(failure) => Err(failure),
        }
    }

    async fn claim(&self) -> Result<Owner, Failure> {
        let held = self.owner().await?;
        if held != Owner::Open {
            return Ok(held);
        }
        let root = serde_json::json!({ "newRoot": self.root()? }).to_string();
        let response = self
            .endpoint
            .send(&self.request(Method::Post, INIT, Some(root), None))
            .await?;
        if response.is_success() {
            return Ok(Owner::Ours);
        }
        match self.owner().await? {
            Owner::Open => Err(self.endpoint.refusal(&response)),
            held => Ok(held),
        }
    }
}
