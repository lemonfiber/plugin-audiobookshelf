use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Declaring as _, Encrypting as _, Fronted as _, Library as _, Upkeep as _,
};

use super::{
    asked, audiobookshelf, body_of, refused_saying, signed_in_and, unsupported, LIBRARIES,
};

const ITEM: &str = "c747698d-e2f9-41b8-8770-35e2e2c5fd1a";

const ORIGIN: &str = "https://watch.example";

#[tokio::test]
async fn a_rescan_looks_over_every_library_again() {
    let fake = signed_in_and(vec![
        (Method::Post, "/scan", Answer::reply(200, "")),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    assert!(audiobookshelf(&fake).rescan().await.is_ok());
    assert_eq!(
        asked(&fake)
            .into_iter()
            .filter(|(method, _)| *method == Method::Post)
            .map(|(_, path)| path)
            .collect::<Vec<_>>(),
        [
            "/login",
            "/api/libraries/491da074-347b-4680-8239-2a230e4cdd9e/scan",
            "/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/scan",
        ]
    );
}

#[tokio::test]
async fn a_rescan_it_refuses_is_a_refusal() {
    let fake = signed_in_and(vec![
        (Method::Post, "/scan", Answer::reply(500, "")),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    assert!(refused_saying(
        audiobookshelf(&fake).rescan().await,
        "HTTP 500"
    ));
}

#[tokio::test]
async fn a_refresh_scans_that_one_item_again() {
    let fake = signed_in_and(vec![(
        Method::Post,
        "/scan",
        Answer::reply(200, r#"{"result":"UPTODATE"}"#),
    )]);
    assert!(audiobookshelf(&fake).refresh(ITEM).await.is_ok());
    assert!(asked(&fake).contains(&(Method::Post, format!("/api/items/{ITEM}/scan"))));
    let absent = signed_in_and(vec![(Method::Post, "/scan", Answer::reply(404, ""))]);
    assert!(refused_saying(
        audiobookshelf(&absent).refresh(ITEM).await,
        "HTTP 404"
    ));
}

#[tokio::test]
async fn a_refresh_of_nothing_it_could_hold_is_refused_before_anything_is_asked() {
    let fake = Fake::silent();
    assert!(refused_saying(
        audiobookshelf(&fake).refresh("../../api/me").await,
        "names nothing"
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn the_origins_it_allows_are_its_own_settings() {
    let fake = signed_in_and(vec![(
        Method::Post,
        "/api/authorize",
        Answer::reply(
            200,
            r#"{"user":{},"serverSettings":{"allowedOrigins":["https://watch.example"]}}"#,
        ),
    )]);
    assert_eq!(
        audiobookshelf(&fake).allowed_origins().await.ok(),
        Some(vec![ORIGIN.to_owned()])
    );
}

#[tokio::test]
async fn one_origin_alone_is_written_and_held_to_having_been_kept() {
    let fake = signed_in_and(vec![(
        Method::Patch,
        "/api/settings",
        Answer::reply(
            200,
            r#"{"serverSettings":{"allowedOrigins":["https://watch.example"]}}"#,
        ),
    )]);
    assert!(audiobookshelf(&fake).allow_only(ORIGIN).await.is_ok());
    assert_eq!(
        body_of(&fake, "/api/settings"),
        Some(serde_json::json!({ "allowedOrigins": [ORIGIN] }))
    );
    let kept_another = signed_in_and(vec![(
        Method::Patch,
        "/api/settings",
        Answer::reply(200, r#"{"serverSettings":{"allowedOrigins":[]}}"#),
    )]);
    assert!(refused_saying(
        audiobookshelf(&kept_another).allow_only(ORIGIN).await,
        "other origins"
    ));
}

#[tokio::test]
async fn an_empty_origin_or_a_wildcard_is_refused_before_anything_is_written() {
    for origin in ["", " ", "*", "https://*.example"] {
        let fake = Fake::silent();
        assert!(refused_saying(
            audiobookshelf(&fake).allow_only(origin).await,
            "never a wildcard"
        ));
        assert!(fake.requests().is_empty());
    }
}

#[tokio::test]
async fn what_it_has_no_equivalent_for_is_unsupported_and_asks_nothing() {
    let fake = Fake::silent();
    let audiobookshelf = audiobookshelf(&fake);
    assert!(unsupported(
        audiobookshelf.has_item(Kind::Movies, "t").await
    ));
    assert!(unsupported(audiobookshelf.series_held(10).await));
    assert!(unsupported(audiobookshelf.known_proxies().await));
    assert!(unsupported(audiobookshelf.trust_only("10.0.0.1").await));
    assert!(unsupported(audiobookshelf.restart().await));
    assert!(unsupported(audiobookshelf.declared().await));
    assert!(unsupported(
        audiobookshelf
            .serve_tls("/config/internal.p12", "pin")
            .await
    ));
    assert!(fake.requests().is_empty());
}
