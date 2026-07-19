//! arenaline runner — the autonomous driver.
//!
//! Pulls live TxLINE 1X2 odds for a fixture, turns the home-win implied
//! probability into a price, and ticks both agents (momentum vs mean-reversion)
//! on every move. It prints the live P&L race and appends each price to a
//! tickpruv input log, so the whole match stays replayable: genesis state plus
//! this log reproduces the exact final state the referee settles on.
//!
//! Two modes:
//!   live    (default) — poll the odds endpoint every POLL_SECS
//!   replay  REPLAY=path — read one decimal probability per line (offline demo)
//!
//! Env: TXLINE_HOST TXLINE_GUEST_JWT TXLINE_API_TOKEN FIXTURE_ID
//!      [POLL_SECS=6] [MAX_TICKS=0] [OUTCOME_INDEX=0] [LOG=match.tplog] [REPLAY=path]

use duel::{Duel, INPUT_ENTRY_SIZE, STATE_SIZE};
use std::io::{BufRead, Write};
use std::{env, fs, thread, time::Duration};
use tick_core::TickLogic;

const ONE: i64 = 1 << 32;

fn env_or(k: &str, d: &str) -> String {
    env::var(k).unwrap_or_else(|_| d.to_string())
}
fn fx_to_f64(v: i128) -> f64 {
    v as f64 / ONE as f64
}
fn prob_to_fx(p: f64) -> i64 {
    (p * ONE as f64) as i64
}

/// One 1X2 home-win probability from a live odds snapshot, as an Fx price.
fn fetch_price(host: &str, jwt: &str, api: &str, fixture: &str, idx: usize) -> Option<i64> {
    let url = format!("{host}/api/odds/snapshot/{fixture}");
    let body: serde_json::Value = ureq::get(&url)
        .set("Authorization", &format!("Bearer {jwt}"))
        .set("X-Api-Token", api)
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let rows = body.as_array()?;
    // Newest 1X2 row with a real percentage.
    let mut best: Option<(i64, i64)> = None; // (ts, price_fx)
    for r in rows {
        if r.get("SuperOddsType")?.as_str()? != "1X2_PARTICIPANT_RESULT" {
            continue;
        }
        let pct = r.get("Pct")?.as_array()?.get(idx)?.as_str()?;
        if pct == "NA" {
            continue;
        }
        let p = pct.parse::<f64>().ok()? / 100.0;
        let ts = r.get("Ts").and_then(|v| v.as_i64()).unwrap_or(0);
        if best.map_or(true, |(bts, _)| ts >= bts) {
            best = Some((ts, prob_to_fx(p)));
        }
    }
    best.map(|(_, px)| px)
}

fn scoreboard(tick: u64, price: i64, state: &[u8]) {
    let (ea, eb) = duel::equities(state);
    let (pa, pb) = duel::positions(state);
    let lead = if ea > eb {
        "A"
    } else if eb > ea {
        "B"
    } else {
        "="
    };
    println!(
        "tick {tick:>4} | price {:.4} | A(mom) eq {:>8.3} pos {:>+5.1} | B(rev) eq {:>8.3} pos {:>+5.1} | lead {lead}",
        fx_to_f64(price as i128),
        fx_to_f64(ea),
        fx_to_f64(pa as i128),
        fx_to_f64(eb),
        fx_to_f64(pb as i128),
    );
}

/// tickpruv input log: "TPL1" magic then per entry a u32 LE length prefix.
fn write_log(path: &str, prices: &[i64]) {
    let mut buf = Vec::from(*b"TPL1");
    for &p in prices {
        buf.extend_from_slice(&(INPUT_ENTRY_SIZE as u32).to_le_bytes());
        buf.extend_from_slice(&p.to_le_bytes());
    }
    if let Ok(mut f) = fs::File::create(path) {
        let _ = f.write_all(&buf);
    }
}

/// One tick: feed a price, advance both agents, print the board, record a trace
/// row (used by the dashboard).
fn advance(state: &mut [u8], prices: &mut Vec<i64>, trace: &mut Vec<String>, price: i64) {
    let mut entry = [0u8; INPUT_ENTRY_SIZE];
    entry.copy_from_slice(&price.to_le_bytes());
    let t = prices.len() as u64;
    Duel::tick(state, &entry, t).unwrap();
    prices.push(price);
    scoreboard(t, price, state);
    let (ea, eb) = duel::equities(state);
    let (pa, pb) = duel::positions(state);
    trace.push(format!(
        "{{\"t\":{t},\"price\":{:.6},\"ea\":{:.6},\"eb\":{:.6},\"pa\":{:.4},\"pb\":{:.4}}}",
        fx_to_f64(price as i128),
        fx_to_f64(ea),
        fx_to_f64(eb),
        fx_to_f64(pa as i128),
        fx_to_f64(pb as i128),
    ));
}

fn finish(state: &[u8], prices: &[i64], log_path: &str, trace: &[String]) {
    write_log(log_path, prices);
    if let Ok(path) = std::env::var("TRACE") {
        let json = format!("[{}]", trace.join(","));
        let _ = fs::write(&path, json);
        println!("trace -> {path}");
    }
    let (ea, eb) = duel::equities(state);
    let winner = match duel::verdict(state).unwrap_or(0) {
        1 => "AGENT A — momentum",
        2 => "AGENT B — mean-reversion",
        _ => "DRAW",
    };
    println!("\n{} ticks played, log -> {log_path}", prices.len());
    println!(
        "final: A {:.3}  vs  B {:.3}   =>   winner: {winner}",
        fx_to_f64(ea),
        fx_to_f64(eb)
    );
    println!("verify: replay `{log_path}` through the duel program; the referee settles the same winner on-chain.");
}

fn main() {
    let mut state = [0u8; STATE_SIZE];
    Duel::init(&mut state).unwrap();
    let mut prices: Vec<i64> = Vec::new();
    let log_path = env_or("LOG", "match.tplog");
    let mut trace: Vec<String> = Vec::new();

    if let Ok(path) = env::var("REPLAY") {
        // offline demo: one decimal probability per line
        println!("arenaline · replay {path} · momentum vs mean-reversion\n");
        let f = fs::File::open(&path).expect("replay file");
        for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
            let s = line.trim();
            if s.is_empty() {
                continue;
            }
            if let Ok(p) = s.parse::<f64>() {
                advance(&mut state, &mut prices, &mut trace, prob_to_fx(p));
            }
        }
        finish(&state, &prices, &log_path, &trace);
        return;
    }

    // live: poll the TxLINE odds feed
    let host = env_or("TXLINE_HOST", "https://txline-dev.txodds.com");
    let jwt = env::var("TXLINE_GUEST_JWT").expect("TXLINE_GUEST_JWT");
    let api = env::var("TXLINE_API_TOKEN").expect("TXLINE_API_TOKEN");
    let fixture = env::var("FIXTURE_ID").expect("FIXTURE_ID");
    let idx: usize = env_or("OUTCOME_INDEX", "0").parse().unwrap_or(0);
    let poll: u64 = env_or("POLL_SECS", "6").parse().unwrap_or(6);
    let max_ticks: usize = env_or("MAX_TICKS", "0").parse().unwrap_or(0);

    println!("arenaline · live · fixture {fixture} · home-win price · poll {poll}s\n");
    let mut last_fed: Option<i64> = None;
    loop {
        if let Some(price) = fetch_price(&host, &jwt, &api, &fixture, idx) {
            if last_fed != Some(price) {
                advance(&mut state, &mut prices, &mut trace, price);
                last_fed = Some(price);
            }
        } else {
            println!("(no 1X2 price yet — waiting)");
        }
        if max_ticks != 0 && prices.len() >= max_ticks {
            break;
        }
        thread::sleep(Duration::from_secs(poll));
    }
    finish(&state, &prices, &log_path, &trace);
}
