# Chessigma Rust client

A typed client for the free [Chessigma public API](https://www.chessigma.com/developers).
Fetch the daily chess puzzle, calculate Elo changes, submit PGN games and retrieve completed analysis.

No API key or signup is needed. The API has a **global limit of 30 requests/minute** across callers. Clients make one request per method call and do not retry automatically. Respect `Retry-After` when present.

**Game submission does not run the chess engine.** Open the returned game URL in a browser and trigger analysis there. `get_game` / `GetGame` returns 404 while analysis is unavailable. Only public-API-submitted games are readable.

Puzzle solutions use UCI notation; the **first move sets up the puzzle**, and the following moves are the solution. A dated puzzle must be a past `YYYY-MM-DD`; future dates are rejected by the API.

## Install

```sh
cargo add chessigma
```

## Use

```rust
use chessigma::{Client, EloOptions, GameResult};

fn main() -> Result<(), chessigma::Error> {
    let client = Client::new()?;
    let puzzle = client.daily_puzzle(None)?;
    println!("{}: {}", puzzle.date, puzzle.fen);

    let elo = client.calculate_elo(1500, 1400, GameResult::Win, EloOptions::default())?;
    println!("{} ({:+})", elo.new_rating, elo.change);
    Ok(())
}
```

`submit_game(pgn)` returns `game_id` and `url`; `get_game(game_id)` retrieves completed analysis. `Analysis.extra` retains analysis statistics beyond the documented schema, and `Game.extra` retains additional game metadata.

This is a **synchronous** client with a default 30-second timeout. In an async app, use a blocking worker. `Client::with_http_client` accepts a configured `reqwest::blocking::Client` for custom timeouts and proxies; `Client::with_base_url` supports local servers.

```rust
match client.daily_puzzle(None) {
    Err(chessigma::Error::Api { status, message, retry_after, .. }) => {
        eprintln!("HTTP {status}: {message}, retry after {retry_after:?}");
    }
    result => { println!("{result:?}"); }
}
```

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo publish --dry-run
```

Protocol tests use local HTTP servers; tests and examples do not run live game submissions.

[Chessigma](https://www.chessigma.com) · [API documentation](https://www.chessigma.com/developers) · [OpenAPI schema](https://www.chessigma.com/openapi.json) · [Report a bug](https://github.com/mehdi-bh/chessigma-rust/issues)

The original client code is MIT licensed. Use of the hosted API is subject to [Chessigma's terms](https://www.chessigma.com/tos). The license does not cover the hosted service or third-party puzzle data.
