use chessigma::{Client, EloOptions, Error, GameResult, RatingSystem, SideToMove};
#[test]
fn daily_puzzle_query_and_null_fields() {
    let mut s = mockito::Server::new();
    let m=s.mock("GET","/daily-puzzle").match_query(mockito::Matcher::UrlEncoded("date".into(),"2026-10-01".into())).with_status(200).with_body(r#"{"date":"2026-10-01","puzzleId":"p1","fen":"position","sideToMove":"black","rating":1500,"themes":["fork"],"solution":["e2e4"],"gameUrl":null}"#).create();
    let p = Client::with_base_url(s.url())
        .unwrap()
        .daily_puzzle(Some("2026-10-01"))
        .unwrap();
    assert_eq!(p.puzzle_id, "p1");
    assert_eq!(p.side_to_move, SideToMove::Black);
    assert!(p.game_url.is_none());
    m.assert();
}
#[test]
fn elo_options_and_submission() {
    let mut s = mockito::Server::new();
    let elo=s.mock("GET","/elo-calculator").match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded("yourRating".into(),"1500".into()),mockito::Matcher::UrlEncoded("opponentRating".into(),"1400".into()),mockito::Matcher::UrlEncoded("result".into(),"win".into()),mockito::Matcher::UrlEncoded("system".into(),"lichess".into()),mockito::Matcher::UrlEncoded("kFactor".into(),"40".into())])).with_status(200).with_body(r#"{"yourRating":1500,"opponentRating":1400,"result":"win","system":"lichess","kFactor":40,"expectedScore":0.64,"newRating":1514,"change":14}"#).create();
    let game=s.mock("POST","/game").match_header("content-type","application/json").match_body(mockito::Matcher::JsonString(r#"{"pgn":"example pgn"}"#.into())).with_status(201).with_body(r#"{"gameId":"abc","url":"https://www.chessigma.com/games/abc","status":"submitted","message":"ok"}"#).create();
    let c = Client::with_base_url(s.url()).unwrap();
    assert_eq!(
        c.calculate_elo(
            1500,
            1400,
            GameResult::Win,
            EloOptions {
                system: Some(RatingSystem::Lichess),
                k_factor: Some(40)
            }
        )
        .unwrap()
        .change,
        14
    );
    assert_eq!(c.submit_game("example pgn").unwrap().game_id, "abc");
    elo.assert();
    game.assert();
}
#[test]
fn analysis_retains_extra_statistics() {
    let mut s = mockito::Server::new();
    let m=s.mock("GET","/game/abc").with_status(200).with_body(r#"{"gameId":"abc","status":"completed","analysis":{"game":{"id":"abc","pgn":"pgn","source":"public-api","eco":"C20"},"accuracy":{"white":95}},"metadata":{"analysisCompletedAt":"2026-10-03T00:00:00Z","version":"1.0"}}"#).create();
    let g = Client::with_base_url(s.url())
        .unwrap()
        .get_game("abc")
        .unwrap();
    assert!(g.analysis.game.white_rating.is_none());
    assert_eq!(g.analysis.extra["accuracy"]["white"], 95);
    assert_eq!(g.analysis.game.extra["eco"], "C20");
    m.assert();
}
#[test]
fn rate_limit_preserves_status_and_retry_after() {
    let mut s = mockito::Server::new();
    let m = s
        .mock("GET", "/daily-puzzle")
        .with_status(429)
        .with_header("Retry-After", "60")
        .with_body(r#"{"error":"Slow down","details":{"limit":30}}"#)
        .create();
    match Client::with_base_url(s.url())
        .unwrap()
        .daily_puzzle(None)
        .unwrap_err()
    {
        Error::Api {
            status,
            message,
            details,
            retry_after,
        } => {
            assert_eq!(status, 429);
            assert_eq!(message, "Slow down");
            assert_eq!(details.unwrap()["limit"], 30);
            assert_eq!(retry_after.as_deref(), Some("60"));
        }
        e => panic!("wrong error: {e}"),
    };
    m.assert();
}
#[test]
fn non_json_error_and_malformed_success() {
    for status in [503, 200] {
        let mut s = mockito::Server::new();
        let m = s
            .mock("GET", "/daily-puzzle")
            .with_status(status)
            .with_body("not json")
            .create();
        let e = Client::with_base_url(s.url())
            .unwrap()
            .daily_puzzle(None)
            .unwrap_err();
        if status == 503 {
            assert!(matches!(e, Error::Api { status: 503, .. }));
        } else {
            assert!(matches!(e, Error::Decode(_)));
        }
        m.assert();
    }
}
