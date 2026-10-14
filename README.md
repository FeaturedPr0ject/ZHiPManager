# ZHiPManager

**Zone Hybrid Improvement Plan** — a responsive, browser-based archive creator and extractor for the native `.zhip` format.

## Features

- Responsive interface for desktop and mobile
- Drag-and-drop or multi-file selection
- Combined input limit of 400 MiB
- Custom archive name with Fast, Smart hybrid, and Maximum DEFLATE modes
- Create and extract native ZHIP v2 archives; read existing v1 archives
- Per-file hybrid selection: tests DEFLATE levels and keeps the smallest result; stores original bytes if compression would increase size
- CRC-32 integrity checks
- Client-side processing; selected file contents are not uploaded to an application server

## ZHIP v2 binary format

This format is **not ZIP** and does not use ZIP container structures.

All integer fields are unsigned little-endian.

### Archive header (9 bytes)

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 | ASCII signature `ZHIP` |
| 4 | 1 | Format version (`2`) |
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

The container uses DEFLATE directly through `fflate.deflateSync` / `fflate.inflateSync`; it does not call ZIP archive functions. Smart hybrid mode compares levels 1, 6, and 9 per file and picks the smallest result. If the compressed result is not smaller than the original, the payload is stored as-is to prevent compression overhead from inflating that file. The reader supports ZHIP v1 and v2. ZIP files are not accepted.

## Deployment

This is a static site. Deploy the repository to Vercel with no build command and output directory `.`, or serve `index.html` using GitHub Pages.

The compression library is loaded from jsDelivr, so an internet connection is needed to load it. Compression and extraction happen in the browser. Peak RAM use can be significantly higher than the input size, especially near the 400 MiB limit.

## Project

- Website: **ZHiPManager**
- Format: **ZHIP v2 (reads v1)**
- Extension: **.zhip**
- Maximum combined input: **400 MiB**
