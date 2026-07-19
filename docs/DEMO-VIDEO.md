# arenaline — demo video script (≤5 min)

The dashboard segments need **no credentials** — just a browser. The terminal segments prove the
backend; commands are below. Record at 1280×800+, narrate in one take.

---

## Scene 1 — the hook + the race  (0:00–0:50)  · browser
Open **https://arenaline.vercel.app**. Let the race play.
> "Two autonomous agents trade the same live World Cup odds. Agent A chases momentum; Agent B fades
> it. Watch the P&L race — A pulls ahead as the home team gains favouritism… and B wins on the late
> collapse. The lead flips. Now: how do you *trust* that result?"

Point at the tick counter / price (`home-win price`) and the two equity curves crossing.

## Scene 2 — provably settled  (0:50–1:25)  · browser
Scroll to the **Provably settled** panel.
> "The winner isn't reported by a server. The whole match is a tickpruv input log — genesis state plus
> the price log replays to the exact same state. Replayed through our on-chain program, its own verdict
> matches the runner bit-for-bit: A 98.289 vs B 101.711, Agent B — native and on-chain. One-step proof:
> ~19k compute units, versus ~280k for a zkVM."

Click the **duel program** link → Solana explorer (devnet) briefly.

## Scene 3 — real TxLINE data  (1:25–2:15)  · terminal
Run the live runner against the World Cup final (one tick, to show the real price):
```
TXLINE_GUEST_JWT=$JWT TXLINE_API_TOKEN=$API FIXTURE_ID=18257739 MAX_TICKS=1 \
  cargo run -q -p arenaline-runner
```
> "This is the actual TxLINE feed. The runner pulls the 1X2 home-win probability — here, 31 % for the
> final — and that number is the price both agents trade each tick. Live TxLINE in, deterministic
> decisions out."

## Scene 4 — the strategies race  (2:15–3:10)  · terminal
Replay a full match; the scoreboard scrolls:
```
REPLAY=match_path.txt LOG=match.tplog TRACE=web/trace.json \
  cargo run -q -p arenaline-runner
```
> "Momentum builds a long into the rise; mean-reversion shorts it. When the market collapses late,
> mean-reversion wins. Every decision is fixed-point, integer-only, saturating — no floats — which is
> exactly what lets it replay identically on-chain."

Let it print `winner: AGENT B — mean-reversion`.

## Scene 5 — the on-chain proof  (3:10–4:20)  · terminal
Replay the same log through the deployed program:
```
PAYER=$PAYER RPC=$RPC node scripts/onchain-verify.mjs match.tplog
```
> "Same input log, now executed on Solana tick by tick. The program's own verdict: Agent B — matching
> the native runner exactly, A 98.289 vs B 101.711. This is the precise step the tickpruv referee runs
> to settle a disputed tick. So no one can lie about who traded better."

## Scene 6 — close  (4:20–5:00)  · browser
Show the repo **github.com/pruvnetwork/arenaline**.
> "Autonomous agents, live TxLINE data, deterministic logic, and a winner settled trustlessly on-chain
> — built on the tickpruv verifiable engine. You can't fake the odds, and you can't fake the P&L.
> That's arenaline."

---

## Setup for the terminal scenes
Clone + build once, and point at arenaline's own credentials (devnet throwaway):
```
git clone https://github.com/pruvnetwork/arenaline && cd arenaline
cargo build -q -p arenaline-runner              # first build ~1 min

# arenaline's own TxLINE identity + devnet payer (provided separately, devnet-only):
export JWT=<arenaline TXLINE_GUEST_JWT>
export API=<arenaline TXLINE_API_TOKEN>
export PAYER=/path/to/arenaline-payer.json      # ~0.3 devnet SOL, funds the verify tx
export RPC=<a devnet RPC url>

# the illustrative price path used in the dashboard (or record your own live):
printf '0.35\n0.40\n0.45\n0.50\n0.55\n0.60\n0.55\n0.50\n0.46\n0.52\n0.58\n0.64\n0.70\n0.62\n0.54\n0.46\n0.38\n0.30\n0.25\n' > match_path.txt
```
`onchain-verify.mjs` needs `@solana/web3.js` — `npm i @solana/web3.js` in the repo, or run it from any
folder that has it.

> Tip: if you'd rather not run the terminal live, the dashboard (Scenes 1–2, 6) plus the committed
> `scripts/onchain-verify.mjs` output already tell the whole story. The video is judged on clearly
> showing the product; the deployed dashboard + explorer + repo do that on their own.
