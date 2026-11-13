//! Share vault. One vault account per rights-share token, created by the
//! registry with zero access keys. It holds that share's token balance.
//!
//! No state, no owner, no init, no upgrade, no key management. The parent
//! registry is derived from the vault's own account id ("s12.<registry>"
//! gives "<registry>"). The only method is `withdraw`, callable only by the
//! parent, and it can only call `ft_transfer` on the token it is given.
//!
//! `withdraw` arguments are borsh: (token: String, receiver: String, amount: u128).
//! It returns the `ft_transfer` promise, so the registry callback sees
//! whether the transfer itself succeeded.
#![cfg_attr(target_arch = "wasm32", no_std)]

/// NEAR account ids are at most 64 bytes.
pub const MAX_ACCOUNT_LEN: usize = 64;
/// Gas passed to the token's `ft_transfer` (10 Tgas).
pub const GAS_FT_TRANSFER: u64 = 10_000_000_000_000;

/// Parent of a sub-account: everything after the first dot.
pub fn parent_of(account: &[u8]) -> Option<&[u8]> {
    let dot = account.iter().position(|b| *b == b'.')?;
    Some(&account[dot + 1..])
}

/// Accepts only bytes that can appear in a NEAR account id, so the value can
/// be placed inside JSON without escaping.
pub fn is_account_like(id: &[u8]) -> bool {
    (2..=MAX_ACCOUNT_LEN).contains(&id.len())
        && id
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(b))
}

/// Decoded `withdraw` arguments, borrowed from the input buffer.
pub struct Withdraw<'a> {
    pub token: &'a [u8],
    pub receiver: &'a [u8],
    pub amount: u128,
}

fn take_str<'a>(input: &'a [u8], at: &mut usize) -> Option<&'a [u8]> {
    let len = u32::from_le_bytes(input.get(*at..*at + 4)?.try_into().ok()?) as usize;
    let s = input.get(*at + 4..*at + 4 + len)?;
    *at += 4 + len;
    Some(s)
}

/// Strict borsh decode. Rejects trailing bytes, bad account ids and zero amounts.
pub fn decode_withdraw(input: &[u8]) -> Option<Withdraw<'_>> {
    let mut at = 0;
    let token = take_str(input, &mut at)?;
    let receiver = take_str(input, &mut at)?;
    let amount = u128::from_le_bytes(input.get(at..at + 16)?.try_into().ok()?);
    let ok =
        at + 16 == input.len() && is_account_like(token) && is_account_like(receiver) && amount > 0;
    ok.then_some(Withdraw {
        token,
        receiver,
        amount,
    })
}

/// Writes `{"receiver_id":"<receiver>","amount":"<amount>"}` into `out`.
pub fn ft_transfer_args(receiver: &[u8], amount: u128, out: &mut [u8; 160]) -> usize {
    let mut n = 0;
    let mut put = |bytes: &[u8]| {
        out[n..n + bytes.len()].copy_from_slice(bytes);
        n += bytes.len();
    };
    put(b"{\"receiver_id\":\"");
    put(receiver);
    put(b"\",\"amount\":\"");
    let mut digits = [0u8; 39];
    let mut i = digits.len();
    let mut v = amount;
    loop {
        i -= 1;
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    put(&digits[i..]);
    put(b"\"}");
    n
}

#[cfg(target_arch = "wasm32")]
mod contract {
    use super::*;
    use near_sys as sys;

    fn fail(msg: &str) -> ! {
        unsafe { sys::panic_utf8(msg.len() as u64, msg.as_ptr() as u64) }
    }

    /// Reads register 0 into `buf`, failing if it does not fit.
    fn read_into(buf: &mut [u8]) -> &[u8] {
        let len = unsafe { sys::register_len(0) } as usize;
        if len > buf.len() {
            fail("input too long");
        }
        unsafe { sys::read_register(0, buf.as_mut_ptr() as u64) };
        &buf[..len]
    }

    #[no_mangle]
    pub extern "C" fn withdraw() {
        let mut deposit = [0u8; 16];
        unsafe { sys::attached_deposit(deposit.as_mut_ptr() as u64) };
        if deposit != [0u8; 16] {
            fail("withdraw takes no deposit");
        }

        let mut me = [0u8; MAX_ACCOUNT_LEN];
        unsafe { sys::current_account_id(0) };
        let me = read_into(&mut me);
        let mut caller = [0u8; MAX_ACCOUNT_LEN];
        unsafe { sys::predecessor_account_id(0) };
        let caller = read_into(&mut caller);
        match parent_of(me) {
            Some(parent) if parent == caller => {}
            _ => fail("only the parent registry can call withdraw"),
        }

        let mut input = [0u8; 4 + MAX_ACCOUNT_LEN + 4 + MAX_ACCOUNT_LEN + 16];
        unsafe { sys::input(0) };
        let input = read_into(&mut input);
        let w = decode_withdraw(input).unwrap_or_else(|| fail("bad withdraw arguments"));

        let mut args = [0u8; 160];
        let args_len = ft_transfer_args(w.receiver, w.amount, &mut args);
        let method = b"ft_transfer";
        let one_yocto: u128 = 1;
        unsafe {
            let id = sys::promise_create(
                w.token.len() as u64,
                w.token.as_ptr() as u64,
                method.len() as u64,
                method.as_ptr() as u64,
                args_len as u64,
                args.as_ptr() as u64,
                &one_yocto as *const u128 as u64,
                GAS_FT_TRANSFER,
            );
            sys::promise_return(id);
        }
    }

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        core::arch::wasm32::unreachable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(token: &str, receiver: &str, amount: u128) -> Vec<u8> {
        let mut v = Vec::new();
        for s in [token, receiver] {
            v.extend_from_slice(&(s.len() as u32).to_le_bytes());
            v.extend_from_slice(s.as_bytes());
        }
        v.extend_from_slice(&amount.to_le_bytes());
        v
    }

    #[test]
    fn parent_is_everything_after_first_dot() {
        assert_eq!(parent_of(b"s12.reg.testnet"), Some(&b"reg.testnet"[..]));
        assert_eq!(parent_of(b"nodot"), None);
    }

    #[test]
    fn decodes_valid_arguments() {
        let raw = encode("usdc.reg.near", "alice.near", 42);
        let w = decode_withdraw(&raw).unwrap();
        assert_eq!(w.token, b"usdc.reg.near");
        assert_eq!(w.receiver, b"alice.near");
        assert_eq!(w.amount, 42);
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(decode_withdraw(&encode("usdc.near", "alice.near", 0)).is_none());
        assert!(decode_withdraw(&encode("usdc.near", "al\"ice", 1)).is_none());
        assert!(decode_withdraw(&encode("USDC.near", "alice.near", 1)).is_none());
        let mut extra = encode("usdc.near", "alice.near", 1);
        extra.push(0);
        assert!(decode_withdraw(&extra).is_none());
        let long = "a".repeat(65);
        assert!(decode_withdraw(&encode(&long, "alice.near", 1)).is_none());
        assert!(decode_withdraw(&[1, 2, 3]).is_none());
    }

    #[test]
    fn builds_ft_transfer_json() {
        let mut out = [0u8; 160];
        for amount in [1u128, 1_000_000, u128::MAX] {
            let n = ft_transfer_args(b"alice.near", amount, &mut out);
            let s = std::str::from_utf8(&out[..n]).unwrap();
            assert_eq!(
                s,
                format!("{{\"receiver_id\":\"alice.near\",\"amount\":\"{amount}\"}}")
            );
        }
        let long = [b'a'; MAX_ACCOUNT_LEN];
        let n = ft_transfer_args(&long, u128::MAX, &mut out);
        assert!(n <= out.len());
    }
}
