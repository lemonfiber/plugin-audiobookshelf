use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{AppKeys as _, Dated, Failure};

use super::{
    asked, audiobookshelf, body_of, refused_saying, signed_in_and, unsupported, SIGNED_IN,
};

const APP: &str = "lemonfiber-conformance";

const KEY_ID: &str = "36843377-c075-467d-97e3-f6265f483638";

const LISTED: &str = r#"{"apiKeys":[
    {"id":"46843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance","createdAt":"2026-10-25T03:05:00.000Z","lastUsedAt":null},
    {"id":"56843377-c075-467d-97e3-f6265f483638","name":"another-app","createdAt":"2026-10-25T03:00:00.000Z"},
    {"id":"36843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance","createdAt":"2026-10-25T03:04:00.000Z","lastUsedAt":"2026-10-25T03:06:00.000Z"}
]}"#;

fn minted_as(claims: &serde_json::Value) -> String {
    format!(
        "eyJhbGciOiJIUzI1NiJ9.{}.c2lnbmVk",
        URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

fn a_minted_key() -> String {
    minted_as(&serde_json::json!({ "keyId": KEY_ID, "name": APP, "type": "api", "iat": 1 }))
}

#[tokio::test]
async fn the_keys_filed_under_a_name_are_unsupported_since_each_is_shown_once() {
    let fake = Fake::silent();
    assert!(unsupported(audiobookshelf(&fake).filed_as(APP).await));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn every_key_filed_under_a_name_is_dated_oldest_first() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/api-keys",
        Answer::reply(200, LISTED),
    )]);
    assert_eq!(
        audiobookshelf(&fake).dated(APP).await.ok(),
        Some(vec![
            Dated {
                created: Some("2026-10-25T03:04:00.000Z".to_owned()),
                last_used: Some("2026-10-25T03:06:00.000Z".to_owned()),
            },
            Dated {
                created: Some("2026-10-25T03:05:00.000Z".to_owned()),
                last_used: None,
            },
        ])
    );
}

fn minting(after: &'static str, minted: &'static str) -> std::sync::Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (Method::Post, "/login", vec![Answer::reply(200, SIGNED_IN)]),
        (
            Method::Get,
            "/api/me",
            vec![Answer::reply(
                200,
                r#"{"id":"f6d39a15-3965-4abd-9df8-903da8b4e362"}"#,
            )],
        ),
        (
            Method::Get,
            "/api/api-keys",
            vec![
                Answer::reply(
                    200,
                    r#"{"apiKeys":[{"id":"46843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"}]}"#,
                ),
                Answer::reply(200, after),
            ],
        ),
        (
            Method::Post,
            "/api/api-keys",
            vec![Answer::reply(200, minted)],
        ),
    ])
}

const AFTER_ONE: &str = r#"{"apiKeys":[
    {"id":"46843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"},
    {"id":"36843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"}
]}"#;

const MINTED: &str = r#"{"apiKey":{"id":"36843377-c075-467d-97e3-f6265f483638","apiKey":"the.minted.key","name":"lemonfiber-conformance"}}"#;

#[tokio::test]
async fn a_minted_key_is_the_one_new_key_filed_under_the_name_made_for_the_root() {
    let fake = minting(AFTER_ONE, MINTED);
    assert_eq!(
        audiobookshelf(&fake).mint(APP).await.ok().as_deref(),
        Some("the.minted.key")
    );
    assert_eq!(
        body_of(&fake, "/api/api-keys"),
        Some(serde_json::json!({
            "name": APP,
            "userId": "f6d39a15-3965-4abd-9df8-903da8b4e362",
            "isActive": true,
        }))
    );
}

#[tokio::test]
async fn a_mint_that_is_not_exactly_one_new_key_is_refused() {
    let two_new = r#"{"apiKeys":[
        {"id":"36843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"},
        {"id":"66843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"}
    ]}"#;
    let another_new = r#"{"apiKeys":[{"id":"66843377-c075-467d-97e3-f6265f483638","name":"lemonfiber-conformance"}]}"#;
    let keyless = r#"{"apiKey":{"id":"36843377-c075-467d-97e3-f6265f483638","apiKey":""}}"#;
    for (after, minted) in [
        (two_new, MINTED),
        (another_new, MINTED),
        (AFTER_ONE, keyless),
    ] {
        assert!(refused_saying(
            audiobookshelf(&minting(after, minted)).mint(APP).await,
            "as the one new key"
        ));
    }
}

#[tokio::test]
async fn whether_it_answers_to_a_key_is_asked_with_that_key_alone() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/api/me",
        Answer::reply(200, r#"{"id":"u"}"#),
    )]);
    let key = a_minted_key();
    assert!(audiobookshelf(&fake).answers_to(&key).await.is_ok());
    assert_eq!(asked(&fake), [(Method::Get, "/api/me".to_owned())]);
    assert!(fake.requests().iter().all(|one| one
        .headers
        .contains(&("Authorization".to_owned(), format!("Bearer {key}")))));
    let refused = Fake::always(Answer::reply(401, "Unauthorized"));
    assert!(matches!(
        audiobookshelf(&refused).answers_to(&key).await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_key_not_spelled_as_one_is_refused_before_anything_is_asked() {
    let fake = Fake::silent();
    let audiobookshelf = audiobookshelf(&fake);
    assert!(refused_saying(
        audiobookshelf.answers_to("not a key").await,
        "not spelled as a key"
    ));
    assert!(refused_saying(
        audiobookshelf.revoke("not a key").await,
        "not spelled as a key"
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_revoked_key_is_the_one_its_claims_name_among_the_keys_held() {
    let fake = signed_in_and(vec![
        (Method::Get, "/api/api-keys", Answer::reply(200, LISTED)),
        (Method::Delete, "/api/api-keys/", Answer::reply(200, "")),
    ]);
    assert!(audiobookshelf(&fake).revoke(&a_minted_key()).await.is_ok());
    assert!(asked(&fake).contains(&(Method::Delete, format!("/api/api-keys/{KEY_ID}"))));
}

#[tokio::test]
async fn a_key_it_holds_none_of_is_refused_and_nothing_is_revoked() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/api-keys",
        Answer::reply(200, r#"{"apiKeys":[]}"#),
    )]);
    assert!(refused_saying(
        audiobookshelf(&fake).revoke(&a_minted_key()).await,
        "holds no key"
    ));
    assert!(!asked(&fake)
        .iter()
        .any(|(method, _)| *method == Method::Delete));
}

#[tokio::test]
async fn a_key_audiobookshelf_did_not_mint_is_refused_before_anything_is_asked() {
    for key in [
        "no-claims-at-all".to_owned(),
        "head.a.signature".to_owned(),
        format!("head.{}.signature", URL_SAFE_NO_PAD.encode("not json")),
        minted_as(&serde_json::json!({ "keyId": "../users", "name": APP })),
    ] {
        let fake = Fake::silent();
        assert!(
            refused_saying(
                audiobookshelf(&fake).revoke(&key).await,
                "not a key Audiobookshelf minted"
            ),
            "{key}"
        );
        assert!(fake.requests().is_empty());
    }
}
