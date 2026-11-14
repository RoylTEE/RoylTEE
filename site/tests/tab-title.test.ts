import { describe, expect, it } from 'vitest';
import { AREAS, tabTitle } from '../src/data/ia';

describe('browser tab titles', () => {
  it('names the page, its area and the project, separated by " . "', () => {
    expect(tabTitle('Ledger', 'Shares', 'RoylTEE')).toBe('Ledger . Shares . RoylTEE');
    expect(tabTitle('Sealed-bid auction', 'Auction', 'RoylTEE')).toBe('Sealed-bid auction . Auction . RoylTEE');
  });
  it('does not repeat the area when the page carries the same name', () => {
    expect(tabTitle('Activity', 'Activity', 'RoylTEE')).toBe('Activity . RoylTEE');
    expect(tabTitle('your shares', 'Your shares', 'RoylTEE')).toBe('your shares . RoylTEE');
    for (const a of AREAS) expect(tabTitle(a.label, a.label, 'RoylTEE')).toBe(`${a.label} . RoylTEE`);
  });
});
