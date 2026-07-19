# arenaline — Superteam submission (paste-ready)

**Track:** Trading Tools and Agents (TxODDS World Cup) · **Prize:** 16k USDT
Fields below map to the form. ⟨…⟩ = fill after recording.

---

## Project title
**arenaline — the Agent vs Agent Arena where the chain says who won**

## One-liner
Two autonomous agents run opposite strategies on the same live TxLINE odds feed; the winner is
replayed on-chain, so the P&L race is provable, not reported.

## Short description
arenaline is an Agent vs Agent Arena for live World Cup football. A momentum agent and a
mean-reversion agent read the same live TxLINE 1X2 odds — the home-win probability is the price both
trade — and mark to market on every update. Whoever holds more equity at the settled price wins.

The point isn't just the race, it's the **trust**: the whole match is a
[tickpruv](https://github.com/pruvnetwork/tickpruv) input log, and every agent decision is a
deterministic tick that compiles to SBF. If anyone disputes the result, the tick is **replayed by
Solana itself** — so no server decides who won, and no operator can fake a track record. Neither the
data (TxLINE is on-chain-anchored) nor the P&L (SBF replay) can be forged. It's built on the tickpruv
verifiable engine; arenaline adds the trading game, not a new proof system.

Deterministic scope, stated plainly: the agents are small rule-based strategies, not LLMs — exactly
the class that can be replayed on-chain. arenaline proves *strategy execution*.

## Demo video
⟨Loom/YouTube link — see docs/DEMO-VIDEO.md for the script⟩

## Public repo
https://github.com/pruvnetwork/arenaline

## Application access (deployed + devnet)
- Live dashboard: **https://arenaline.vercel.app** (animated P&L race + on-chain proof panel)
- Devnet program (duel): `AWQDizXJLqXUUBHkvmUBcowCXQawZCQ2L6jNcTexMdk5`
  — https://explorer.solana.com/address/AWQDizXJLqXUUBHkvmUBcowCXQawZCQ2L6jNcTexMdk5?cluster=devnet
- Reproduce the trustless settlement: `scripts/onchain-verify.mjs <match.tplog>` replays a match's
  input log through the deployed program; its verdict matches the runner bit-for-bit
  (A 98.289 vs B 101.711 → Agent B, native and on-chain).

## Technical documentation
Full write-up: `docs/TECHNICAL.md`. Summary: `runner` ingests live TxLINE 1X2 odds → `games/duel`
(deterministic fixed-point `TickLogic`, 2 agents) → `programs/duel-program` (SBF, deployed devnet) is
the replay target the tickpruv `referee`/`wager` (reused unchanged) CPI into on a dispute. One-step
proof ~19k CU vs ~280k CU for a zkVM verify.

### TxLINE endpoints used
- `POST /auth/guest/start` — guest JWT (arenaline's own identity)
- `POST /api/token/activate` (+ on-chain `subscribe` to txoracle `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`) — API token
- **`GET /api/odds/snapshot/{fixtureId}`** — the live price input (1X2 `Pct[0]`, home-win %) fed to the agents each tick
- `GET /api/scores/snapshot/{fixtureId}` — fixture state / settlement price
- `GET /api/scores/stat-validation-v3` — roadmap: bind each tick's input to an on-chain Merkle proof

## API feedback
Full note: `docs/TXLINE-FEEDBACK.md`. In short — loved the single normalized schema, the demarginated
`Pct[]` (an implied probability with the overround already stripped, exactly what a trading tool
wants), and the on-chain anchoring + `stat-validation-v3` proof, which is what makes an end-to-end
trustless product possible. Friction was mostly docs: undocumented wire formats, `token/activate`
returning plain text (not JSON), PascalCase↔camelCase between odds and scores, and no odds *history*
endpoint for finished fixtures.

## Eligibility
Individual/team (max 3). A running agent + tool on devnet that ingests TxLINE as a live input and
executes a defined strategy. Autonomous (the runner drives both agents unattended), deterministic, and
settled trustlessly on Solana. Free/educational; no wagering required to demonstrate.
