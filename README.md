# ZHiPManager

**Zone Hybrid Improvement Plan** — a responsive, browser-based archive creator and extractor for the native `.zhip` format.

## Features

- Responsive interface for desktop and mobile
- Drag-and-drop or multi-file selection
- Combined input limit of 400 MiB
- Custom archive name and compression level
- Create and extract native ZHIP v1 archives
- DEFLATE compression per file, with CRC-32 integrity checks
- Client-side processing; selected file contents are not uploaded to an application server

## ZHIP v1 binary format

This format is **not ZIP** and does not use ZIP container structures.

All integer fields are unsigned little-endian.

### Archive header (9 bytes)

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 | ASCII signature `ZHIP` |
| 4 | 1 | Format version (`1`) |
| 5 | 4 | Number of entries |

### Entry record

Each entry is stored sequentially as a 14-byte record header, UTF-8 path, then DEFLATE-compressed bytes.

| Field | Size | Meaning |
|---|---:|---|
| Path length | 2 bytes | UTF-8 path length |
| Original size | 4 bytes | Uncompressed byte count |
| Compressed size | 4 bytes | DEFLATE payload length |
| CRC-32 | 4 bytes | Checksum of original bytes |
| Path | Variable | UTF-8 relative path |
| Payload | Variable | Raw DEFLATE-compressed file contents |

The container uses the DEFLATE compression algorithm directly through `fflate.deflateSync` / `fflate.inflateSync`; it does not call ZIP archive functions. Only ZHiPManager's ZHIP v1 reader currently understands this container. ZIP files are not accepted by the native reader.

## Deployment

This is a static site. Deploy the repository to Vercel with no build command and output directory `.`, or serve `index.html` using GitHub Pages.

The compression library is loaded from jsDelivr, so an internet connection is needed to load it. Compression and extraction happen in the browser. Peak RAM use can be significantly higher than the input size, especially near the 400 MiB limit.

## Project

- Website: **ZHiPManager**
- Format: **ZHIP v1**
- Extension: **.zhip**
- Maximum combined input: **400 MiB**
