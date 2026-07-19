# arenaline

**Two autonomous trading agents duel on live World Cup odds. The chain decides who traded better — and neither the strategy execution nor the winner can be faked.**

arenaline is an *Agent vs Agent Arena* for the [TxODDS World Cup hackathon](https://superteam.fun/earn/listing/trading-tools-and-agents) (Trading Tools & Agents track). Two agents read the **same live TxLINE odds feed** and run opposite deterministic strategies — one chases momentum, one fades it (mean-reversion). Positions mark to market every TxLINE update; whoever holds more equity at the settled price wins.

**Live demo: [arenaline.vercel.app](https://arenaline.vercel.app)** · devnet program `AWQDizXJLqXUUBHkvmUBcowCXQawZCQ2L6jNcTexMdk5`

The twist: it runs on the **[tickpruv](https://github.com/pruvnetwork/tickpruv) verifiable engine.** Each agent's decision is a deterministic tick that compiles to SBF and runs off-chain at full speed. If anyone disputes the result, the tick is **replayed by Solana itself** — so the P&L race is provable, not reported. No trusted server decides who won.

> Scope, stated honestly: the "agents" are small **deterministic rule-based strategies**, not LLMs. That is exactly the class of agent that can be replayed on-chain. arenaline proves *strategy execution*, not general "AI."

## Why this fits the track

| Judging criterion | arenaline |
|---|---|
| **Data ingestion** | Both agents ingest live TxLINE odds; each update is a tick |
| **Autonomous operation** | The runner drives both strategies with no human input once started |
| **Logic & architecture** | Deterministic fixed-point strategies — determinism is *forced*, non-deterministic code cannot replay |
| **Innovation** | Trustless strategy settlement + TxLINE-anchored inputs — you can't fake the data *or* the P&L |
| **Production readiness** | Reuses tickpruv's benchmarked engine (~19k CU one-step proof vs ~280k CU for a zkVM) |

## How it's built

arenaline is a thin layer on the tickpruv engine — it adds a game, not a new proof system.

```
games/duel            the strategy: TickLogic over a 48-byte state (2 agents:
                      cash + position), input = one TxLINE price per tick,
                      verdict = higher mark-to-market equity. no_std, no floats.
programs/duel-program on-chain wrapper (tick / load-state / verdict) — the
                      replay target the tickpruv referee CPIs into on dispute.
runner/               the autonomous driver: pulls live TxLINE odds, ticks both
                      agents, writes the tickpruv input log, prints the P&L race.
```

Reused from tickpruv, unchanged: `wager` (stake escrow + payout), `referee`
(checkpoints, bisection, native replay), `runtime` (input log + checkpoints).
The referee stores the game program as a session parameter, so the deployed
engine replays *this* game with no redeployment.

## The two strategies

Both see the same price stream `p0, p1, ...` (implied probability of the outcome, Q32.32 fixed-point):

- **Agent A — Momentum:** price rose -> buy a unit; price fell -> sell a unit.
- **Agent B — Mean-reversion:** the exact opposite — fade every move.

Positions clamp to a max; a buy that can't be paid for is skipped; all math is
saturating so a hostile price can never open a panic path that diverges on-chain
from off-chain. Equity marks to the last settled price; higher equity wins.

## Status — working end to end

- `games/duel` — implemented and tested (determinism + strategy edge + cash-safety). ✅
- `runner/` — pulls the **real** live TxLINE home-win price (0.3124 for the World Cup
  final) and drives both agents; replay mode runs a full race where the lead flips as
  the trend reverses. ✅
- `programs/duel-program` — **deployed to devnet**: `AWQDizXJLqXUUBHkvmUBcowCXQawZCQ2L6jNcTexMdk5`. ✅
- **On-chain == native, proven:** `scripts/onchain-verify.mjs` replays a match's input log
  through the deployed program tick by tick; the program's own verdict matches the runner's
  native result exactly (A 99.250 vs B 100.750 → agent B, both). This is the step the
  tickpruv referee runs to settle a disputed tick — so the winner is provable, not reported. ✅

A full staked match + scripted dispute reuse tickpruv's `devnet-match` / `devnet-dispute`
against the deployed `duel-program` (the referee takes the game program as a parameter).

Built on [tickpruv](https://github.com/pruvnetwork/tickpruv) · [PRUV Network](https://github.com/pruvnetwork) — *don't trust, verify.*
