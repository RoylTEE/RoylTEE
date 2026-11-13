/**
 * Second reviewer 03d item 7: "Keyless" only when the account has no keys at all. Function call keys
 * are listed with receiver and methods, never hidden behind a full access count of zero.
 */
import type { AccessKey } from './chain';
export type KeyStatus = { text: string; tone: 'ok' | 'warn' | 'bad'; detail: string[] };
export function keyStatus(ks: AccessKey[]): KeyStatus {
  if (!ks.length) return { text: 'Keyless', tone: 'ok', detail: [] };
  const detail = ks.map((k) => k.access_key.permission === 'FullAccess'
    ? `${k.public_key}: full access`
    : `${k.public_key}: function call to ${k.access_key.permission.FunctionCall.receiver_id}, ${k.access_key.permission.FunctionCall.method_names.length ? k.access_key.permission.FunctionCall.method_names.join(', ') : 'any method'}`);
  const full = ks.filter((k) => k.access_key.permission === 'FullAccess').length;
  if (full) return { text: 'Deletion planned', tone: 'warn', detail };
  return { text: `${ks.length} function call key${ks.length === 1 ? '' : 's'}`, tone: 'warn', detail };
}
