import { createHash } from 'node:crypto';

// Blocked terms are kept as salted sha256 digests of normalized text (lowercase letters and digits only),
// so this file holds no readable list. LENGTHS are the sizes checked as substrings of a single token.
const SALT = 'royltee blocked terms v1:';
const LENGTHS = [5, 7, 9, 10, 11, 12, 13, 14];
// Digests that must not appear inside any token, and not as a whole token or 2 to 3 adjacent tokens joined.
const INSIDE = [
  '4ce415b037e7fe81bbf82f5faddf020fa9d93a2dd86103ffd9bc71dc159d549f',
  '77e6a5505adc7d7a525394b1690d3583238e54646ab5722900b551b641500083',
  '245d85f578bfef99d4323eece8a7fa2630fc773293c095f8396ddad650ec0b74',
  '2d83116b8a83316ee345ed5dcbb743242c8d9433c5662b57a2c5293afb7c2a5b',
  'e10f5bcfe2313a7f9b31a3f284b63abdfcee2d34ee0d5293f550785a451b3591',
  '1a1bdd0947851a99c66dfcb211ccb2f5e5724f35fd0f2c9db8c21b33754fc482',
  '72dbfe4876897d3e73801d102efc93fbc9d0fff2da56c9be0e6926e80575d8ed',
  '6875ccc996e72a0e88266323eecb584b88ce2885f8a7474380c945f845a4cf3d',
  'a169bfc09e0f75068647485101a8cad43f7d4ad95d0556b0ab5ef451ad8ad1d0',
  '884d28f6850a4da02c842e39bc2cfdd76f6ef4cc3fd4728573805d6c6fb4d884',
  '815e5340a4fa4c1af78f6f0d7c0c79d8383ee3ad3d7b5d381032405ebc1ab40a',
  '36e8ed44b89b4295694f9c294cb35b3d5085552a7cd736ef09ee0b7604e95d80',
  '271f7b7aa03b7bb7bbc7b0cbce4351160a37e741c04c6e84dce6fff2876fdbb0',
];
// Digests that only count as a whole token or 2 to 3 adjacent tokens joined.
const WHOLE_ONLY = [
  'afe1ebd1d47e0ad65a4ce5537be7ad5e51118bfb84245556ee192e0b4e60c670',
  'dc57e09e7fd8577ace6e0cfc4cef1885904f32caad67ce9b1ec8a8f09b29a7c6',
  'c40e37dcb711df09c7f4fa2cb1edcd1f823d09b2f4b61b022d88b19a63e79e9e',
  'ee45bad7519d5d111d2ab692cd3297d0b3932d3efcd83c6927fc9de041a4c929',
  'd4acdcc328c3ae7934d7883fe8f88f9a697cefd9abd13fe04f16b74059c3de0e',
];

export function createBlockedCheck({ salt = SALT, lengths = LENGTHS, inside = INSIDE, wholeOnly = WHOLE_ONLY } = {}) {
  const hash = (s) => createHash('sha256').update(salt + s).digest('hex');
  const insideSet = new Set(inside);
  const wholeSet = new Set([...inside, ...wholeOnly]);
  const cache = new Map();
  const check = (s, kind) => {
    const key = kind + s;
    let hit = cache.get(key);
    if (hit === undefined) { hit = (kind === 'i' ? insideSet : wholeSet).has(hash(s)); cache.set(key, hit); }
    return hit;
  };
  const minLen = Math.min(...lengths);
  return function blocked(text) {
    const tokens = text.toLowerCase().match(/[a-z0-9]+/g) ?? [];
    const found = [];
    for (let i = 0; i < tokens.length; i++) {
      let joined = '';
      for (let n = 0; n < 3 && i + n < tokens.length; n++) {
        joined += tokens[i + n];
        if (check(joined, 'w')) found.push(`blocked term: ${tokens.slice(i, i + n + 1).join(' ')}`);
      }
      const t = tokens[i];
      if (t.length < minLen) continue;
      for (const len of lengths) {
        if (t.length < len) continue;
        for (let s = 0; s + len <= t.length; s++) if (check(t.slice(s, s + len), 'i')) found.push(`blocked term: ${t}`);
      }
    }
    return [...new Set(found)];
  };
}

const blocked = createBlockedCheck();

const patterns = [
  [/[\u2013\u2014]/g, 'long dash'],
  [/utc\s*\+\s*7/gi, 'identifier'], [/\/w[o]rkspace\//g, 'local path'], [/\/h[o]me\/box/g, 'local path'],
];
export function forbidden(text) {
  return [
    ...patterns.flatMap(([re, kind]) => [...text.matchAll(re)].map(m => `${kind}: ${m[0]}`)),
    ...blocked(text),
  ];
}
