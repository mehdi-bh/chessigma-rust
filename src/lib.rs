//! Typed client for the [Chessigma public chess API](https://www.chessigma.com/developers).
//!
//! No API key is required. The service has a global limit of 30 requests/minute.
//! Requests are not automatically retried. Game submission stores PGN; engine
//! analysis only runs when someone opens the returned URL in a browser.
//!
//! ```no_run
//! let client = chessigma::Client::new()?;
//! let puzzle = client.daily_puzzle(None)?;
//! println!("{}: {}", puzzle.date, puzzle.fen);
//! # Ok::<(), chessigma::Error>(())
//! ```

use reqwest::blocking::{Client as HttpClient, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt, time::Duration};

/// Production API root.
pub const DEFAULT_BASE_URL: &str = "https://www.chessigma.com/public-api/v1";

/// HTTP/JSON failures and API errors, retaining status and rate-limit metadata.
#[derive(Debug)]
pub enum Error {
    Transport(reqwest::Error),
    Decode(serde_json::Error),
    Api {
        status: u16,
        message: String,
        details: Option<serde_json::Value>,
        retry_after: Option<String>,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "chessigma transport: {e}"),
            Self::Decode(e) => write!(f, "chessigma decode: {e}"),
            Self::Api {
                status, message, ..
            } => write!(f, "chessigma HTTP {status}: {message}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            Self::Decode(e) => Some(e),
            Self::Api { .. } => None,
        }
    }
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Transport(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Decode(e)
    }
}

/// Synchronous client with a default 30-second timeout. Use from a blocking context.
#[derive(Clone, Debug)]
pub struct Client {
    base_url: String,
    http: HttpClient,
}
impl Client {
    /// Connect to the public production API.
    pub fn new() -> Result<Self, Error> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }
    /// Override the API root (useful for local servers and tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, Error> {
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("chessigma-rust/0.1.0")
            .build()?;
        Ok(Self::with_http_client(base_url, http))
    }
    /// Configure your own reqwest client, including timeout or proxy settings.
    pub fn with_http_client(base_url: impl Into<String>, http: HttpClient) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http,
        }
    }
    fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, Error> {
        let response = request.header("Accept", "application/json").send()?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes()?;
        if !status.is_success() {
            let value = serde_json::from_slice::<serde_json::Value>(&body).ok();
            let message = value
                .as_ref()
                .and_then(|v| v.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or(status.canonical_reason().unwrap_or("API error"))
                .to_owned();
            let details = value.and_then(|v| v.get("details").cloned());
            return Err(Error::Api {
                status: status.as_u16(),
                message,
                details,
                retry_after,
            });
        }
        Ok(serde_json::from_slice(&body)?)
    }
    /// Store PGN with White, Black and Result tags; does not run analysis itself.
    pub fn submit_game(&self, pgn: &str) -> Result<GameSubmissionResponse, Error> {
        self.send(
            self.http
                .post(format!("{}/game", self.base_url))
                .json(&serde_json::json!({"pgn":pgn})),
        )
    }
    /// Read a public-API-submitted game after browser analysis is complete.
    /// A 404 can also mean that analysis has not been triggered yet.
    pub fn get_game(&self, game_id: &str) -> Result<GameAnalysisResponse, Error> {
        let mut url =
            reqwest::Url::parse(&format!("{}/game/", self.base_url)).map_err(|_| Error::Api {
                status: 0,
                message: "Invalid base URL".into(),
                details: None,
                retry_after: None,
            })?;
        url.path_segments_mut()
            .map_err(|_| Error::Api {
                status: 0,
                message: "Base URL cannot be a base".into(),
                details: None,
                retry_after: None,
            })?
            .pop_if_empty()
            .push(game_id);
        self.send(self.http.get(url))
    }
    /// Compute Elo rating change. The default system and K-factor are server-side.
    pub fn calculate_elo(
        &self,
        your_rating: u16,
        opponent_rating: u16,
        result: GameResult,
        options: EloOptions,
    ) -> Result<EloResponse, Error> {
        let mut q = vec![
            ("yourRating", your_rating.to_string()),
            ("opponentRating", opponent_rating.to_string()),
            ("result", result.as_str().into()),
        ];
        if let Some(s) = options.system {
            q.push(("system", s.as_str().into()));
        }
        if let Some(k) = options.k_factor {
            q.push(("kFactor", k.to_string()));
        }
        self.send(
            self.http
                .get(format!("{}/elo-calculator", self.base_url))
                .query(&q),
        )
    }
    /// Fetch today's puzzle or a past YYYY-MM-DD date. Future dates are rejected.
    /// Solution moves use UCI notation; the first move sets up the puzzle.
    pub fn daily_puzzle(&self, date: Option<&str>) -> Result<DailyPuzzle, Error> {
        let request = self.http.get(format!("{}/daily-puzzle", self.base_url));
        self.send(match date {
            Some(date) => request.query(&[("date", date)]),
            None => request,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameResult {
    Win,
    Draw,
    Loss,
}
impl GameResult {
    fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Draw => "draw",
            Self::Loss => "loss",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RatingSystem {
    Fide,
    Chesscom,
    Lichess,
}
impl RatingSystem {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fide => "fide",
            Self::Chesscom => "chesscom",
            Self::Lichess => "lichess",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SideToMove {
    White,
    Black,
}
/// Optional rating system and K-factor override.
#[derive(Debug, Clone, Copy, Default)]
pub struct EloOptions {
    pub system: Option<RatingSystem>,
    pub k_factor: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSubmissionResponse {
    pub game_id: String,
    pub url: String,
    pub status: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAnalysisResponse {
    pub game_id: String,
    pub status: String,
    pub analysis: Analysis,
    pub metadata: AnalysisMetadata,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub game: Game,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisMetadata {
    pub analysis_completed_at: String,
    pub version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub pgn: String,
    pub white_username: Option<String>,
    pub black_username: Option<String>,
    pub white_rating: Option<i32>,
    pub black_rating: Option<i32>,
    pub white_result: Option<String>,
    pub black_result: Option<String>,
    pub time_class: Option<String>,
    pub time_control: Option<String>,
    pub source: String,
    /// Retain additional analysis fields when the API returns them.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EloResponse {
    pub your_rating: u16,
    pub opponent_rating: u16,
    pub result: GameResult,
    pub system: RatingSystem,
    pub k_factor: u8,
    pub expected_score: f64,
    pub new_rating: i32,
    pub change: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPuzzle {
    pub date: String,
    pub puzzle_id: String,
    pub fen: String,
    pub side_to_move: SideToMove,
    pub rating: i32,
    pub themes: Vec<String>,
    pub solution: Vec<String>,
    pub game_url: Option<String>,
}
