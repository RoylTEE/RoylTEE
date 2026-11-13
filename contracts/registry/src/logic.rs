//! Pure rules with no chain access: bps split, code shapes, field checks.

use crate::types::{Bps, CodeType, FULL_BPS};

/// One share taking part in a split: (token number, bps).
pub type SplitPart = (u64, Bps);

/// Splits `amount` over `parts` by bps (decision 2).
///
/// Each credit is floor(amount * bps / 10000). The remainder, always smaller
/// than the number of parts, goes to the part with the largest bps; ties go
/// to the lowest token number. The credits always sum to `amount` exactly.
/// Returns None if the parts do not sum to exactly 10000 bps (decision 4),
/// if `parts` is empty, or on arithmetic overflow.
pub fn split(amount: u128, parts: &[SplitPart]) -> Option<Vec<u128>> {
    let total: u32 = parts.iter().map(|(_, b)| u32::from(*b)).sum();
    if parts.is_empty() || total != FULL_BPS {
        return None;
    }
    let mut credits = Vec::with_capacity(parts.len());
    let mut sum: u128 = 0;
    for (_, bps) in parts {
        let c = amount.checked_mul(u128::from(*bps))? / u128::from(FULL_BPS);
        sum += c;
        credits.push(c);
    }
    let remainder = amount - sum;
    let idx = remainder_index(parts);
    credits[idx] += remainder;
    Some(credits)
}

/// Splits a payment for a work + right whose registered shares may total
/// less than 10000 bps (decision 4). Each share gets floor(amount * bps /
/// 10000). If the shares total exactly 10000, the rounding remainder goes
/// to the largest share as in `split`. If they total less, everything not
/// credited (the unassigned fraction plus rounding) is returned as the
/// unallocated amount, to be held in escrow. Credits plus unallocated always
/// equal `amount`. Returns None above 10000 bps, for no parts, or on overflow.
pub fn split_partial(amount: u128, parts: &[SplitPart]) -> Option<(Vec<u128>, u128)> {
    let total: u32 = parts.iter().map(|(_, b)| u32::from(*b)).sum();
    if parts.is_empty() || total > FULL_BPS {
        return None;
    }
    if total == FULL_BPS {
        return split(amount, parts).map(|c| (c, 0));
    }
    let mut credits = Vec::with_capacity(parts.len());
    let mut sum: u128 = 0;
    for (_, bps) in parts {
        let c = amount.checked_mul(u128::from(*bps))? / u128::from(FULL_BPS);
        sum += c;
        credits.push(c);
    }
    Some((credits, amount - sum))
}

/// Index of the part that takes the remainder: largest bps, then lowest number.
pub fn remainder_index(parts: &[SplitPart]) -> usize {
    let mut best = 0;
    for (i, (num, bps)) in parts.iter().enumerate() {
        let (bnum, bbps) = parts[best];
        if *bps > bbps || (*bps == bbps && *num < bnum) {
            best = i;
        }
    }
    best
}

/// Normalises and checks the shape of an ISRC or ISWC.
/// ISRC: 2 letters, 3 letters or digits, 7 digits (hyphens removed).
/// ISWC: "T" and 10 digits (hyphens and dots removed). The ISWC check digit
/// is not verified.
pub fn normalize_code(code_type: CodeType, code: &str) -> Option<String> {
    let c: String = code
        .chars()
        .filter(|ch| *ch != '-' && *ch != '.' && *ch != ' ')
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    let b = c.as_bytes();
    let ok = match code_type {
        CodeType::Isrc => {
            b.len() == 12
                && b[..2].iter().all(u8::is_ascii_uppercase)
                && b[2..5].iter().all(u8::is_ascii_alphanumeric)
                && b[5..].iter().all(u8::is_ascii_digit)
        }
        CodeType::Iswc => b.len() == 11 && b[0] == b'T' && b[1..].iter().all(u8::is_ascii_digit),
    };
    ok.then_some(c)
}

/// 64-character lowercase hex.
pub fn is_hash32(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// "WW" or two uppercase letters.
pub fn is_territory(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_uppercase())
}

/// Advances a rolling window of hourly buckets to `now_hour`, clearing the
/// buckets of hours that fell out of the window. Returns the new bucket hour.
pub fn roll_window<T: Copy + Default, const N: usize>(
    buckets: &mut [T; N],
    bucket_hour: u64,
    now_hour: u64,
) -> u64 {
    if now_hour <= bucket_hour {
        return bucket_hour;
    }
    let gap = now_hour - bucket_hour;
    if gap >= N as u64 {
        *buckets = [T::default(); N];
    } else {
        for h in bucket_hour + 1..=now_hour {
            buckets[(h % N as u64) as usize] = T::default();
        }
    }
    now_hour
}

/// Parses "s<N>" into N.
pub fn share_number(token_id: &str) -> Option<u64> {
    token_id.strip_prefix('s')?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_sum(amount: u128, parts: &[SplitPart]) -> Vec<u128> {
        let c = split(amount, parts).unwrap();
        assert_eq!(c.iter().sum::<u128>(), amount, "sum must equal payment");
        c
    }

    #[test]
    fn even_split() {
        assert_eq!(
            check_sum(1_000_000, &[(1, 5000), (2, 5000)]),
            vec![500_000, 500_000]
        );
    }

    #[test]
    fn remainder_goes_to_largest_share() {
        // 3333 / 3333 / 3334 of 1: only the 3334 share gets the unit.
        assert_eq!(
            check_sum(1, &[(1, 3333), (2, 3333), (3, 3334)]),
            vec![0, 0, 1]
        );
        // 7 units: floors are 2, 2, 2 and the remainder 1 goes to 3334.
        assert_eq!(
            check_sum(7, &[(1, 3333), (2, 3334), (3, 3333)]),
            vec![2, 3, 2]
        );
    }

    #[test]
    fn ties_go_to_lowest_token_number() {
        assert_eq!(check_sum(1, &[(7, 5000), (3, 5000)]), vec![0, 1]);
        assert_eq!(
            check_sum(3, &[(2, 2500), (1, 2500), (4, 2500), (3, 2500)]),
            vec![0, 3, 0, 0]
        );
    }

    #[test]
    fn awkward_amounts_always_sum_exactly() {
        let groups: [&[SplitPart]; 4] = [
            &[(1, 3333), (2, 3333), (3, 3334)],
            &[(1, 1), (2, 9999)],
            &[(1, 10000)],
            &[(1, 1234), (2, 4321), (3, 2222), (4, 2223)],
        ];
        for parts in groups {
            for amount in [
                0u128,
                1,
                7,
                9_999,
                10_000,
                10_001,
                1_000_007,
                123_456_789_012,
            ] {
                check_sum(amount, parts);
            }
        }
    }

    #[test]
    fn remainder_is_smaller_than_part_count() {
        let parts = [(1, 3333), (2, 3333), (3, 3334)];
        for amount in 0..2_000u128 {
            let c = split(amount, &parts).unwrap();
            let floors: u128 = parts
                .iter()
                .map(|(_, b)| amount * u128::from(*b) / 10_000)
                .sum();
            assert!(c.iter().sum::<u128>() - floors < parts.len() as u128);
        }
    }

    #[test]
    fn refuses_anything_but_exactly_full() {
        assert!(split(100, &[(1, 5000)]).is_none());
        assert!(split(100, &[(1, 5000), (2, 5001)]).is_none());
        assert!(split(100, &[]).is_none());
    }

    #[test]
    fn overflow_is_refused_not_wrapped() {
        assert!(split(u128::MAX, &[(1, 5000), (2, 5000)]).is_none());
    }

    #[test]
    fn partial_split_escrows_the_unassigned_part() {
        let (c, u) = split_partial(1_000_000, &[(1, 5000), (2, 2500)]).unwrap();
        assert_eq!((c, u), (vec![500_000, 250_000], 250_000));
        // Rounding dust also stays unallocated, never over-credits a share.
        let (c, u) = split_partial(7, &[(1, 3333), (2, 3333)]).unwrap();
        assert_eq!((c.clone(), u), (vec![2, 2], 3));
        for amount in [0u128, 1, 7, 9_999, 10_001, 1_000_007] {
            let (c, u) = split_partial(amount, &[(1, 1234), (2, 4321)]).unwrap();
            assert_eq!(c.iter().sum::<u128>() + u, amount);
        }
        // Exactly full behaves like split: nothing unallocated.
        assert_eq!(
            split_partial(1, &[(1, 3333), (2, 3334), (3, 3333)]),
            Some((vec![0, 1, 0], 0))
        );
        assert!(split_partial(10, &[(1, 5000), (2, 5001)]).is_none());
        assert!(split_partial(10, &[]).is_none());
    }

    #[test]
    fn rolling_window_clears_only_expired_hours() {
        let mut b = [0u32; 24];
        b[10] = 5;
        b[11] = 7;
        assert_eq!(roll_window(&mut b, 11, 11), 11);
        assert_eq!(b.iter().sum::<u32>(), 12);
        // 23 hours later: hour 10 (index 10) is still inside, hour 34 clears index 10.
        roll_window(&mut b, 11, 33);
        assert_eq!(b[10], 5);
        assert_eq!(b[11], 7);
        roll_window(&mut b, 33, 34);
        assert_eq!(b[10], 0);
        assert_eq!(b[11], 7);
        roll_window(&mut b, 34, 100);
        assert_eq!(b.iter().sum::<u32>(), 0);
    }

    #[test]
    fn isrc_and_iswc_shapes() {
        assert_eq!(
            normalize_code(CodeType::Isrc, "us-rc1-26-00001").as_deref(),
            Some("USRC12600001")
        );
        assert!(normalize_code(CodeType::Isrc, "USRC1260000").is_none());
        assert!(normalize_code(CodeType::Isrc, "1SRC12600001").is_none());
        assert_eq!(
            normalize_code(CodeType::Iswc, "T-123.456.789-0").as_deref(),
            Some("T1234567890")
        );
        assert!(normalize_code(CodeType::Iswc, "X1234567890").is_none());
    }

    #[test]
    fn field_checks() {
        assert!(is_hash32(&"a1".repeat(32)));
        assert!(!is_hash32(&"A1".repeat(32)));
        assert!(!is_hash32("abc"));
        assert!(is_territory("WW") && is_territory("TH"));
        assert!(!is_territory("ww") && !is_territory("WWW"));
        assert_eq!(share_number("s12"), Some(12));
        assert_eq!(share_number("l12"), None);
    }
}
