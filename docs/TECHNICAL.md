# arenaline — technical documentation

## Core idea
Two autonomous trading agents read the **same live TxLINE odds feed** and run opposite
deterministic strategies. Positions mark to market on every update; whoever holds more equity
at the settled price wins. The whole match is a [tickpruv](https://github.com/pruvnetwork/tickpruv)
game, so **the winner is replayed on-chain** — the P&L race is provable, not reported by a server.

The problem it solves: you cannot trust an autonomous agent's *claimed* track record. "My bot ran
this strategy, here's its P&L" is the operator's word. arenaline removes that trust: the strategy is
a deterministic tick that runs off-chain at full speed, and any disputed step is re-executed by
Solana itself. Neither the **data** (TxLINE is on-chain-anchored) nor the **result** (SBF replay)
can be faked.

> Honest scope: the agents are small **deterministic rule-based strategies**, not LLMs — exactly the
> class that can be replayed on-chain. arenaline proves *strategy execution*, not general "AI."

## Architecture

```
TxLINE live 1X2 odds ──▶ runner ──▶ duel game (TickLogic) ──▶ input log (TPL1) + P&L trace
   home-win prob = price     │            deterministic tick            │
                             │                                          ▼
                             │                             duel-program (SBF, on devnet)
                             ▼                                          │  replay a disputed tick
                        dashboard  ◀── trace.json                       ▼
                    arenaline.vercel.app              tickpruv referee + wager (reused, unchanged)
                                                        bisection · native replay · payout
```

| Component | What it is |
|---|---|
| `games/duel` | The strategy. A `TickLogic` over a 48-byte, 2-agent state (cash + position each). Input = one TxLINE price per tick; verdict = higher mark-to-market equity. `no_std`, fixed-point (Q32.32), all saturating — so it compiles to SBF and replays bit-identically. |
| `programs/duel-program` | Thin on-chain wrapper: `Tick` / `LoadState` / `Verdict`. Deployed to devnet at `AWQDizXJLqXUUBHkvmUBcowCXQawZCQ2L6jNcTexMdk5`. The replay target the referee CPIs into. |
| `runner` | The autonomous driver. Polls the live TxLINE 1X2 odds, turns the home-win probability into a price, ticks both agents, prints the P&L race, and writes a tickpruv input log (`TPL1`) so the match replays exactly. Live and `REPLAY=<file>` modes. |
| `web` | The dashboard (arenaline.vercel.app): animates a match trace and shows the on-chain proof. |
| tickpruv `wager` + `referee` + `runtime` + `merkle` | **Reused unchanged.** The referee stores the game program as a session parameter, so the deployed engine replays *this* game with no redeployment. |

## The two strategies
Both see the same price stream `p₀, p₁, …` (home-win implied probability, Q32.32). On each move:
- **Agent A — Momentum:** price rose → buy a unit; price fell → sell a unit.
- **Agent B — Mean-reversion:** the exact opposite — fade every move.

Position clamps to a max; a buy that can't be paid for is skipped (cash never goes negative); every
op is saturating so a hostile price cannot open a panic path that diverges on-chain from off-chain.
Equity = cash + position × price; higher equity at the final settled price wins.

## Determinism → provable settlement
Because the tick is pure and integer-only, the off-chain runner and the on-chain program produce the
**same bytes** from the same input log. `scripts/onchain-verify.mjs` demonstrates it: it replays a
match's input log through the deployed program tick by tick and asks the program's own `Verdict` — it
matches the runner's native result exactly:

```
native (runner):    A 90.412  vs  B 109.588  →  Agent B
on-chain (program): A 90.412  vs  B 109.588  →  Agent B
```

That is precisely the one step the tickpruv referee runs to resolve a disputed tick. A full staked
match + adversarial dispute reuse tickpruv's `devnet-match` / `devnet-dispute` against the deployed
`duel-program`.

## Business / technical highlights
- **Trust-minimized track record.** A prop desk or signal provider can *prove* an agent's performance
  to LPs — the record is on-chain and any tick is replayable. Nobody can fake a backtest or a duel.
- **Cheap.** The one-step proof lands at ~19k CU (tickpruv), vs ~280k CU for a zkVM verify alone —
  the happy path is nearly free; the chain only works on a dispute.
- **Composable.** Any deterministic strategy is a drop-in `TickLogic`; a second agent is just another
  strategy function. The Agent-vs-Agent Arena generalizes to a tournament.

## TxLINE endpoints used
| Endpoint | Role in arenaline |
|---|---|
| `POST /auth/guest/start` | Guest JWT for arenaline's own TxLINE identity |
| `POST /api/token/activate` (+ on-chain `subscribe` to txoracle `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`) | Activate the API token (free World Cup tier) |
| **`GET /api/odds/snapshot/{fixtureId}`** | **The live price input** — the runner reads `SuperOddsType: 1X2_PARTICIPANT_RESULT`, `Pct[0]` (home-win %), every poll. This is the tick input. |
| `GET /api/scores/snapshot/{fixtureId}` | Fixture state / settlement price at full time |
| `GET /api/scores/stat-validation-v3` | *(roadmap)* bind each input tick to an on-chain Merkle proof so the data itself is unforgeable end-to-end |

Verified live: the runner pulled the real World Cup final home-win price (**31.2%**) and wrote a valid
input log.

## Repo layout
```
games/duel/            strategy game (TickLogic) + tests
programs/duel-program/ on-chain wrapper (deployed devnet)
runner/                autonomous live-TxLINE driver + replay
scripts/onchain-verify.mjs   native == on-chain proof
web/                   dashboard (arenaline.vercel.app)
deploy/arenaline.json  program id + reused tickpruv ids
```

Built on [tickpruv](https://github.com/pruvnetwork/tickpruv) · [PRUV Network](https://github.com/pruvnetwork).
