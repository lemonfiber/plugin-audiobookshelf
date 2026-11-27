use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{
    Allowing as _, Declaring as _, Encrypting as _, Fronted as _, Library as _, Recounting as _,
    Tuning as _, Upkeep as _,
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
    assert!(unsupported(
        audiobookshelf
            .events(
                Some("https://lemonfiber/events"),
                Some("p"),
                Some("k"),
                Some(60)
            )
            .await
    ));
    assert!(unsupported(audiobookshelf.channels("m").await));
    assert!(unsupported(audiobookshelf.guide("m", 12).await));
    assert!(unsupported(audiobookshelf.tune("m", "c").await));
    assert!(unsupported(audiobookshelf.untune("c").await));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn every_origin_asked_for_is_written_and_held_to_being_kept() {
    let origins = [ORIGIN.to_owned(), "https://console.example".to_owned()];
    let fake = signed_in_and(vec![(
        Method::Patch,
        "/api/settings",
        Answer::reply(
            200,
            r#"{"serverSettings":{"allowedOrigins":["https://watch.example","https://console.example"]}}"#,
        ),
    )]);
    assert!(audiobookshelf(&fake).allow_origins(&origins).await.is_ok());
    assert_eq!(
        body_of(&fake, "/api/settings"),
        Some(serde_json::json!({ "allowedOrigins": origins }))
    );
    let kept_one = signed_in_and(vec![(
        Method::Patch,
        "/api/settings",
        Answer::reply(
            200,
            r#"{"serverSettings":{"allowedOrigins":["https://watch.example"]}}"#,
        ),
    )]);
    assert!(refused_saying(
        audiobookshelf(&kept_one).allow_origins(&origins).await,
        "other origins"
    ));
}

#[tokio::test]
async fn no_origin_or_a_wildcard_among_them_is_refused_before_anything_is_written() {
    for origins in [vec![], vec![ORIGIN.to_owned(), "*".to_owned()]] {
        let fake = Fake::silent();
        assert!(refused_saying(
            audiobookshelf(&fake).allow_origins(&origins).await,
            "never a wildcard"
        ));
        assert!(fake.requests().is_empty());
    }
}

#[tokio::test]
async fn a_book_whose_title_holds_the_term_is_in_a_library() {
    let fake = signed_in_and(vec![
        (
            Method::Get,
            "/api/libraries/491da074-347b-4680-8239-2a230e4cdd9e/search",
            Answer::reply(200, r#"{"book":[]}"#),
        ),
        (
            Method::Get,
            "/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/search",
            Answer::reply(
                200,
                r#"{"book":[{"libraryItem":{"media":{"metadata":{"title":"The Hobbit"}}}},{"libraryItem":{"media":{"metadata":{}}}}]}"#,
            ),
        ),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    let audiobookshelf = audiobookshelf(&fake);
    assert_eq!(
        audiobookshelf
            .has_item(TitleKind::Books, "HOBBIT")
            .await
            .ok(),
        Some(true)
    );
    assert_eq!(
        audiobookshelf.has_item(TitleKind::Books, "Dune").await.ok(),
        Some(false)
    );
    assert!(fake.requests().iter().any(|one| one
        .url
        .ends_with("/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/search?q=HOBBIT")));
    assert!(!fake
        .requests()
        .iter()
        .any(|one| one.url.contains("not/an-id")));
}

#[tokio::test]
async fn an_audiobook_is_found_among_the_books_and_a_podcast_among_the_podcasts() {
    let fake = signed_in_and(vec![
        (
            Method::Get,
            "/api/libraries/491da074-347b-4680-8239-2a230e4cdd9e/search",
            Answer::reply(
                200,
                r#"{"podcast":[{"libraryItem":{"media":{"metadata":{"title":"Darknet Diaries"}}}}]}"#,
            ),
        ),
        (
            Method::Get,
            "/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/search",
            Answer::reply(
                200,
                r#"{"book":[{"libraryItem":{"media":{"metadata":{"title":"The Hobbit"}}}}]}"#,
            ),
        ),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    let audiobookshelf = audiobookshelf(&fake);
    for (kind, term, held) in [
        (TitleKind::Audiobooks, "hobbit", true),
        (TitleKind::Audiobooks, "darknet", false),
        (TitleKind::Podcasts, "darknet", true),
        (TitleKind::Podcasts, "hobbit", false),
    ] {
        assert_eq!(
            audiobookshelf.has_item(kind, term).await.ok(),
            Some(held),
            "{kind:?} {term}"
        );
    }
}

#[tokio::test]
async fn podcasts_it_cannot_read_leave_it_unable_to_say() {
    let fake = signed_in_and(vec![
        (Method::Get, "/search", Answer::reply(200, "{")),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    assert!(refused_saying(
        audiobookshelf(&fake)
            .has_item(TitleKind::Podcasts, "t")
            .await,
        "podcasts Audiobookshelf holds could not be read"
    ));
}

#[tokio::test]
async fn nothing_but_books_is_in_a_library_and_nothing_is_asked_for_it() {
    let fake = Fake::silent();
    for kind in [TitleKind::Tv, TitleKind::Movies, TitleKind::Music] {
        assert_eq!(
            audiobookshelf(&fake).has_item(kind, "t").await.ok(),
            Some(false)
        );
    }
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn books_it_cannot_read_leave_it_unable_to_say() {
    let fake = signed_in_and(vec![
        (Method::Get, "/search", Answer::reply(200, "{")),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    assert!(refused_saying(
        audiobookshelf(&fake).has_item(TitleKind::Books, "t").await,
        "books Audiobookshelf holds could not be read"
    ));
}

#[tokio::test]
async fn origins_it_cannot_read_back_are_refused() {
    let fake = signed_in_and(vec![(
        Method::Patch,
        "/api/settings",
        Answer::reply(200, "{"),
    )]);
    assert!(refused_saying(
        audiobookshelf(&fake)
            .allow_origins(&[ORIGIN.to_owned()])
            .await,
        "could not be read back"
    ));
}
