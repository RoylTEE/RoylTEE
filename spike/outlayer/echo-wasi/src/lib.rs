//! Tiny WASI Preview 1 command for the OutLayer call-path check.
//!
//! Reads all of stdin and writes one JSON line to stdout:
//! `{"ok":true,"echo":"<stdin as a JSON string>","len":<stdin bytes>}`.
//! The input is echoed as an escaped JSON string, so the output is valid
//! JSON whatever the input is. Input over `MAX_IN` bytes is cut and
//! `"truncated":true` is added.
//!
//! It is `no_std` and imports the three WASI Preview 1 functions it needs
//! directly, so it builds with the installed `wasm32-unknown-unknown` target.
//! A WASI runtime (wasmtime in the OutLayer worker) links the imports and
//! calls the exported `_start`, the same as for a `wasm32-wasip1` binary.
#![cfg_attr(target_arch = "wasm32", no_std)]

/// Largest input read, in bytes.
pub const MAX_IN: usize = 16 * 1024;
/// Output buffer: worst case every input byte becomes a 6 byte `\u00XX`.
pub const MAX_OUT: usize = MAX_IN * 6 + 128;

/// Writes the response for `input` into `out` and returns its length.
/// `out` must be at least `input.len() * 6 + 128` bytes.
pub fn render(input: &[u8], truncated: bool, out: &mut [u8]) -> usize {
    let mut n = 0;
    let mut put = |b: &[u8], n: &mut usize| {
        out[*n..*n + b.len()].copy_from_slice(b);
        *n += b.len();
    };
    put(b"{\"ok\":true,\"echo\":\"", &mut n);
    for &c in input {
        match c {
            b'"' => put(b"\\\"", &mut n),
            b'\\' => put(b"\\\\", &mut n),
            b'\n' => put(b"\\n", &mut n),
            b'\r' => put(b"\\r", &mut n),
            b'\t' => put(b"\\t", &mut n),
            0x00..=0x1f | 0x7f..=0xff => {
                // Control bytes and non ASCII bytes are escaped one byte at a
                // time, so invalid UTF-8 can never reach the output.
                const HEX: &[u8; 16] = b"0123456789abcdef";
                put(
                    &[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(c >> 4) as usize],
                        HEX[(c & 15) as usize],
                    ],
                    &mut n,
                );
            }
            _ => put(&[c], &mut n),
        }
    }
    put(b"\",\"len\":", &mut n);
    let mut digits = [0u8; 20];
    let mut v = input.len();
    let mut d = 0;
    loop {
        digits[d] = b'0' + (v % 10) as u8;
        d += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while d > 0 {
        d -= 1;
        put(&[digits[d]], &mut n);
    }
    if truncated {
        put(b",\"truncated\":true", &mut n);
    }
    put(b"}\n", &mut n);
    n
}

#[cfg(target_arch = "wasm32")]
mod wasi {
    #[repr(C)]
    pub struct Iovec {
        pub buf: *const u8,
        pub len: usize,
    }

    #[link(wasm_import_module = "wasi_snapshot_preview1")]
    extern "C" {
        pub fn fd_read(fd: i32, iovs: *const Iovec, iovs_len: i32, nread: *mut usize) -> i32;
        pub fn fd_write(fd: i32, iovs: *const Iovec, iovs_len: i32, nwritten: *mut usize) -> i32;
        pub fn proc_exit(code: i32) -> !;
    }
}

#[cfg(target_arch = "wasm32")]
static mut IN_BUF: [u8; MAX_IN + 1] = [0; MAX_IN + 1];
#[cfg(target_arch = "wasm32")]
static mut OUT_BUF: [u8; MAX_OUT] = [0; MAX_OUT];

/// WASI command entry point.
///
/// # Safety
/// Called once by the WASI runtime; the static buffers are used by this
/// single thread only.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn _start() {
    use core::ptr::addr_of_mut;
    let inb = &mut *addr_of_mut!(IN_BUF);
    let outb = &mut *addr_of_mut!(OUT_BUF);
    let mut len = 0usize;
    while len < inb.len() {
        let iov = wasi::Iovec {
            buf: inb[len..].as_ptr(),
            len: inb.len() - len,
        };
        let mut got = 0usize;
        if wasi::fd_read(0, &iov, 1, &mut got) != 0 || got == 0 {
            break;
        }
        len += got;
    }
    let truncated = len > MAX_IN;
    let len = len.min(MAX_IN);
    let n = render(&inb[..len], truncated, outb);
    let mut done = 0usize;
    while done < n {
        let iov = wasi::Iovec {
            buf: outb[done..].as_ptr(),
            len: n - done,
        };
        let mut wrote = 0usize;
        if wasi::fd_write(1, &iov, 1, &mut wrote) != 0 || wrote == 0 {
            wasi::proc_exit(1);
        }
        done += wrote;
    }
    wasi::proc_exit(0);
}

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &[u8], truncated: bool) -> String {
        let mut out = vec![0u8; input.len() * 6 + 128];
        let n = render(input, truncated, &mut out);
        String::from_utf8(out[..n].to_vec()).unwrap()
    }

    #[test]
    fn echoes_json_input_as_string() {
        let s = run(br#"{"hello":"tee"}"#, false);
        assert_eq!(
            s,
            "{\"ok\":true,\"echo\":\"{\\\"hello\\\":\\\"tee\\\"}\",\"len\":15}\n"
        );
    }

    #[test]
    fn output_is_valid_json_for_any_bytes() {
        let input: Vec<u8> = (0u8..=255).collect();
        let s = run(&input, false);
        let v: serde_json_check::Value = serde_json_check::parse(&s);
        assert!(v.ok);
        assert_eq!(v.len, 256);
    }

    #[test]
    fn empty_input() {
        assert_eq!(run(b"", false), "{\"ok\":true,\"echo\":\"\",\"len\":0}\n");
    }

    #[test]
    fn marks_truncation() {
        assert!(run(b"x", true).ends_with(",\"len\":1,\"truncated\":true}\n"));
    }

    #[test]
    fn worst_case_fits_buffer() {
        let input = vec![0x01u8; MAX_IN];
        let mut out = vec![0u8; MAX_OUT];
        let n = render(&input, true, &mut out);
        assert!(n <= MAX_OUT);
    }

    /// Minimal checker so the crate needs no dependencies: confirms the
    /// output is one JSON object with the expected keys by round tripping
    /// the escapes.
    mod serde_json_check {
        pub struct Value {
            pub ok: bool,
            pub len: usize,
        }
        pub fn parse(s: &str) -> Value {
            assert!(s.starts_with("{\"ok\":true,\"echo\":\""));
            assert!(s.is_ascii(), "output must be pure ASCII");
            let len_at = s.rfind(",\"len\":").unwrap();
            let len: usize = s[len_at + 7..].trim_end_matches("}\n").parse().unwrap();
            // every backslash starts a valid escape
            let body = &s[19..len_at - 1];
            let b = body.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'\\' {
                    match b[i + 1] {
                        b'"' | b'\\' | b'n' | b'r' | b't' => i += 2,
                        b'u' => i += 6,
                        other => panic!("bad escape {other}"),
                    }
                } else {
                    assert!(b[i] != b'"', "unescaped quote");
                    i += 1;
                }
            }
            Value { ok: true, len }
        }
    }
}
