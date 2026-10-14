# ZHiPManager

**Zone Hybrid Improvement Plan** — a responsive, browser-based archive creator and extractor with a separate `app.js` application script for the native `.zhip` format.

## Features

- Responsive interface for desktop and mobile
- Drag-and-drop or multi-file selection
- Combined input limit of 400 MiB for standard mode; Pro code unlocks an 800 MiB input limit
- Custom archive name with Fast, Smart hybrid, Maximum DEFLATE, and ULTRA ZSTD modes
- ULTRA ZSTD tests Zstandard levels 19–22, compares the smallest Zstandard candidate against the best DEFLATE result, and verifies selected Zstandard output with CRC-32 before storing it
- Creates ZHIP v2 archives for DEFLATE modes and ZHIP v4 archives for ULTRA ZSTD; reads existing ZHIP v1–v4 archives and verifies legacy signed v3 archives
- Progressive per-file compression tiers: Fast tests DEFLATE level 1; Smart tests levels 1 and 6; Maximum tests levels 1, 6, and 9; ULTRA compares those DEFLATE candidates with Zstandard level 19
- Each higher tier includes every candidate tested by the previous tier, so the stored archive payload cannot get larger when moving to a stronger tier for the same input; original bytes are kept when compression does not help
- CRC-32 integrity checks
- Optional ECDSA P-256 / SHA-256 digital signature and verification
- Visible timestamped logs for compression and extraction progress, per-file results, and errors
- Application JavaScript is separated into `app.js`
- Compression runs in a dedicated Web Worker so the page stays responsive during CPU-heavy work
- Content-addressed CHC archive cache: IndexedDB on the current browser and optional shared Supabase Storage cache across devices
- Pro-only Ultimate ZSTD mode with a prominent high-memory / potential corruption warning; Pro jobs bypass caching and remove their matching local/cloud cache entry after completion
- Cache entries expire after 48 hours; shared cache uploads are limited to 20 MiB, and only generated archive cache data is uploaded
- Cache keys use SHA-256 hashes of file contents, paths, and compression mode to avoid reusing a cache for different inputs
- Original selected files are processed locally; cloud cache stores only generated archive results

## ZHIP v3 binary format

This format is **not ZIP** and does not use ZIP container structures.

All integer fields are unsigned little-endian.

### Archive header (9 bytes)

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 | ASCII signature `ZHIP` |
| 4 | 1 | Format version (`3`) |
| 5 | 4 | Number of entries |

### Entry record

Each entry is stored sequentially as a 15-byte record header, UTF-8 path, then payload bytes.

| Field | Size | Meaning |
|---|---:|---|
| Path length | 2 bytes | UTF-8 path length |
| Original size | 4 bytes | Uncompressed byte count |
| Compressed size | 4 bytes | DEFLATE payload length |
| CRC-32 | 4 bytes | Checksum of original bytes |
| Codec | 1 byte | `0` = stored as-is, `1` = raw DEFLATE |
| Path | Variable | UTF-8 relative path |
| Payload | Variable | Original bytes for codec `0`; raw DEFLATE for codec `1` |

The container uses DEFLATE directly through `fflate.deflateSync` / `fflate.inflateSync`; it does not call ZIP archive functions. The compression tiers use nested candidate sets: Fast tests DEFLATE level 1; Smart tests levels 1 and 6; Maximum tests levels 1, 6, and 9; ULTRA tests all three DEFLATE candidates plus Zstandard levels 19, 20, 21, and 22. Each tier keeps the smallest candidate, including the original bytes as a fallback. This makes the stored payload size non-increasing from Fast to Smart to Maximum to ULTRA for the same inputs, though some file types cannot be compressed smaller. Signed ZHIP v3 archives use ECDSA P-256 digital signatures with SHA-256. Unsigned archives use ZHIP v2. The signature trailer is `SIGZ` (4 bytes), public-key length (uint16), signature length (uint16), DER/SPKI public key, then signature bytes. The signature covers every preceding archive byte. Verification proves integrity relative to the embedded key, but does not by itself prove the signer’s real-world identity; compare the fingerprint through a trusted channel. The reader also supports ZHIP v1 and v2. ZIP files are not accepted.


## ZHIP v4 (Zstandard codec)

ZHIP v4 keeps the 9-byte archive header and 15-byte entry record used by the v2 container. The version byte at offset 4 is `4` for archives created in ULTRA ZSTD mode.

The entry codec byte is:
- `0` — payload stored as-is
- `1` — raw DEFLATE
- `2` — Zstandard frame, compressed and decompressed with `@bokuweb/zstd-wasm`

Each entry retains the original byte length and CRC-32. During creation, ZHiPManager decompresses a Zstandard candidate and verifies both size and CRC-32 before storing it. Zstandard is selected only when it beats the best DEFLATE result and passes size and CRC-32 verification; otherwise the best DEFLATE result or original bytes are stored. The reader continues to support ZHIP v1–v3.

## Deployment

This is a static site. Deploy `index.html`, `app.js`, and the supporting files to Vercel with no build command and output directory `.`, or serve `index.html` using GitHub Pages.

The DEFLATE library and Zstandard WebAssembly module are loaded from jsDelivr, so an internet connection is needed when using those codecs. Zstandard levels 20–22 can be much slower and use substantially more memory. Compression and extraction happen in the browser. Peak RAM use can be significantly higher than the input size, especially near the 400 MiB limit.

## Project

- Website: **ZHiPManager**
- Format: **ZHIP v2 unsigned / ZHIP v3 signed (reads v1/v2)**
- Extension: **.zhip**
- Maximum combined input: **400 MiB**
