// Runs the auction WASI module under Node's WASI Preview 1, the way OutLayer
// runs it: request JSON on stdin, TEE secret in the environment, one JSON line
// on stdout. Used by the tests and by the local demo.
import { closeSync, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WASI } from "node:wasi";

// `caller` is what OutLayer puts in NEAR_PREDECESSOR_ID: the account that
// called outlayer.testnet. `undefined` leaves it unset, as an old worker would.
export async function runWasi(wasmPath, request, master, caller) {
  const dir = mkdtempSync(join(tmpdir(), "auction-wasi-"));
  const inPath = join(dir, "in.json");
  const outPath = join(dir, "out.txt");
  writeFileSync(inPath, typeof request === "string" ? request : JSON.stringify(request));
  writeFileSync(outPath, "");
  const stdin = openSync(inPath, "r");
  const stdout = openSync(outPath, "w");
  try {
    const env = master === undefined ? {} : { PROTECTED_AUCTION_MASTER: master };
    if (caller !== undefined) env.NEAR_PREDECESSOR_ID = caller;
    const wasi = new WASI({ version: "preview1", args: [], env, stdin, stdout, returnOnExit: true });
    const mod = await WebAssembly.compile(readFileSync(wasmPath));
    const inst = await WebAssembly.instantiate(mod, wasi.getImportObject());
    const code = wasi.start(inst);
    return { code, out: readFileSync(outPath, "utf8").trim() };
  } finally {
    closeSync(stdin);
    closeSync(stdout);
    rmSync(dir, { recursive: true, force: true });
  }
}
