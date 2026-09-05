# DeepFilter Audio Filter Tool

[![CI](https://github.com/bit2zero/DeepFilterTool/actions/workflows/ci.yml/badge.svg)](https://github.com/bit2zero/DeepFilterTool/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**English** | [日本語](README.md)

> This is a translation of [README.md](README.md), which is the authoritative version. If the two disagree, the Japanese version is correct.

A tool that removes noise from recordings. It runs the official DeepFilterNet3 model through the official Rust CLI.

**All audio processing happens on your machine.** Your audio never leaves it. The only network access is a one-time download of the official engine.

## Which one to use

| | Platform | Interface | Status |
|---|---|---|---|
| **DeepFilterTool.exe** | Windows | Graphical | Verified against the real engine |
| **deepfilter-tool** | Windows / Linux / macOS | Command line | Verified by CI on all three |

Both perform the same processing. That the results match has been confirmed by measurement ([verification record](docs/VERIFICATION.md)).

## Supported files

- **48 kHz WAV, mono or stereo** (PCM 16-bit / IEEE Float 32-bit)
- Output is 48 kHz PCM 16-bit for playback compatibility
- **Your original file is never modified.** Input and output match exactly in length and timing
- MP3, video, and real-time microphone processing are out of scope

## Getting started

### Windows (graphical)

Double-click `DeepFilterTool.exe`, then:

1. Choose a WAV file
2. Press "ノイズを除去" (Remove noise)
3. Compare before and after, then press "名前を付けて保存" (Save as)

The interface is in Japanese.

### Command line (Windows / Linux / macOS)

Fetch the official engine and model once:

```bash
deepfilter-tool setup
```

Then just hand it a WAV file:

```bash
deepfilter-tool recording.wav
# → creates recording_clean.wav
```

| Common options | Meaning |
|---|---|
| `-o, --output <FILE>` | Where to write. Defaults to `<input>_clean.wav` |
| `-a, --attenuation <1-100>` | Maximum noise attenuation in dB. Defaults to 100 |
| `--pf` | Stronger removal (post-filter) |
| `--debug` | Detailed logging when something goes wrong |

Run `deepfilter-tool --help` for every option and usage examples. See [Command-line usage](docs/CLI.md) for details. Note that all program output is in Japanese.

## How well does it work

Measured on a real recording (6.7 seconds, **with the noise louder than the speech**).

| Metric | Before | After |
|---|---|---|
| SNR | -5.00 dB | **11.09 dB (+16.09 dB)** |
| Noise floor in silent passages | 3310.8 | **224.8 (-23.36 dB)** |
| Energy in speech passages | baseline | -0.82 dB (essentially preserved) |

**The noise is removed without eating into the voice.** Noise in silent passages drops by roughly 93% in amplitude while speech energy barely changes.

The audio is bundled in `samples/`, so you can reproduce this directly. For the method and a per-setting comparison, see [Measured effectiveness](docs/MEASUREMENT.md).

## Non-ASCII file names

Japanese (full-width) file and folder names work as-is, as do half-width katakana, emoji, spaces, punctuation, and both composed (NFC) and combining (NFD) forms of voiced marks. → [Details](docs/FILENAMES.md)

## Documentation

The documents below are written in Japanese.

| | Contents |
|---|---|
| [Command-line usage](docs/CLI.md) | Building, setup, every option, detailed logging |
| [Measured effectiveness](docs/MEASUREMENT.md) | SNR, noise floor, per-setting comparison |
| [File name encoding](docs/FILENAMES.md) | Behavior per OS, PowerShell caveats |
| [Security and supply chain](docs/SECURITY.md) | Pinned versions, network access, privacy |
| [Verification record](docs/VERIFICATION.md) | Test contents, coverage, what remains unverified |
| [Contributing](docs/CONTRIBUTING.md) | Environment setup, commands, environment variables, writing tests |
| [Codemaps](docs/CODEMAPS/architecture.md) | Overall structure, module dependencies, data flow |

## License

Copyright (c) 2026 bit2zero — **MIT License** ([LICENSE](LICENSE))

The noise removal itself is performed by DeepFilterNet (Copyright (c) 2021 Hendrik Schröter, MIT or Apache-2.0). This software only invokes it as a separate process; none of its code is vendored here. For third-party notices and redistribution conditions, see [NOTICE.md](NOTICE.md).

When redistributing, always include `LICENSE`, and also `runtime/LICENSE-MIT.txt` if you bundle the engine and model.
