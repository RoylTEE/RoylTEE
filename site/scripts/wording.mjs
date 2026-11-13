// Banned identifiers are stored as patterns so this file stays clean under its own scan.
const patterns = [
  [/[\u2013\u2014]/g, 'long dash'],
  [/bl[e]sdbera/gi, 'banned name'], [/sup[e]rscript/gi, 'banned name'], [/chr[o]nicle/gi, 'banned name'], [/\bja[a]a\b/gi, 'banned name'],
  [/sh[e]p\s*dev/gi, 'identifier'], [/game\s*of\s*spl?its/gi, 'identifier'], [/tr[u]e\s*nav/gi, 'identifier'], [/b[a]ngkok/gi, 'identifier'], [/utc\s*\+\s*7/gi, 'identifier'], [/\/w[o]rkspace\//g, 'local path'], [/\/h[o]me\/box/g, 'local path'], [/\bj[o]hn\b/gi, 'personal name'],
];
export function forbidden(text) { return patterns.flatMap(([re, kind]) => [...text.matchAll(re)].map(m => `${kind}: ${m[0]}`)); }
