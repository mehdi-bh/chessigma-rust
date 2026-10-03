fn main() -> Result<(), chessigma::Error> {
    let p = chessigma::Client::new()?.daily_puzzle(None)?;
    println!("{}: {}", p.date, p.fen);
    Ok(())
}
