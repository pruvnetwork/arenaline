# TxLINE API — feedback

Our experience integrating TxLINE into arenaline (and, on the same engine family, two sibling
products), building against the World Cup devnet event.

## What we liked most
- **One normalized schema across everything.** The same JSON shape holds for every competition and
  fixture, so scaling from one match to all 104 was zero extra parsing. The odds snapshot's
  `SuperOddsType` + `PriceNames` + `Pct[]` layout made "give me the home-win probability" a one-liner.
- **Demarginated odds out of the box.** `Bookmaker: TXLineStablePriceDemargined` with a ready `Pct[]`
  meant we didn't have to strip the overround ourselves — the number we feed the agents is already a
  clean implied probability. That is exactly the primitive a trading tool wants.
- **On-chain anchoring + `stat-validation-v3`.** The fact that updates are cryptographically anchored
  on Solana, with a Merkle multiproof endpoint to verify a stat on-chain, is what makes an
  *end-to-end* trustless product possible — it closes the "is the input real?" gap that usually kills
  optimistic/verifiable-compute designs. This was the single most valuable feature for us.
- **Fee-waived event tier.** The free World Cup subscription made it painless to stand up a fresh,
  separate identity per product (guest → on-chain subscribe → activate) without cost friction.

## Where we hit friction
- **Wire formats were largely undocumented.** We reverse-engineered the exact subscribe + activation
  formats from the txoracle IDL and two community SDKs. A short "here are the request/response shapes"
  page (especially for `token/activate` and `stat-validation-v3`) would have saved hours.
- **`POST /api/token/activate` returns a plain-text token, not JSON.** Our first client did
  `res.json()` and got `Unexpected token 'x'…`. A `Content-Type` header or a documented note would
  prevent this exact trip-up.
- **PascalCase vs camelCase inconsistency.** Odds/fixtures come back PascalCase (`FixtureId`, `Pct`),
  scores come back camelCase. Consistent casing (or a documented reason) would remove a small but
  real source of bugs.
- **Odds snapshot is empty (`[]`) for finished matches.** Understandable, but it means you can't pull
  a historical odds *timeline* for a completed fixture to replay against — only live matches produce a
  moving series. An "odds history for fixture X" endpoint would be gold for backtesting/replay tools.
- **The guest → subscribe → activate handshake is a few coordinated steps** (JWT, an on-chain tx, then
  a signed activation). It works well once wired, but a single reference snippet in the docs would
  lower the barrier for new teams a lot.

Net: the data itself is excellent — clean, fast, demarginated, and (crucially) on-chain-verifiable.
Most of the friction was documentation, not the API. We'd happily build on it again.
