# ZHiPManager

**Zone Hybrid Improvement Plan** — a responsive, browser-based archive creator for `.zhip` files.

## Features

- Responsive dark interface for desktop and mobile
- Drag and drop or select multiple files
- Combined input limit of 400 MB
- Archive naming and compression levels
- Client-side compression; selected file contents are not uploaded to an application server
- ZIP payload prefixed by the ZHIP signature `ZHIP\x01\x00`

## Deployment

This is a static site. Deploy the repository to Vercel with no build command and no output directory (or set the output directory to `.`). GitHub Pages can also serve `index.html`.

## Format note

The current first version writes a 6-byte ZHIP signature followed by ZIP-compatible data and uses the `.zhip` extension. This is a lightweight custom container wrapper, not yet a fully independent compression format. Generic ZIP utilities may not open the file directly because of the prefix; ZHiPManager's future reader should validate the signature and decode the ZIP payload after the header.

The compression library (fflate) is loaded from jsDelivr. Opening the site requires an internet connection to fetch that library. Actual compression happens in the user's browser. Files are limited to 400 MiB combined, though memory use during compression can be higher than the selected file size.

## Project

- Website name: **ZHiPManager**
- Archive extension: **.zhip**
- Maximum combined input: **400 MiB**
