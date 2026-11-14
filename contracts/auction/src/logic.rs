//! Pure helpers, unit tested on the host.
use crate::types::Bid;
use near_sdk::env;

pub fn is_hash64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_pubkey_hex(s: &str) -> bool {
    is_hash64(s)
}

/// X25519 public keys with a small order (the point at zero, one, the two
/// order 8 points, and the three encodings of the field prime and its
/// neighbours). A shared secret made with one of them is the same for every
/// private key, so a key like that must never be published. The top bit of
/// the last byte is ignored by X25519, so it is cleared before comparing.
const LOW_ORDER: [&str; 7] = [
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0100000000000000000000000000000000000000000000000000000000000000",
    "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800",
    "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157",
    "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
];

/// A published key: 64 lowercase hex characters and not a low order point.
pub fn is_usable_pubkey(s: &str) -> bool {
    if !is_pubkey_hex(s) {
        return false;
    }
    let (head, last) = s.split_at(62);
    let top = u8::from_str_radix(last, 16).unwrap_or(0) & 0x7f;
    let masked = format!("{head}{top:02x}");
    !LOW_ORDER.contains(&masked.as_str())
}

/// Base64 alphabet check only (standard alphabet, optional `=` padding at the end).
pub fn looks_like_base64(s: &str) -> bool {
    let t = s.trim_end_matches('=');
    s.len() - t.len() <= 2
        && !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

pub fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}

/// Canonical text the TEE module and this contract both hash. Must match
/// `bids_hash` in `tee/auction-wasi/src/lib.rs`.
pub fn bids_text(auction_id: &str, bids: &[Bid]) -> String {
    let mut t = String::new();
    t.push_str("royltee-auction-bids-v1\n");
    t.push_str(auction_id);
    t.push('\n');
    for b in bids {
        t.push_str(b.bidder.as_str());
        t.push('\n');
        t.push_str(&b.ciphertext);
        t.push('\n');
    }
    t
}

pub fn bids_hash(auction_id: &str, bids: &[Bid]) -> String {
    hex(&env::sha256(bids_text(auction_id, bids).as_bytes()))
}

/// What the TEE module answered for a settle request.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Winner and price (micro mUSDC).
    Sale(String, u128),
    /// No valid bid at or above the reserve.
    NoSale,
}

/// Unwraps the older `{success, result}` answer shape. A failed run is an
/// error; a plain guest JSON passes through unchanged.
pub fn unwrap_answer(
    v: near_sdk::serde_json::Value,
) -> Result<near_sdk::serde_json::Value, String> {
    use near_sdk::serde_json::{self, Value};
    // A guest answer delivered as a JSON string is read once. Every check
    // after this is strict, so it can only make a good answer readable.
    if let Value::String(text) = &v {
        return serde_json::from_str::<Value>(text)
            .map_err(|_| "answer is a string that is not JSON".to_string())
            .and_then(|inner| {
                if inner.is_string() {
                    Err("answer is nested too deep".to_string())
                } else {
                    unwrap_answer(inner)
                }
            });
    }
    if v.get("ok").is_none() && (v.get("success").is_some() || v.get("result").is_some()) {
        return match (v.get("success").and_then(Value::as_bool), v.get("result")) {
            (Some(true), Some(result)) => Ok(result.clone()),
            (Some(false), _) => Err("OutLayer reported a failed run".into()),
            _ => Err("answer wrapper is malformed".into()),
        };
    }
    Ok(v)
}

/// Reads the OutLayer answer for a settle request. Accepts the guest JSON as
/// returned today and the older `{success, result}` wrapper. Every check is
/// strict: a wrong auction id or bid list hash is an error, never a sale.
pub fn parse_settle_output(
    raw: &[u8],
    auction_id: &str,
    expected_hash: &str,
) -> Result<Outcome, String> {
    use near_sdk::serde_json::{self, Value};
    let v: Value = serde_json::from_slice(raw).map_err(|_| "answer is not JSON".to_string())?;
    if v.is_null() {
        return Err("null answer: the run failed or printed nothing".into());
    }
    let v = unwrap_answer(v)?;
    if v.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err("module reported ok=false".into());
    }
    if v.get("v").and_then(Value::as_u64) != Some(1) {
        return Err("unknown answer version".into());
    }
    if v.get("auction").and_then(Value::as_str) != Some(auction_id) {
        return Err("answer is for another auction".into());
    }
    if v.get("bids_hash").and_then(Value::as_str) != Some(expected_hash) {
        return Err("answer covers a different bid list".into());
    }
    match (v.get("winner"), v.get("price")) {
        (Some(Value::Null), Some(Value::Null)) => Ok(Outcome::NoSale),
        (Some(Value::String(w)), Some(Value::String(p))) => {
            if p.is_empty() || p.len() > 38 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err("price is not a decimal string".into());
            }
            let price: u128 = p.parse().map_err(|_| "price out of range".to_string())?;
            Ok(Outcome::Sale(w.clone(), price))
        }
        _ => Err("winner and price are malformed".into()),
    }
}
