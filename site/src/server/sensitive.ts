import { wordlist } from '@scure/bip39/wordlists/english.js';
const words = new Set(wordlist);
/** Conservative intake filter for demo support tickets. It reduces risk; it does not detect every secret. */
export function sensitive(value: string): string | null {
  if (/[A-Z0-9._%+-]+@[A-Z0-9-]+(?:\.[A-Z0-9-]+)*\.[A-Z]{2,}/i.test(value)) return 'an email address';
  if (/(?:ed25519|secp256k1):[1-9A-HJ-NP-Za-km-z]{60,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|private[_ -]?key\s*[:=]/i.test(value)) return 'a private key';
  let run = 0;
  for (const word of value.toLowerCase().split(/[^a-z]+/).filter(Boolean)) {
    run = words.has(word) ? run + 1 : 0;
    if (run >= 12) return 'a recovery phrase';
  }
  const scan = value.replace(/\b\d{4}-\d{2}-\d{2}(?:[ T]\d{2}:\d{2}(?::\d{2})?)?\b/g, ' ');
  for (const match of scan.matchAll(/\+?\d[\d ().-]{5,}\d/g)) {
    const digits = match[0].replace(/\D/g, '');
    // Cards first: no number format exception applies to a Luhn valid 13 to 19 digit run.
    if (digits.length >= 13 && digits.length <= 19 && luhn(digits)) return 'a card number';
    // A dotted amount such as 1.000.000 is skipped only with a currency next to it (review 03b, second reviewer item 1).
    if (/^\d{1,3}(?:\.\d{3})+(?:,\d+)?$/.test(match[0]) && monetary(scan, match.index ?? 0, match[0].length)) continue;
    const formatted = /^\+|[ ().-]/.test(match[0]);
    if (digits.length >= 7 && digits.length <= 15 && (formatted || /^0\d{8,10}$/.test(digits))) return 'a phone number';
  }
  return null;
}
const CURRENCY_BEFORE = /(?:[$€£]|\b(?:m?USDC|NEAR|USD|EUR|GBP))\s*$/i;
const CURRENCY_AFTER = /^\s*(?:[$€£]|(?:m?USDC|NEAR|USD|EUR|GBP|yoctoNEAR)\b)/i;
function monetary(text: string, at: number, length: number) {
  return CURRENCY_BEFORE.test(text.slice(Math.max(0, at - 8), at)) || CURRENCY_AFTER.test(text.slice(at + length, at + length + 12));
}
export function luhn(digits: string) {
  let sum = 0;
  [...digits].reverse().forEach((c, i) => { let n = Number(c); if (i % 2) { n *= 2; if (n > 9) n -= 9; } sum += n; });
  return sum % 10 === 0;
}
