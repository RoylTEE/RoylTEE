//! WASI command run by OutLayer for the RoylTEE sealed-bid share auction.
//!
//! Reads one JSON request on stdin and the TEE secret
//! `PROTECTED_AUCTION_MASTER` from the environment, and writes one JSON line
//! to stdout.
//!
//! Modes:
//! - `pubkey`: prints the public key bidders encrypt to. The private key is
//!   derived from the master secret and never printed.
//! - `settle`: decrypts every bid, drops invalid ones, applies a second
//!   price (Vickrey) rule with a reserve and prints only the winner, the
//!   price and the sha256 of the bid list it decided on. Losing bids, the
//!   winner's own top bid, the count of valid bids and every plaintext stay
//!   inside the module.
//!
//! The pure logic lives here (`handle`) so it is tested on the host. The
//! `wasi` module at the bottom imports the WASI Preview 1 functions it needs
//! directly, so the file builds with the installed `wasm32-unknown-unknown`
//! target and runs under any WASI runtime (same method as the OutLayer
//! call-path spike).
#![cfg_attr(target_arch = "wasm32", no_main)]

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hkdf::Hkdf;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

/// Largest request accepted, in bytes.
pub const MAX_INPUT: usize = 64 * 1024;
/// Most bids a request may carry.
pub const MAX_BIDS: usize = 16;
/// Version tag inside every bid plaintext and every output.
pub const VERSION: u64 = 1;

const KEY_SALT: &[u8] = b"royltee-auction-v1";
const KEY_INFO: &[u8] = b"x25519-secret";
const BID_INFO: &[u8] = b"royltee-auction-bid-v1";
const HASH_PREFIX: &str = "royltee-auction-bids-v1";

#[derive(Deserialize)]
struct Request {
    mode: String,
    #[serde(default)]
    auction_id: String,
    #[serde(default)]
    contract: String,
    #[serde(default)]
    reserve: String,
    #[serde(default)]
    max_deposit: String,
    #[serde(default)]
    bids: Vec<BidIn>,
}

#[derive(Deserialize)]
struct BidIn {
    bidder: String,
    /// base64 of: ephemeral X25519 public key (32) || nonce (12) ||
    /// ChaCha20-Poly1305 ciphertext with tag.
    ct: String,
}

#[derive(Deserialize)]
struct BidPlain {
    v: u64,
    auction: String,
    contract: String,
    bidder: String,
    amount: String,
    #[allow(dead_code)]
    salt: String,
}

fn fail(msg: &str) -> String {
    serde_json::json!({ "v": VERSION, "ok": false, "error": msg }).to_string()
}

/// The auction key pair, derived from the TEE master secret.
pub fn derive_secret(master: &str) -> StaticSecret {
    let hk = Hkdf::<Sha256>::new(Some(KEY_SALT), master.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(KEY_INFO, &mut okm)
        .expect("32 bytes is a valid length");
    StaticSecret::from(okm)
}

pub fn public_key_hex(master: &str) -> String {
    hex(PublicKey::from(&derive_secret(master)).as_bytes())
}

pub fn hex(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &c in b {
        s.push(H[(c >> 4) as usize] as char);
        s.push(H[(c & 15) as usize] as char);
    }
    s
}

/// Standard base64 decode (with or without padding). None on any bad byte.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    let t = s.trim_end_matches('=');
    for c in t.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // Leftover bits must be zero padding.
    if bits >= 6 || acc != 0 {
        return None;
    }
    Some(out)
}

/// sha256 hex of the canonical bid list. The contract computes the same
/// value from its own state, so anyone can check which bids a result covers.
pub fn bids_hash(auction_id: &str, bids: &[(String, String)]) -> String {
    let mut h = Sha256::new();
    h.update(HASH_PREFIX.as_bytes());
    h.update(b"\n");
    h.update(auction_id.as_bytes());
    h.update(b"\n");
    for (bidder, ct) in bids {
        h.update(bidder.as_bytes());
        h.update(b"\n");
        h.update(ct.as_bytes());
        h.update(b"\n");
    }
    hex(&h.finalize())
}

/// Decrypts one bid. None if anything is wrong with it. Never reports why,
/// so the output cannot be used to probe a ciphertext.
fn open_bid(sk: &StaticSecret, ct_b64: &str) -> Option<BidPlain> {
    let raw = b64_decode(ct_b64)?;
    if raw.len() < 32 + 12 + 16 {
        return None;
    }
    let (eph, rest) = raw.split_at(32);
    let (nonce, body) = rest.split_at(12);
    let mut eph_pk = [0u8; 32];
    eph_pk.copy_from_slice(eph);
    let shared = sk.diffie_hellman(&PublicKey::from(eph_pk));
    if !shared.was_contributory() {
        return None;
    }
    let my_pk = PublicKey::from(sk);
    let mut salt = Vec::with_capacity(64);
    salt.extend_from_slice(&eph_pk);
    salt.extend_from_slice(my_pk.as_bytes());
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(BID_INFO, &mut key).ok()?;
    let cipher = ChaCha20Poly1305::new(&key.into());
    let plain = cipher.decrypt(Nonce::from_slice(nonce), body).ok()?;
    serde_json::from_slice::<BidPlain>(&plain).ok()
}

fn parse_amount(s: &str) -> Option<u128> {
    if s.is_empty() || s.len() > 38 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Second price with a reserve. `valid` holds (index, amount) for every valid
/// bid, already at or above the reserve. Highest amount wins, the earlier
/// index wins a tie. Price is the second highest amount, or the reserve if
/// there is only one valid bid.
pub fn pick_winner(valid: &[(usize, u128)], reserve: u128) -> Option<(usize, u128)> {
    let mut best: Option<(usize, u128)> = None;
    for &(i, a) in valid {
        match best {
            Some((_, ba)) if a <= ba => {}
            _ => best = Some((i, a)),
        }
    }
    let (wi, wa) = best?;
    let mut second: Option<u128> = None;
    for &(i, a) in valid {
        if i == wi {
            continue;
        }
        second = Some(second.map_or(a, |s| s.max(a)));
    }
    let _ = wa;
    Some((wi, second.unwrap_or(reserve).max(reserve)))
}

/// Handles one request. `master` is the value of `PROTECTED_AUCTION_MASTER`.
pub fn handle(input: &str, master: Option<&str>) -> String {
    if input.len() > MAX_INPUT {
        return fail("input too large");
    }
    let req: Request = match serde_json::from_str(input) {
        Ok(r) => r,
        Err(_) => return fail("bad request json"),
    };
    let Some(master) = master.filter(|m| m.len() >= 32) else {
        return fail("master secret missing");
    };
    match req.mode.as_str() {
        "pubkey" => serde_json::json!({
            "v": VERSION, "ok": true, "pubkey": public_key_hex(master)
        })
        .to_string(),
        "settle" => settle(&req, master),
        _ => fail("unknown mode"),
    }
}

fn settle(req: &Request, master: &str) -> String {
    let (Some(reserve), Some(max_deposit)) =
        (parse_amount(&req.reserve), parse_amount(&req.max_deposit))
    else {
        return fail("bad reserve or max_deposit");
    };
    if req.auction_id.is_empty() || req.contract.is_empty() || req.bids.len() > MAX_BIDS {
        return fail("bad auction fields");
    }
    if reserve == 0 || reserve > max_deposit {
        return fail("bad reserve");
    }
    let sk = derive_secret(master);
    let pairs: Vec<(String, String)> = req
        .bids
        .iter()
        .map(|b| (b.bidder.clone(), b.ct.clone()))
        .collect();
    let hash = bids_hash(&req.auction_id, &pairs);

    let mut valid: Vec<(usize, u128)> = Vec::new();
    for (i, b) in req.bids.iter().enumerate() {
        let Some(p) = open_bid(&sk, &b.ct) else {
            continue;
        };
        // Bound to this auction, this contract and the account that
        // deposited, so a copied ciphertext is dropped.
        if p.v != VERSION
            || p.auction != req.auction_id
            || p.contract != req.contract
            || p.bidder != b.bidder
        {
            continue;
        }
        let Some(amount) = parse_amount(&p.amount) else {
            continue;
        };
        if amount < reserve || amount > max_deposit {
            continue;
        }
        valid.push((i, amount));
    }
    match pick_winner(&valid, reserve) {
        Some((wi, price)) => serde_json::json!({
            "v": VERSION, "ok": true, "auction": req.auction_id, "bids_hash": hash,
            "winner": req.bids[wi].bidder, "price": price.to_string()
        })
        .to_string(),
        None => serde_json::json!({
            "v": VERSION, "ok": true, "auction": req.auction_id, "bids_hash": hash,
            "winner": serde_json::Value::Null, "price": serde_json::Value::Null
        })
        .to_string(),
    }
}

#[cfg(target_arch = "wasm32")]
mod wasi {
    use super::{handle, MAX_INPUT};

    #[repr(C)]
    struct Iovec {
        buf: *mut u8,
        len: usize,
    }
    #[repr(C)]
    struct Ciovec {
        buf: *const u8,
        len: usize,
    }

    #[link(wasm_import_module = "wasi_snapshot_preview1")]
    extern "C" {
        fn fd_read(fd: u32, iovs: *const Iovec, n: usize, nread: *mut usize) -> u16;
        fn fd_write(fd: u32, iovs: *const Ciovec, n: usize, nw: *mut usize) -> u16;
        fn environ_sizes_get(count: *mut usize, size: *mut usize) -> u16;
        fn environ_get(environ: *mut *mut u8, buf: *mut u8) -> u16;
        fn proc_exit(code: u32) -> !;
    }

    fn read_stdin() -> Vec<u8> {
        let mut data = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let iov = Iovec {
                buf: chunk.as_mut_ptr(),
                len: chunk.len(),
            };
            let mut n = 0usize;
            // SAFETY: valid iovec and out pointer for the call.
            let err = unsafe { fd_read(0, &iov, 1, &mut n) };
            if err != 0 || n == 0 {
                break;
            }
            data.extend_from_slice(&chunk[..n]);
            if data.len() > MAX_INPUT + 1 {
                break;
            }
        }
        data
    }

    fn env_var(name: &str) -> Option<String> {
        let (mut count, mut size) = (0usize, 0usize);
        // SAFETY: valid out pointers.
        if unsafe { environ_sizes_get(&mut count, &mut size) } != 0 || count == 0 {
            return None;
        }
        let mut ptrs: Vec<*mut u8> = vec![core::ptr::null_mut(); count];
        let mut buf = vec![0u8; size];
        // SAFETY: buffers are sized as the host reported.
        if unsafe { environ_get(ptrs.as_mut_ptr(), buf.as_mut_ptr()) } != 0 {
            return None;
        }
        let base = buf.as_ptr() as usize;
        let prefix = format!("{name}=");
        for p in ptrs {
            let off = (p as usize).checked_sub(base)?;
            let rest = buf.get(off..)?;
            let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
            if let Ok(s) = core::str::from_utf8(&rest[..end]) {
                if let Some(v) = s.strip_prefix(&prefix) {
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    fn write_stdout(bytes: &[u8]) {
        let mut off = 0;
        while off < bytes.len() {
            let iov = Ciovec {
                buf: bytes[off..].as_ptr(),
                len: bytes.len() - off,
            };
            let mut n = 0usize;
            // SAFETY: valid iovec and out pointer for the call.
            let err = unsafe { fd_write(1, &iov, 1, &mut n) };
            if err != 0 || n == 0 {
                break;
            }
            off += n;
        }
    }

    #[no_mangle]
    pub extern "C" fn _start() {
        let input = read_stdin();
        let out = match core::str::from_utf8(&input) {
            Ok(s) => handle(s, env_var("PROTECTED_AUCTION_MASTER").as_deref()),
            Err(_) => super::fail("input is not utf8"),
        };
        write_stdout(out.as_bytes());
        write_stdout(b"\n");
        // SAFETY: ends the process with success.
        unsafe { proc_exit(0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chacha20poly1305::aead::Aead;

    const MASTER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const CONTRACT: &str = "auction.example.testnet";

    fn b64(b: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            s.push(T[(n >> 18) as usize & 63] as char);
            s.push(T[(n >> 12) as usize & 63] as char);
            s.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    }

    /// Test-only sealer with a fixed ephemeral key (same format the Node
    /// script makes).
    fn seal(plain: &str, eph_seed: u8, nonce_seed: u8) -> String {
        let recipient = PublicKey::from(&derive_secret(MASTER));
        let eph = StaticSecret::from([eph_seed; 32]);
        let eph_pub = PublicKey::from(&eph);
        let shared = eph.diffie_hellman(&recipient);
        let mut salt = eph_pub.as_bytes().to_vec();
        salt.extend_from_slice(recipient.as_bytes());
        let hk = Hkdf::<Sha256>::new(Some(&salt), shared.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(BID_INFO, &mut key).unwrap();
        let nonce = [nonce_seed; 12];
        let ct = ChaCha20Poly1305::new(&key.into())
            .encrypt(Nonce::from_slice(&nonce), plain.as_bytes())
            .unwrap();
        let mut raw = eph_pub.as_bytes().to_vec();
        raw.extend_from_slice(&nonce);
        raw.extend_from_slice(&ct);
        b64(&raw)
    }

    fn plain(auction: &str, bidder: &str, amount: &str) -> String {
        format!(
            r#"{{"v":1,"auction":"{auction}","contract":"{CONTRACT}","bidder":"{bidder}","amount":"{amount}","salt":"00"}}"#
        )
    }

    fn req(bids: &[(&str, String)], reserve: &str, max: &str) -> String {
        serde_json::json!({
            "mode": "settle", "auction_id": "3", "contract": CONTRACT,
            "reserve": reserve, "max_deposit": max,
            "bids": bids.iter().map(|(b, c)| serde_json::json!({"bidder": b, "ct": c})).collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn run(r: &str) -> serde_json::Value {
        serde_json::from_str(&handle(r, Some(MASTER))).unwrap()
    }

    #[test]
    fn second_price_with_a_reserve() {
        let v = [(0, 50u128), (1, 70), (2, 60)];
        assert_eq!(pick_winner(&v, 10), Some((1, 60)));
        assert_eq!(
            pick_winner(&[(0, 50)], 10),
            Some((0, 10)),
            "one bid pays the reserve"
        );
        assert_eq!(
            pick_winner(&[(0, 50), (1, 5)], 10),
            Some((0, 10)),
            "second below reserve"
        );
        assert_eq!(pick_winner(&[], 10), None);
        assert_eq!(
            pick_winner(&[(0, 50), (1, 50)], 10),
            Some((0, 50)),
            "tie: earlier bid wins"
        );
        assert_eq!(pick_winner(&[(0, 40), (1, 50), (2, 50)], 10), Some((1, 50)));
    }

    #[test]
    fn base64_decoder_is_strict() {
        assert_eq!(b64_decode("QUJD"), Some(b"ABC".to_vec()));
        assert_eq!(b64_decode("QUI="), Some(b"AB".to_vec()));
        assert_eq!(b64_decode("QQ=="), Some(b"A".to_vec()));
        assert_eq!(b64_decode("QQ"), Some(b"A".to_vec()));
        assert_eq!(b64_decode("QU Jd"), None);
        assert_eq!(b64_decode("Q"), None);
        assert_eq!(b64_decode("QR=="), None, "non-zero padding bits");
        assert_eq!(b64_decode("-_"), None, "url alphabet is not accepted");
    }

    #[test]
    fn key_is_deterministic_and_secret_dependent() {
        assert_eq!(public_key_hex(MASTER), public_key_hex(MASTER));
        assert_ne!(
            public_key_hex(MASTER),
            public_key_hex(&MASTER.replace('0', "1"))
        );
        assert_eq!(public_key_hex(MASTER).len(), 64);
        let out = run(r#"{"mode":"pubkey"}"#);
        assert_eq!(out["pubkey"], public_key_hex(MASTER));
        assert_eq!(out["ok"], true);
        assert!(!handle(r#"{"mode":"pubkey"}"#, Some(MASTER)).contains(MASTER));
    }

    #[test]
    fn settle_picks_the_second_price() {
        let r = req(
            &[
                ("a.testnet", seal(&plain("3", "a.testnet", "50"), 1, 1)),
                ("b.testnet", seal(&plain("3", "b.testnet", "70"), 2, 2)),
                ("c.testnet", seal(&plain("3", "c.testnet", "60"), 3, 3)),
            ],
            "10",
            "100",
        );
        let out = run(&r);
        assert_eq!(out["ok"], true);
        assert_eq!(out["winner"], "b.testnet");
        assert_eq!(out["price"], "60");
        assert_eq!(out["auction"], "3");
        // Only these keys leave the module.
        let mut keys: Vec<_> = out.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["auction", "bids_hash", "ok", "price", "v", "winner"]);
    }

    #[test]
    fn invalid_bids_are_dropped_silently() {
        let good = seal(&plain("3", "a.testnet", "50"), 1, 1);
        let r = req(
            &[
                ("a.testnet", good.clone()),
                // Copy of a's ciphertext sent by another account.
                ("copy.testnet", good.clone()),
                // Wrong auction, wrong contract, over max, under reserve, junk amount.
                ("d.testnet", seal(&plain("4", "d.testnet", "99"), 4, 4)),
                (
                    "e.testnet",
                    seal(
                        &plain("3", "e.testnet", "99").replace(CONTRACT, "other.testnet"),
                        5,
                        5,
                    ),
                ),
                ("f.testnet", seal(&plain("3", "f.testnet", "101"), 6, 6)),
                ("g.testnet", seal(&plain("3", "g.testnet", "9"), 7, 7)),
                ("h.testnet", seal(&plain("3", "h.testnet", "1e2"), 8, 8)),
                ("i.testnet", seal(&plain("3", "i.testnet", "-5"), 9, 9)),
                (
                    "j.testnet",
                    "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo".to_string(),
                ),
                ("k.testnet", "not base64!".to_string()),
                (
                    "l.testnet",
                    seal(
                        &plain("3", "l.testnet", "99").replace("\"v\":1", "\"v\":2"),
                        10,
                        10,
                    ),
                ),
            ],
            "10",
            "100",
        );
        let out = run(&r);
        assert_eq!(out["winner"], "a.testnet");
        assert_eq!(out["price"], "10", "only one valid bid: the reserve");
    }

    #[test]
    fn a_flipped_byte_breaks_the_tag() {
        let good = seal(&plain("3", "a.testnet", "50"), 1, 1);
        let mut raw = b64_decode(&good).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 1;
        let r = req(&[("a.testnet", b64(&raw))], "10", "100");
        assert_eq!(run(&r)["winner"], serde_json::Value::Null);
    }

    #[test]
    fn no_valid_bid_gives_no_winner() {
        let r = req(
            &[("a.testnet", seal(&plain("3", "a.testnet", "5"), 1, 1))],
            "10",
            "100",
        );
        let out = run(&r);
        assert_eq!(out["ok"], true);
        assert!(out["winner"].is_null() && out["price"].is_null());
        let out = run(&req(&[], "10", "100"));
        assert!(out["winner"].is_null());
    }

    #[test]
    fn bad_requests_fail_without_leaking() {
        let f = |r: &str, m: Option<&str>| {
            serde_json::from_str::<serde_json::Value>(&handle(r, m)).unwrap()
        };
        assert_eq!(f("not json", Some(MASTER))["ok"], false);
        assert_eq!(f(r#"{"mode":"pubkey"}"#, None)["ok"], false);
        assert_eq!(f(r#"{"mode":"pubkey"}"#, Some("short"))["ok"], false);
        assert_eq!(f(r#"{"mode":"nope"}"#, Some(MASTER))["ok"], false);
        assert_eq!(
            f(&req(&[], "0", "100"), Some(MASTER))["ok"],
            false,
            "zero reserve"
        );
        assert_eq!(
            f(&req(&[], "200", "100"), Some(MASTER))["ok"],
            false,
            "reserve above max"
        );
        assert_eq!(f(&req(&[], "x", "100"), Some(MASTER))["ok"], false);
        let many: Vec<(&str, String)> = (0..MAX_BIDS + 1)
            .map(|_| ("a.testnet", String::new()))
            .collect();
        assert_eq!(
            f(&req(&many, "10", "100"), Some(MASTER))["ok"],
            false,
            "too many bids"
        );
        let big = format!(
            "{{\"mode\":\"pubkey\",\"pad\":\"{}\"}}",
            "x".repeat(MAX_INPUT)
        );
        assert_eq!(f(&big, Some(MASTER))["ok"], false);
    }

    #[test]
    fn bids_hash_matches_the_contract_golden_value() {
        let h = bids_hash(
            "7",
            &[
                ("a.testnet".into(), "QUJD".into()),
                ("b.testnet".into(), "REVG".into()),
            ],
        );
        assert_eq!(h, include_str!("../fixtures/golden_bids_hash.txt").trim());
    }

    #[test]
    fn the_output_names_the_bid_list_it_decided_on() {
        let bids = [("a.testnet", seal(&plain("3", "a.testnet", "50"), 1, 1))];
        let out = run(&req(&bids, "10", "100"));
        let pairs: Vec<(String, String)> = bids
            .iter()
            .map(|(b, c)| (b.to_string(), c.clone()))
            .collect();
        assert_eq!(out["bids_hash"], bids_hash("3", &pairs));
    }
}
