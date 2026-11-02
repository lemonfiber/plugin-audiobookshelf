use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, FiledAt, Furnished, Furnishing as _};

use super::{asked, audiobookshelf, refused_saying, SIGNED_IN};

const AUDIOBOOKS: &str = "/data/media/audiobooks";

const PODCASTS: &str = "/data/media/podcasts";

const NONE_HELD: &str = r#"{"libraries":[]}"#;

const EXISTS: &str = r#"{"posix":true,"directories":[]}"#;

const ASKED_AUDIOBOOKS: &str = "/api/filesystem?path=%2Fdata%2Fmedia%2Faudiobooks";

const ASKED_PODCASTS: &str = "/api/filesystem?path=%2Fdata%2Fmedia%2Fpodcasts";

fn filed(asked: &[(&str, &str)]) -> Vec<FiledAt> {
    asked
        .iter()
        .map(|(media_type, folder)| FiledAt {
            media_type: (*media_type).to_owned(),
            folder: (*folder).to_owned(),
        })
        .collect()
}

fn held(answered: &[(&str, bool)]) -> Vec<Furnished> {
    answered
        .iter()
        .map(|(media_type, held)| Furnished {
            media_type: (*media_type).to_owned(),
            held: *held,
        })
        .collect()
}

fn library(name: &str, folder: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "491da074-347b-4680-8239-2a230e4cdd9e",
        "name": name,
        "folders": [{ "id": "f", "fullPath": folder, "libraryId": "l" }],
    })
}

fn made(name: &str, folder: &str) -> String {
    library(name, folder).to_string()
}

fn holding(name: &str, folder: &str) -> String {
    serde_json::json!({ "libraries": [library(name, folder)] }).to_string()
}

fn audiobookshelf_answering(mut routes: Vec<(Method, &'static str, Vec<Answer>)>) -> Arc<Fake> {
    routes.insert(
        0,
        (Method::Post, "/login", vec![Answer::reply(200, SIGNED_IN)]),
    );
    Fake::by_route_in_turn(routes)
}

fn libraries_added(fake: &Fake) -> Vec<serde_json::Value> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == Method::Post && one.url.ends_with("/api/libraries"))
        .filter_map(|one| one.body)
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect()
}

fn sent(fake: &Fake) -> Vec<(Method, String)> {
    asked(fake)
        .into_iter()
        .filter(|(method, path)| *method != Method::Post || path != "/login")
        .collect()
}

#[tokio::test]
async fn a_library_is_added_for_each_media_type_it_takes_whose_folder_exists() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, NONE_HELD)],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(200, EXISTS)],
        ),
        (
            Method::Post,
            "/api/libraries",
            vec![
                Answer::reply(200, made("Audiobooks", AUDIOBOOKS)),
                Answer::reply(200, made("Podcasts", PODCASTS)),
            ],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[
                ("audiobooks", AUDIOBOOKS),
                ("podcasts", PODCASTS)
            ]))
            .await
            .ok(),
        Some(held(&[("audiobooks", true), ("podcasts", true)]))
    );
    assert_eq!(
        sent(&fake),
        [
            (Method::Get, "/api/libraries".to_owned()),
            (Method::Get, ASKED_AUDIOBOOKS.to_owned()),
            (Method::Post, "/api/libraries".to_owned()),
            (Method::Get, ASKED_PODCASTS.to_owned()),
            (Method::Post, "/api/libraries".to_owned()),
        ]
    );
    assert_eq!(
        libraries_added(&fake),
        [
            serde_json::json!({
                "name": "Audiobooks",
                "folders": [{ "fullPath": AUDIOBOOKS }],
                "mediaType": "book",
                "provider": "google",
            }),
            serde_json::json!({
                "name": "Podcasts",
                "folders": [{ "fullPath": PODCASTS }],
                "mediaType": "podcast",
                "provider": "itunes",
            }),
        ]
    );
}

#[tokio::test]
async fn a_library_already_reading_the_folder_is_left_as_it_is() {
    let fake = audiobookshelf_answering(vec![(
        Method::Get,
        "/api/libraries",
        vec![Answer::reply(
            200,
            holding("Listening", "/data/media/audiobooks/"),
        )],
    )]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", true)]))
    );
    assert_eq!(sent(&fake), [(Method::Get, "/api/libraries".to_owned())]);
}

#[tokio::test]
async fn a_library_under_the_title_it_would_add_reading_another_folder_is_left_and_not_held() {
    let fake = audiobookshelf_answering(vec![(
        Method::Get,
        "/api/libraries",
        vec![Answer::reply(200, holding("Audiobooks", "/audiobooks"))],
    )]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", false)]))
    );
    assert_eq!(sent(&fake), [(Method::Get, "/api/libraries".to_owned())]);
}

#[tokio::test]
async fn a_media_type_it_does_not_take_is_not_held() {
    let fake = audiobookshelf_answering(vec![(
        Method::Get,
        "/api/libraries",
        vec![Answer::reply(200, NONE_HELD)],
    )]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[
                ("movies", "/data/media/movies"),
                ("books", "/data/media/books"),
            ]))
            .await
            .ok(),
        Some(held(&[("movies", false), ("books", false)]))
    );
    assert_eq!(sent(&fake), [(Method::Get, "/api/libraries".to_owned())]);
}

#[tokio::test]
async fn a_folder_that_does_not_exist_is_not_held_and_given_no_library() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, NONE_HELD)],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(400, r#"Invalid "path" query string"#)],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", false)]))
    );
    assert_eq!(
        sent(&fake),
        [
            (Method::Get, "/api/libraries".to_owned()),
            (Method::Get, ASKED_AUDIOBOOKS.to_owned()),
        ]
    );
}

#[tokio::test]
async fn a_folder_audiobookshelf_refuses_to_read_is_not_held() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, NONE_HELD)],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(200, EXISTS)],
        ),
        (
            Method::Post,
            "/api/libraries",
            vec![Answer::reply(
                400,
                "Invalid request. Invalid folder directory",
            )],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", false)]))
    );
}

#[tokio::test]
async fn a_library_made_reading_another_folder_is_not_held() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, NONE_HELD)],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(200, EXISTS)],
        ),
        (
            Method::Post,
            "/api/libraries",
            vec![Answer::reply(200, made("Audiobooks", "/elsewhere"))],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", false)]))
    );
}

#[tokio::test]
async fn a_refusal_anywhere_is_a_failure() {
    for (method, path) in [
        (Method::Get, "/api/libraries"),
        (Method::Get, "/api/filesystem"),
        (Method::Post, "/api/libraries"),
    ] {
        let fake = audiobookshelf_answering(vec![
            (method, path, vec![Answer::reply(500, "")]),
            (
                Method::Get,
                "/api/libraries",
                vec![Answer::reply(200, NONE_HELD)],
            ),
            (
                Method::Get,
                "/api/filesystem",
                vec![Answer::reply(200, EXISTS)],
            ),
        ]);
        assert!(
            refused_saying(
                audiobookshelf(&fake)
                    .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
                    .await,
                "HTTP 500"
            ),
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_library_reading_another_folder_is_kept_as_it_is_and_one_added_beside_it() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, holding("Listening", "/audiobooks"))],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(200, EXISTS)],
        ),
        (
            Method::Post,
            "/api/libraries",
            vec![Answer::reply(200, made("Audiobooks", AUDIOBOOKS))],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await
            .ok(),
        Some(held(&[("audiobooks", true)]))
    );
    assert_eq!(
        sent(&fake),
        [
            (Method::Get, "/api/libraries".to_owned()),
            (Method::Get, ASKED_AUDIOBOOKS.to_owned()),
            (Method::Post, "/api/libraries".to_owned()),
        ]
    );
}

#[tokio::test]
async fn an_audiobookshelf_that_does_not_answer_is_unavailable() {
    assert!(matches!(
        audiobookshelf(&Fake::silent())
            .furnish(&filed(&[("audiobooks", AUDIOBOOKS)]))
            .await,
        Err(Failure::Unavailable { service }) if service == "audiobookshelf"
    ));
}

#[tokio::test]
async fn a_library_it_added_is_not_added_again_for_the_same_folder() {
    let fake = audiobookshelf_answering(vec![
        (
            Method::Get,
            "/api/libraries",
            vec![Answer::reply(200, NONE_HELD)],
        ),
        (
            Method::Get,
            "/api/filesystem",
            vec![Answer::reply(200, EXISTS)],
        ),
        (
            Method::Post,
            "/api/libraries",
            vec![Answer::reply(200, made("Audiobooks", AUDIOBOOKS))],
        ),
    ]);
    assert_eq!(
        audiobookshelf(&fake)
            .furnish(&filed(&[
                ("audiobooks", AUDIOBOOKS),
                ("audiobooks", AUDIOBOOKS)
            ]))
            .await
            .ok(),
        Some(held(&[("audiobooks", true), ("audiobooks", true)]))
    );
    assert_eq!(libraries_added(&fake).len(), 1);
}
