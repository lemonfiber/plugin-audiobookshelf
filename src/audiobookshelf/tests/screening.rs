use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Failure, Holds, HowFar, Image, Item, ItemDetail, Medium, Picture, Playback, Screening as _,
    PICTURE_MOST,
};

use super::super::screening::{a_date, whole_minutes};
use super::{
    asked, audiobookshelf, refused_saying, signed_in_and, unsupported, LIBRARIES, SIGNED_IN,
};

const ITEM: &str = "c747698d-e2f9-41b8-8770-35e2e2c5fd1a";

const BOOK: &str = r#"{
    "id": "c747698d-e2f9-41b8-8770-35e2e2c5fd1a",
    "mediaType": "book",
    "media": {
        "coverPath": "/metadata/items/c747698d/cover.jpg",
        "numTracks": 3,
        "duration": 3725.4,
        "metadata": {
            "title": "Some Book",
            "publishedYear": "2019",
            "publishedDate": "2019-05-01",
            "descriptionPlain": "What it is about.",
            "description": "<p>What it is about.</p>",
            "genres": ["Fantasy"]
        }
    }
}"#;

const PODCAST: &str = r#"{
    "id": "d747698d-e2f9-41b8-8770-35e2e2c5fd1a",
    "mediaType": "podcast",
    "media": {
        "metadata": {
            "title": "Some Podcast",
            "publishedYear": "a while ago",
            "publishedDate": "2019",
            "description": " "
        }
    }
}"#;

fn book() -> Item {
    Item {
        id: ITEM.to_owned(),
        title: "Some Book".to_owned(),
        year: Some(2019),
        medium: Medium::Other,
        holds: Holds {
            poster: true,
            backdrop: false,
            plays: true,
        },
    }
}

fn podcast() -> Item {
    Item {
        id: "d747698d-e2f9-41b8-8770-35e2e2c5fd1a".to_owned(),
        title: "Some Podcast".to_owned(),
        year: None,
        medium: Medium::Other,
        holds: Holds::default(),
    }
}

#[tokio::test]
async fn a_title_is_read_with_everything_audiobookshelf_holds_about_it() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/items/c747698d-e2f9-41b8-8770-35e2e2c5fd1a",
        Answer::reply(200, BOOK),
    )]);
    assert_eq!(
        audiobookshelf(&fake).title(None, ITEM).await.ok().flatten(),
        Some(ItemDetail {
            item: book(),
            overview: Some("What it is about.".to_owned()),
            minutes: Some(62),
            genres: vec!["Fantasy".to_owned()],
            certificate: None,
            released: Some("2019-05-01".to_owned()),
            seasons: Vec::new(),
        })
    );
    assert!(asked(&fake).contains(&(Method::Get, format!("/api/items/{ITEM}?expanded=1"))));
}

#[tokio::test]
async fn what_a_title_does_not_hold_is_answered_as_nothing() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/items/",
        Answer::reply(200, PODCAST),
    )]);
    assert_eq!(
        audiobookshelf(&fake).title(None, ITEM).await.ok().flatten(),
        Some(ItemDetail {
            item: podcast(),
            overview: None,
            minutes: None,
            genres: Vec::new(),
            certificate: None,
            released: None,
            seasons: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_title_nothing_holds_is_answered_as_none() {
    let absent = signed_in_and(vec![(
        Method::Get,
        "expanded=1",
        Answer::reply(404, "Not Found"),
    )]);
    assert!(matches!(
        audiobookshelf(&absent).title(None, ITEM).await,
        Ok(None)
    ));
    let never_asked = Fake::silent();
    assert!(matches!(
        audiobookshelf(&never_asked).title(None, "../api/me").await,
        Ok(None)
    ));
    assert!(never_asked.requests().is_empty());
    let failing = signed_in_and(vec![(Method::Get, "expanded=1", Answer::reply(500, ""))]);
    assert!(refused_saying(
        audiobookshelf(&failing).title(None, ITEM).await,
        "HTTP 500"
    ));
}

#[tokio::test]
async fn a_poster_is_its_cover_as_audiobookshelf_answered_it() {
    let fake = signed_in_and(vec![
        (
            Method::Get,
            "/cover",
            Answer::served(200, "image/gif", "GIF89a-picture"),
        ),
        (Method::Get, "expanded=1", Answer::reply(200, BOOK)),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .picture(None, ITEM, Picture::Poster)
            .await
            .ok()
            .flatten(),
        Some(Image {
            media_type: "image/gif".to_owned(),
            bytes: b"GIF89a-picture".to_vec(),
        })
    );
    assert!(asked(&fake).contains(&(Method::Get, format!("/api/items/{ITEM}/cover"))));
}

#[tokio::test]
async fn a_cover_it_refused_the_session_for_is_asked_again_once() {
    let fake = Fake::by_route_in_turn(vec![
        (Method::Post, "/login", vec![Answer::reply(200, SIGNED_IN)]),
        (
            Method::Get,
            "/cover",
            vec![
                Answer::reply(401, "Unauthorized"),
                Answer::served(200, "image/webp", "RIFF0000WEBPVP8 "),
            ],
        ),
        (Method::Get, "expanded=1", vec![Answer::reply(200, BOOK)]),
    ]);
    assert!(matches!(
        audiobookshelf(&fake)
            .picture(None, ITEM, Picture::Poster)
            .await,
        Ok(Some(image)) if image.media_type == "image/webp"
    ));
    assert_eq!(
        fake.requests()
            .iter()
            .filter(|one| one.url.ends_with("/login"))
            .count(),
        2
    );
}

#[tokio::test]
async fn a_picture_it_does_not_hold_is_answered_as_none() {
    let oversized = "x".repeat(PICTURE_MOST + 1);
    for cover in [
        Answer::reply(404, "Not Found"),
        Answer::served(200, "text/html", "<html>"),
        Answer::served(
            200,
            "image/svg+xml",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#,
        ),
        Answer::served(200, "image/png", "<html><script>alert(1)</script></html>"),
        Answer::reply(200, "no type"),
        Answer::served(200, "image/jpeg", oversized),
    ] {
        let fake = signed_in_and(vec![
            (Method::Get, "/cover", cover),
            (Method::Get, "expanded=1", Answer::reply(200, BOOK)),
        ]);
        assert!(matches!(
            audiobookshelf(&fake)
                .picture(None, ITEM, Picture::Poster)
                .await,
            Ok(None)
        ));
    }
    let untitled = signed_in_and(vec![(Method::Get, "expanded=1", Answer::reply(404, ""))]);
    assert!(matches!(
        audiobookshelf(&untitled)
            .picture(None, ITEM, Picture::Poster)
            .await,
        Ok(None)
    ));
    assert!(!untitled.asked_for("/cover"));
    let backdrop = Fake::silent();
    assert!(matches!(
        audiobookshelf(&backdrop)
            .picture(None, ITEM, Picture::Backdrop)
            .await,
        Ok(None)
    ));
    assert!(backdrop.requests().is_empty());
}

#[tokio::test]
async fn a_cover_it_would_not_answer_is_refused() {
    let refused = signed_in_and(vec![
        (Method::Get, "/cover", Answer::reply(500, "")),
        (Method::Get, "expanded=1", Answer::reply(200, BOOK)),
    ]);
    assert!(refused_saying(
        audiobookshelf(&refused)
            .picture(None, ITEM, Picture::Poster)
            .await,
        "would not answer the cover"
    ));
    let unreachable = signed_in_and(vec![(Method::Get, "expanded=1", Answer::reply(200, BOOK))]);
    assert!(matches!(
        audiobookshelf(&unreachable)
            .picture(None, ITEM, Picture::Poster)
            .await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn holdings_are_read_library_by_library_until_there_are_as_many_as_were_asked_for() {
    let fake = signed_in_and(vec![
        (
            Method::Get,
            "/api/libraries/491da074-347b-4680-8239-2a230e4cdd9e/items",
            Answer::reply(200, format!(r#"{{"results":[{BOOK}]}}"#)),
        ),
        (
            Method::Get,
            "/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/items",
            Answer::reply(200, format!(r#"{{"results":[{PODCAST},{BOOK}]}}"#)),
        ),
        (Method::Get, "/api/libraries", Answer::reply(200, LIBRARIES)),
    ]);
    assert_eq!(
        audiobookshelf(&fake).holdings(None, 2).await.ok(),
        Some(vec![book(), podcast()])
    );
    assert_eq!(
        asked(&fake)
            .into_iter()
            .filter(|(_, path)| path.contains("/items"))
            .map(|(_, path)| path)
            .collect::<Vec<_>>(),
        [
            "/api/libraries/491da074-347b-4680-8239-2a230e4cdd9e/items?limit=2&sort=media.metadata.title",
            "/api/libraries/591da074-347b-4680-8239-2a230e4cdd9e/items?limit=1&sort=media.metadata.title",
        ]
    );
}

#[tokio::test]
async fn no_library_is_read_once_the_holdings_are_full() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/libraries",
        Answer::reply(200, LIBRARIES),
    )]);
    assert_eq!(
        audiobookshelf(&fake).holdings(None, 0).await.ok(),
        Some(Vec::new())
    );
    assert!(!fake.asked_for("/items"));
}

#[tokio::test]
async fn nothing_is_playing_where_no_session_is_open() {
    let fake = signed_in_and(vec![(
        Method::Get,
        "/api/users/online",
        Answer::reply(200, r#"{"usersOnline":[],"openSessions":[]}"#),
    )]);
    assert_eq!(
        audiobookshelf(&fake).playing(None).await.ok(),
        Some(Vec::new())
    );
    assert!(!asked(&fake).contains(&(Method::Get, "/api/users".to_owned())));
}

#[tokio::test]
async fn every_open_session_is_somebody_listening_named_by_their_account() {
    let fake = signed_in_and(vec![
        (
            Method::Get,
            "/api/users/online",
            Answer::reply(
                200,
                r#"{"openSessions":[
                    {"userId":"u1","displayTitle":"Some Book","episodeId":null,
                     "mediaMetadata":{"title":"Some Book"},
                     "deviceInfo":{"deviceName":"Pixel 8","clientName":"Abs Android"}},
                    {"userId":"u2","displayTitle":"Episode 4","episodeId":"e4",
                     "mediaMetadata":{"title":"Some Podcast"},
                     "deviceInfo":{"clientName":"Abs Web"}},
                    {"userId":"gone","displayTitle":"Other Book"}
                ]}"#,
            ),
        ),
        (
            Method::Get,
            "/api/users",
            Answer::reply(
                200,
                r#"{"users":[{"id":"u1","username":"ann"},{"id":"u2","username":"bo"}]}"#,
            ),
        ),
    ]);
    let listening = |member_id: &str, member: &str, title: &str| Playback {
        member_id: member_id.to_owned(),
        member: member.to_owned(),
        title: title.to_owned(),
        series: None,
        season: None,
        episode: None,
        medium: Medium::Other,
        paused: false,
        device: String::new(),
    };
    assert_eq!(
        audiobookshelf(&fake).playing(None).await.ok(),
        Some(vec![
            Playback {
                device: "Pixel 8".to_owned(),
                ..listening("u1", "ann", "Some Book")
            },
            Playback {
                series: Some("Some Podcast".to_owned()),
                medium: Medium::Episode,
                device: "Abs Web".to_owned(),
                ..listening("u2", "bo", "Episode 4")
            },
            listening("gone", "", "Other Book"),
        ])
    );
}

#[tokio::test]
async fn whatever_names_a_member_or_a_device_is_unsupported_and_asks_nothing() {
    let fake = Fake::silent();
    let audiobookshelf = audiobookshelf(&fake);
    let how_far = HowFar {
        position: 60,
        ended: false,
    };
    assert!(unsupported(audiobookshelf.signed_in("m", "d").await));
    assert!(unsupported(audiobookshelf.title(Some("m"), ITEM).await));
    assert!(unsupported(audiobookshelf.part_way("m", 10).await));
    assert!(unsupported(
        audiobookshelf.progressed("m", ITEM, &how_far).await
    ));
    assert!(unsupported(audiobookshelf.sign_out("d").await));
    assert!(unsupported(
        audiobookshelf
            .picture(Some("m"), ITEM, Picture::Poster)
            .await
    ));
    assert!(unsupported(audiobookshelf.holdings(Some("m"), 10).await));
    assert!(unsupported(audiobookshelf.playing(Some("m")).await));
    assert!(fake.requests().is_empty());
}

#[test]
fn a_length_is_whole_minutes_and_nothing_else_is() {
    assert_eq!(whole_minutes(3725.4), Some(62));
    assert_eq!(whole_minutes(5.0), Some(0));
    assert_eq!(whole_minutes(-120.0), None);
    assert_eq!(whole_minutes(f64::NAN), None);
    assert_eq!(whole_minutes(f64::INFINITY), None);
}

#[test]
fn a_release_date_is_a_calendar_date() {
    assert!(a_date("2019-05-01"));
    assert!(!a_date("2019"));
    assert!(!a_date("2019/05/01"));
    assert!(!a_date("20190-5-01"));
}
