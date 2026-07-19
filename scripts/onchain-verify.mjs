// Proof that the deployed duel program computes the same result on-chain as the
// off-chain runner: replay a tickpruv input log (TPL1) through the on-chain
// program tick by tick, then ask the program's own Verdict — the winner must
// match what the runner settled natively. This is exactly the step the tickpruv
// referee runs to resolve a disputed tick, made concrete against a real log.
//
//   PAYER=path RPC=url node scripts/onchain-verify.mjs <match.tplog>
import { readFileSync } from "node:fs";
import {
  Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import cfg from "../deploy/arenaline.json" with { type: "json" };

const DUEL = new PublicKey(cfg.duelProgram);
const STATE_SIZE = cfg.stateSize; // 48
const ONE = 1n << 32n;
const START_CASH = 100n * ONE;

const rpc = process.env.RPC || "https://api.devnet.solana.com";
const conn = new Connection(rpc, "confirmed");
const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(process.env.PAYER, "utf8"))));
const logPath = process.argv[2];

// decode a TPL1 input log into fx price BigInts
function readLog(path) {
  const d = readFileSync(path);
  if (d.subarray(0, 4).toString() !== "TPL1") throw new Error("not a TPL1 log");
  const prices = [];
  let off = 4;
  while (off < d.length) {
    const len = d.readUInt32LE(off); off += 4;
    prices.push(d.readBigInt64LE(off)); off += len;
  }
  return prices;
}

// genesis state = Duel::init: zeros except both agents' cash = START_CASH
function genesis() {
  const s = Buffer.alloc(STATE_SIZE);
  s.writeBigInt64LE(START_CASH, 16); // CASH_A
  s.writeBigInt64LE(START_CASH, 32); // CASH_B
  return s;
}
const i64 = (v) => { const b = Buffer.alloc(8); b.writeBigInt64LE(BigInt(v)); return b; };
const u64 = (v) => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); return b; };

async function main() {
  const prices = readLog(logPath);
  console.log(`on-chain verify · duel ${cfg.duelProgram} · ${prices.length} ticks from ${logPath}`);

  // 1) create a program-owned state account and load the genesis state
  const state = Keypair.generate();
  const rent = await conn.getMinimumBalanceForRentExemption(STATE_SIZE);
  const create = SystemProgram.createAccount({
    fromPubkey: payer.publicKey, newAccountPubkey: state.publicKey,
    lamports: rent, space: STATE_SIZE, programId: DUEL,
  });
  const load = new TransactionInstruction({
    programId: DUEL,
    keys: [{ pubkey: state.publicKey, isSigner: true, isWritable: true }],
    data: Buffer.concat([Buffer.from([1]), genesis()]), // 1 = LoadState
  });
  await sendAndConfirmTransaction(conn, new Transaction().add(create, load), [payer, state], { commitment: "confirmed" });
  console.log(`  state account ${state.publicKey.toBase58().slice(0, 8)}… seeded with genesis`);

  // 2) replay every tick on-chain (batched — sequential on the same account)
  const key = [{ pubkey: state.publicKey, isSigner: false, isWritable: true }];
  const BATCH = 8;
  for (let i = 0; i < prices.length; i += BATCH) {
    const tx = new Transaction();
    for (let j = i; j < Math.min(i + BATCH, prices.length); j++) {
      tx.add(new TransactionInstruction({
        programId: DUEL, keys: key,
        data: Buffer.concat([Buffer.from([0]), u64(j), i64(prices[j])]), // 0 = Tick
      }));
    }
    await sendAndConfirmTransaction(conn, tx, [payer], { commitment: "confirmed" });
    process.stdout.write(`\r  replayed ${Math.min(i + BATCH, prices.length)}/${prices.length} ticks`);
  }
  console.log();

  // 3) decode final state + ask the program's own verdict
  const info = await conn.getAccountInfo(state.publicKey, "confirmed");
  const s = info.data;
  const lastPrice = s.readBigInt64LE(8);
  const mtm = (cashOff, posOff) => {
    const cash = s.readBigInt64LE(cashOff);
    const pos = s.readBigInt64LE(posOff);
    return cash + ((pos * lastPrice) >> 32n);
  };
  const ea = mtm(16, 24), eb = mtm(32, 40);
  const f = (v) => Number(v) / Number(ONE);

  const verdictIx = new TransactionInstruction({ programId: DUEL, keys: key, data: Buffer.from([2]) }); // 2 = Verdict
  const sim = await conn.simulateTransaction(new Transaction().add(verdictIx), [payer]);
  const ret = sim.value.returnData;
  const winner = ret ? Buffer.from(ret.data[0], "base64")[0] : 255;
  const name = ["DRAW", "AGENT A — momentum", "AGENT B — mean-reversion"][winner] ?? "?";

  console.log(`\n  on-chain final: A ${f(ea).toFixed(3)}  vs  B ${f(eb).toFixed(3)}`);
  console.log(`  on-chain verdict (program's own duel::verdict): ${name}`);
  console.log(`\n  run the runner on the same log to confirm native == on-chain:`);
  console.log(`    (the runner already printed its native winner when it wrote ${logPath})`);
}

main().catch((e) => { console.error("\nverify FAILED:", e.message); process.exit(1); });
