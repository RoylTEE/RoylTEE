/* External audit finding F2: transaction outcome text. */
import { outcomeText } from '../src/scripts/tx-outcome';

describe('audit F2: a pending or unknown transaction is never reported as confirmed', () => {
  const ok = { SuccessValue: '' };
  it('treats NotStarted and Started as not final', () => {
    expect(outcomeText([{ status: 'NotStarted', receipts_outcome: [] }], 'Claim')).toMatch(/^Claim sent, but the chain reports it as not final \(NotStarted\), so success is not confirmed/);
    expect(outcomeText([{ status: 'Started', receipts_outcome: [] }], 'Claim')).toMatch(/not final \(Started\)/);
  });
  it('treats an unknown receipt status as not final, even when the transaction status is a success', () => {
    expect(outcomeText([{ status: ok, receipts_outcome: [{ outcome: { status: 'Unknown' } }] }], 'Transfer')).toMatch(/not final \(Unknown\)/);
    expect(outcomeText([{ status: ok, receipts_outcome: [{ outcome: {} }] }], 'Transfer')).toMatch(/not final \(no receipt status\)/);
  });
  it('treats a status object without a success field as not final', () => {
    expect(outcomeText([{ status: {} as never, receipts_outcome: [] }], 'Claim')).toMatch(/not confirmed/);
  });
  it('checks every outcome of a batch: one pending outcome makes the whole batch unconfirmed', () => {
    const done = { status: ok, receipts_outcome: [{ outcome: { status: { SuccessReceiptId: 'x' } } }] };
    expect(outcomeText([done, done], 'Registration')).toBe('Registration confirmed on chain.');
    expect(outcomeText([done, { status: 'Started', receipts_outcome: [] }], 'Registration')).toMatch(/not confirmed/);
  });
  it('still reports a failure before a pending receipt', () => {
    const fail = { Failure: { ActionError: { kind: { FunctionCallError: { ExecutionError: 'Smart contract panicked: only the current holder can claim' } } } } };
    expect(outcomeText([{ status: 'Started', receipts_outcome: [{ outcome: { status: fail } }] }], 'Claim')).toMatch(/Claim failed on chain \(1 of 1\): only the current holder can claim/);
  });
});
