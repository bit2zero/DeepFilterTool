# DeepFilter 音声フィルターツール

[![CI](https://github.com/bit2zero/DeepFilterTool/actions/workflows/ci.yml/badge.svg)](https://github.com/bit2zero/DeepFilterTool/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**日本語** | [English](README.en.md)

> このファイルが正本です。`README.en.md` は翻訳で、内容が食い違った場合はこちらが正しいものとします。

録音から雑音を取り除く日本語のツールです。DeepFilterNet3 の公式モデルを公式 Rust CLI で実行します。

**音声処理はすべて手元で完結します。** 音声がネットワークに出ることはありません。通信は最初の一度、公式エンジンを取得するときだけです。

## どちらを使うか

| | 対象 | 使い方 | 状態 |
|---|---|---|---|
| **DeepFilterTool.exe** | Windows | 画面で操作 | 実エンジンで検証済み |
| **deepfilter-tool** | Windows / Linux / macOS | コマンドライン | 3 OS の CI で検証済み |

どちらも同じ処理を行います。処理の中身が同じであることは実測で確認しています（[検証記録](docs/VERIFICATION.md)）。

## 扱えるファイル

- **48 kHz、モノラルまたはステレオの WAV**（PCM 16bit / IEEE Float 32bit）
- 出力は再生互換性のため 48 kHz PCM 16bit
- **元のファイルは変更しません**。入出力の長さと時間位置は完全に一致します
- MP3・動画・マイクのリアルタイム処理は対象外です

## 使ってみる

### Windows（画面で操作）

`DeepFilterTool.exe` をダブルクリックし、

1. WAV を選ぶ
2. 「ノイズを除去」を押す
3. 処理前後を聴き比べて「名前を付けて保存」

### コマンドライン（Windows / Linux / macOS）

最初に一度だけ、公式エンジンとモデルを取得します。

```bash
deepfilter-tool setup
```

あとは WAV を渡すだけです。

```bash
deepfilter-tool 会議の録音.wav
# → 会議の録音_clean.wav ができる
```

| よく使うオプション | 意味 |
|---|---|
| `-o, --output <ファイル>` | 出力先。既定は 入力名`_clean.wav` |
| `-a, --attenuation <1-100>` | 最大ノイズ抑制 dB。既定は 100 |
| `--pf` | 強めの除去 |
| `--debug` | うまくいかないときに詳細ログを出す |

全オプションと使用例は `deepfilter-tool --help` で表示されます。詳しくは [コマンドライン版の使い方](docs/CLI.md)。

## どのくらい効くか

実際の録音（6.7秒、**雑音のほうが音声より大きい状態**）での測定結果です。

| 指標 | 処理前 | 処理後 |
|---|---|---|
| SNR | -5.00 dB | **11.09 dB（+16.09 dB）** |
| 無音区間のノイズ床 | 3310.8 | **224.8（-23.36 dB）** |
| 声の区間のエネルギー | 基準 | -0.82 dB（ほぼ保持） |

**声を削らずに雑音だけを落とせています。** 無音部分の雑音は振幅で約93%落ち、声のエネルギーはほぼ変わりません。

音源は `samples/` に同梱しているのでそのまま再現できます。測定方法と設定ごとの比較は [効果の実測値](docs/MEASUREMENT.md)。

## 日本語ファイル名

日本語（全角）のファイル名・フォルダー名をそのまま扱えます。半角カタカナ、絵文字、空白や記号、濁点の合成形／結合文字も同様です。→ [詳細](docs/FILENAMES.md)

## ドキュメント

| | 内容 |
|---|---|
| [コマンドライン版の使い方](docs/CLI.md) | ビルド、導入、全オプション、詳細ログ |
| [効果の実測値](docs/MEASUREMENT.md) | SNR、ノイズ床、設定ごとの比較 |
| [ファイル名の文字コード](docs/FILENAMES.md) | 各OSでの扱い、PowerShell の注意 |
| [セキュリティと供給網対策](docs/SECURITY.md) | 固定版の管理、通信、プライバシー |
| [検証記録](docs/VERIFICATION.md) | テスト内容、カバレッジ、未確認事項 |
| [開発の手引き](docs/CONTRIBUTING.md) | 環境の用意、コマンド、環境変数、テストの書き方 |
| [コードマップ](docs/CODEMAPS/architecture.md) | 全体構成、モジュール依存、データの流れ |

## ライセンス

Copyright (c) 2026 bit2zero — **MIT ライセンス**（[LICENSE](LICENSE)）

ノイズ除去の実体は DeepFilterNet（Copyright (c) 2021 Hendrik Schröter、MIT または Apache-2.0）です。本ソフトウェアはそれを別プロセスとして呼び出すだけで、コードを取り込んではいません。第三者ソフトウェアの表示と再配布時の条件は [NOTICE.md](NOTICE.md) を参照してください。

再配布する場合は `LICENSE` と、エンジン・モデルを同梱するなら `runtime/LICENSE-MIT.txt` を必ず一緒に配布してください。
