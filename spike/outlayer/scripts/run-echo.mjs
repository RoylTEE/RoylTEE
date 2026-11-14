// Runs out/echo_wasi.wasm under Node's WASI Preview 1 with stdin piped in.
// Usage: printf '{"hello":"tee"}' | node scripts/run-echo.mjs
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";
const wasi = new WASI({ version: "preview1", args: [], env: {}, returnOnExit: true });
const mod = await WebAssembly.compile(await readFile(new URL("../out/echo_wasi.wasm", import.meta.url)));
const imports = WebAssembly.Module.imports(mod).map((i) => `${i.module}.${i.name}`);
const exports = WebAssembly.Module.exports(mod).map((e) => e.name);
console.error(`imports: ${imports.join(", ")}`);
console.error(`exports: ${exports.join(", ")}`);
const inst = await WebAssembly.instantiate(mod, wasi.getImportObject());
const code = wasi.start(inst);
console.error(`exit: ${code}`);
