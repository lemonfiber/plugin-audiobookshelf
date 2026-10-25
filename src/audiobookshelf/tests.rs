use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Owner, Ownership as _};

use super::{an_id, Audiobookshelf};

mod keys;
mod screening;
mod serving;

const BASE: &str = "http://audiobookshelf:80";

const USERNAME: &str = "qrstuvwx";

const PASSWORD: &str = "ABCDEFGHIJKLMNOP";

const TOKEN: &str = "the-session-token";

const SIGNED_IN: &str = r#"{"user":{"accessToken":"the-session-token","token":"older"}}"#;

const INITIALISED: &str = r#"{"isInit":true,"serverVersion":"2.37.1"}"#;

const UNINITIALISED: &str = r#"{"isInit":false,"serverVersion":"2.37.1"}"#;

const LIBRARIES: &str = r#"{"libraries":[{"id":"491da074-347b-4680-8239-2a230e4cdd9e"},{"id":"not/an-id"},{"id":"591da074-347b-4680-8239-2a230e4cdd9e"}]}"#;

struct Missing;

impl lemonfiber_adapter_kit::credential::Holding for Missing {
    fn key(&self) -> Result<String, String> {
        Err("upstream.json could not be read: not found".to_owned())
    }
}

fn audiobookshelf(fake: &Arc<Fake>) -> Audiobookshelf {
    Audiobookshelf::new(
        fake.clone(),
        BASE,
        Arc::new(USERNAME.to_owned()),
        Arc::new(PASSWORD.to_owned()),
    )
}

fn signed_in_and(mut routes: Vec<(Method, &'static str, Answer)>) -> Arc<Fake> {
    routes.insert(0, (Method::Post, "/login", Answer::reply(200, SIGNED_IN)));
    Fake::by_route(routes)
}

fn asked(fake: &Fake) -> Vec<(Method, String)> {
    fake.requests()
        .into_iter()
        .map(|one| (one.method, one.url.trim_start_matches(BASE).to_owned()))
        .collect()
}

fn body_of(fake: &Fake, path: &str) -> Option<serde_json::Value> {
    fake.requests()
        .into_iter()
        .find(|one| one.url.ends_with(path) && one.body.is_some())
        .and_then(|one| one.body)
        .and_then(|body| serde_json::from_str(&body).ok())
}

fn header_of(fake: &Fake, path: &str, name: &str) -> Option<String> {
    fake.requests()
        .into_iter()
        .find(|one| one.url.contains(path))
        .and_then(|one| {
            one.headers
                .into_iter()
                .find(|(held, _)| held == name)
                .map(|(_, value)| value)
        })
}

fn refused_saying(outcome: Result<impl std::fmt::Debug, Failure>, said: &str) -> bool {
    matches!(outcome, Err(Failure::Refused { detail, .. }) if detail.contains(said))
}

fn unsupported(outcome: Result<impl std::fmt::Debug, Failure>) -> bool {
    matches!(outcome, Err(Failure::Unsupported { service, .. }) if service == "audiobookshelf")
}

#[tokio::test]
async fn it_answers_while_its_health_check_succeeds() {
    let up = Fake::always(Answer::reply(200, r#"{"success":true}"#));
    assert!(audiobookshelf(&up).answers().await);
    assert_eq!(asked(&up), [(Method::Get, "/healthcheck".to_owned())]);
    for down in [Fake::silent(), Fake::always(Answer::reply(500, ""))] {
        assert!(!audiobookshelf(&down).answers().await);
    }
}

#[tokio::test]
async fn one_session_is_opened_as_the_root_and_carried_on_every_read_after() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/libraries",
        Answer::reply(200, r#"{"libraries":[]}"#),
    )]);
    let audiobookshelf = audiobookshelf(&fake);
    assert!(audiobookshelf.libraries().await.is_ok());
    assert!(audiobookshelf.libraries().await.is_ok());
    assert_eq!(
        asked(&fake),
        [
            (Method::Post, "/login".to_owned()),
            (Method::Get, "/api/libraries".to_owned()),
            (Method::Get, "/api/libraries".to_owned()),
        ]
    );
    assert_eq!(
        body_of(&fake, "/login"),
        Some(serde_json::json!({ "username": USERNAME, "password": PASSWORD }))
    );
    assert_eq!(
        header_of(&fake, "/login", "x-return-tokens").as_deref(),
        Some("true")
    );
    assert_eq!(
        header_of(&fake, "/login", "Content-Type").as_deref(),
        Some("application/json")
    );
    assert!(fake
        .requests()
        .iter()
        .filter(|one| one.url.contains("/api/"))
        .all(|one| one
            .headers
            .contains(&("Authorization".to_owned(), format!("Bearer {TOKEN}")))));
}

#[tokio::test]
async fn a_session_audiobookshelf_refuses_is_opened_again_once() {
    let fake = Fake::by_route_in_turn(vec![
        (Method::Post, "/login", vec![Answer::reply(200, SIGNED_IN)]),
        (
            Method::Get,
            "/api/libraries",
            vec![
                Answer::reply(401, "Unauthorized"),
                Answer::reply(200, r#"{"libraries":[]}"#),
            ],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake).libraries().await.ok(),
        Some(Vec::new())
    );
    assert_eq!(
        asked(&fake),
        [
            (Method::Post, "/login".to_owned()),
            (Method::Get, "/api/libraries".to_owned()),
            (Method::Post, "/login".to_owned()),
            (Method::Get, "/api/libraries".to_owned()),
        ]
    );
}

#[tokio::test]
async fn a_session_with_no_token_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"{"user":{"accessToken":""}}"#));
    assert!(refused_saying(
        audiobookshelf(&fake).libraries().await,
        "no token"
    ));
}

#[tokio::test]
async fn a_root_it_does_not_hold_is_refused_before_anything_is_asked() {
    let fake = Fake::silent();
    let audiobookshelf = Audiobookshelf::new(
        fake.clone(),
        BASE,
        Arc::new(Missing),
        Arc::new(PASSWORD.to_owned()),
    );
    assert!(refused_saying(
        audiobookshelf.libraries().await,
        "holds no root account"
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn the_libraries_are_the_ones_spelled_as_its_identifiers_are() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/libraries",
        Answer::reply(200, LIBRARIES),
    )]);
    assert_eq!(
        audiobookshelf(&fake).libraries().await.ok(),
        Some(vec![
            "491da074-347b-4680-8239-2a230e4cdd9e".to_owned(),
            "591da074-347b-4680-8239-2a230e4cdd9e".to_owned(),
        ])
    );
}

#[test]
fn an_identifier_is_thirty_two_hexadecimal_marks_with_or_without_hyphens() {
    assert!(an_id("c747698d-e2f9-41b8-8770-35e2e2c5fd1a"));
    assert!(an_id("00000000000000000000000000000000"));
    assert!(!an_id("c747698d"));
    assert!(!an_id("c747698d-e2f9-41b8-8770-35e2e2c5fd1a-0"));
    assert!(!an_id("../../api/me/../../../../../../../"));
}

#[tokio::test]
async fn nobody_holds_it_while_its_first_run_has_not_made_a_root() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/status",
        Answer::reply(200, UNINITIALISED),
    )]);
    assert_eq!(audiobookshelf(&fake).owner().await.ok(), Some(Owner::Open));
    assert_eq!(asked(&fake), [(Method::Get, "/status".to_owned())]);
}

#[tokio::test]
async fn it_is_ours_where_the_minted_root_signs_in() {
    let fake = signed_in_and(vec![
        (Method::Get, "/status", Answer::reply(200, INITIALISED)),
        (Method::Get, "/api/me", Answer::reply(200, r#"{"id":"u"}"#)),
    ]);
    assert_eq!(audiobookshelf(&fake).owner().await.ok(), Some(Owner::Ours));
    assert_eq!(
        asked(&fake),
        [
            (Method::Get, "/status".to_owned()),
            (Method::Post, "/login".to_owned()),
            (Method::Get, "/api/me".to_owned()),
        ]
    );
}

#[tokio::test]
async fn it_is_theirs_where_the_minted_root_is_refused() {
    for (login, me) in [
        (Answer::reply(401, "Unauthorized"), Answer::reply(200, "{}")),
        (
            Answer::reply(200, SIGNED_IN),
            Answer::reply(403, "Forbidden"),
        ),
    ] {
        let fake = Fake::by_route(vec![
            (Method::Get, "/status", Answer::reply(200, INITIALISED)),
            (Method::Post, "/login", login),
            (Method::Get, "/api/me", me),
        ]);
        assert_eq!(
            audiobookshelf(&fake).owner().await.ok(),
            Some(Owner::Theirs)
        );
    }
}

#[tokio::test]
async fn an_owner_it_cannot_read_is_refused() {
    let unreadable = Fake::always(Answer::reply(200, "<html>"));
    assert!(refused_saying(
        audiobookshelf(&unreadable).owner().await,
        "root account could not be read"
    ));
    let failing = signed_in_and(vec![
        (Method::Get, "/status", Answer::reply(200, INITIALISED)),
        (Method::Get, "/api/me", Answer::reply(500, "")),
    ]);
    assert!(refused_saying(
        audiobookshelf(&failing).owner().await,
        "HTTP 500"
    ));
}

#[tokio::test]
async fn a_claim_makes_the_minted_root_while_nobody_holds_it() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/status", Answer::reply(200, UNINITIALISED)),
        (Method::Post, "/init", Answer::reply(200, "OK")),
    ]);
    assert_eq!(audiobookshelf(&fake).claim().await.ok(), Some(Owner::Ours));
    assert_eq!(
        asked(&fake),
        [
            (Method::Get, "/status".to_owned()),
            (Method::Post, "/init".to_owned()),
        ]
    );
    assert_eq!(
        body_of(&fake, "/init"),
        Some(serde_json::json!({
            "newRoot": { "username": USERNAME, "password": PASSWORD }
        }))
    );
}

#[tokio::test]
async fn a_claim_changes_nothing_somebody_already_holds() {
    let fake = signed_in_and(vec![
        (Method::Get, "/status", Answer::reply(200, INITIALISED)),
        (Method::Get, "/api/me", Answer::reply(200, "{}")),
    ]);
    assert_eq!(audiobookshelf(&fake).claim().await.ok(), Some(Owner::Ours));
    assert!(!fake.asked_for("/init"));
}

#[tokio::test]
async fn a_claim_another_first_run_beat_answers_who_holds_it_now() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/status",
            vec![
                Answer::reply(200, UNINITIALISED),
                Answer::reply(200, INITIALISED),
            ],
        ),
        (Method::Post, "/init", vec![Answer::reply(500, "")]),
        (
            Method::Post,
            "/login",
            vec![Answer::reply(401, "Unauthorized")],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake).claim().await.ok(),
        Some(Owner::Theirs)
    );
}

#[tokio::test]
async fn a_claim_refused_while_nobody_holds_it_is_a_refusal() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/status", Answer::reply(200, UNINITIALISED)),
        (Method::Post, "/init", Answer::reply(500, "")),
    ]);
    assert!(refused_saying(
        audiobookshelf(&fake).claim().await,
        "HTTP 500"
    ));
}
