#!/usr/bin/env python3
"""Builds the FastFS upload payload (Borsh, FastfsData::Simple) for one file.

No keys, no network. Schema from fastnear/near-outlayer
scripts/upload_wasm_fastfs.py (matches fastnear/fastdata-drag-and-drop):
  enum FastfsData { Simple(SimpleFastfs) = 0, Partial(..) = 1 }
  SimpleFastfs { relative_path: String, content: Option<FastfsFileContent> }
  FastfsFileContent { mime_type: String, content: Vec<u8> }
The file is served at https://test.fastfs.io/<sender>/<receiver>/<relative_path>
once the indexer has read the transaction. Files over 1 MiB need the Partial
form; this spike's file is a few KB, so it is refused here.

Usage: fastfs_payload.py <file.wasm> <out.bin>
"""
import hashlib
import struct
import sys

def bstr(s: str) -> bytes:
    b = s.encode()
    return struct.pack("<I", len(b)) + b

def main() -> None:
    src, dst = sys.argv[1], sys.argv[2]
    data = open(src, "rb").read()
    if len(data) > (1 << 20):
        sys.exit("file over 1 MiB needs the chunked form")
    digest = hashlib.sha256(data).hexdigest()
    payload = (
        struct.pack("<B", 0)
        + bstr(f"{digest}.wasm")
        + struct.pack("<B", 1)
        + bstr("application/wasm")
        + struct.pack("<I", len(data))
        + data
    )
    open(dst, "wb").write(payload)
    print(f"file_sha256={digest}")
    print(f"file_bytes={len(data)}")
    print(f"payload_bytes={len(payload)}")
    print(f"payload_sha256={hashlib.sha256(payload).hexdigest()}")
    print(f"relative_path={digest}.wasm")

if __name__ == "__main__":
    main()
