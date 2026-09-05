<!-- Generated: 2026-09-05 | Files scanned: Cargo.lock, Cargo.toml, assets.rs, ci.yml | Token estimate: ~600 -->

# 外部依存

## パッケージ依存: ゼロ

| 生態系 | 状態 |
|---|---|
| Rust / crates.io | **なし**。`cli/Cargo.lock` のパッケージ数は1（自分自身のみ） |
| .NET / NuGet | **なし**。Windows同梱の .NET Framework のみ |
| Node / npm | **なし**。`package.json` が存在しない |

CIの `supply-chain` ジョブが、`Cargo.lock` のパッケージ数が1でなくなった時点、または `[dependencies]` に項目が増えた時点で失敗する。

## 実行時に必要な外部プログラム

| プログラム | いつ | 必須 | 代替 |
|---|---|---|---|
| `deep-filter` | ノイズ除去のたび | はい | なし。これが処理の実体 |
| `curl` | `setup` のときだけ | いずれか1つ | `wget`、PowerShell |
| `xattr` | macOSの`setup`のみ | いいえ | なければ検疫属性を外さない |

`deep-filter` は `runtime/` に置き、別プロセスとして起動する。ライブラリとしてリンクはしていない。

## 取得する公式配布物

出典は `cli/src/assets.rs`。`runtime/manifest.json` はここから生成する（`deepfilter-tool manifest` が同じ内容を出す）。

```
DeepFilterNet v0.5.6
  github.com/Rikorose/DeepFilterNet
  Copyright (c) 2021 Hendrik Schröter / MIT または Apache-2.0

共通
  DeepFilterNet3_onnx.tar.gz    7,983,136 B   raw.githubusercontent.com
  LICENSE-MIT.txt                   1,083 B   raw.githubusercontent.com

エンジン（プラットフォーム別、いずれか1つ）
  windows-x86_64   deep-filter.exe   26,912,256 B
  linux-x86_64     deep-filter       36,417,296 B   musl（完全静的）
  linux-aarch64    deep-filter       39,238,496 B   gnu
  macos-x86_64     deep-filter       29,933,512 B
  macos-aarch64    deep-filter       27,877,081 B
```

いずれも**版・URL・バイト数・SHA-256を固定**し、`runtime/` へ置く前に照合する。一致しなければ破棄する。

## 通信先

| 宛先 | いつ |
|---|---|
| `github.com` | `setup` でのエンジン取得 |
| `raw.githubusercontent.com` | `setup` でのモデル・ライセンス取得 |

これ以外への通信はない。音声処理はすべてローカル。取得前にURLのスキームを検査し、HTTPS以外は取得しない。

## CI で使う外部のもの

| 対象 | 固定方法 |
|---|---|
| `actions/checkout` | コミットSHA `3d3c42e5…`（v7.0.1） |
| `cargo-llvm-cov` | バージョン 0.9.0 + SHA-256 照合 |
| rustup / rustc | ランナー同梱のものを使用 |

第三者製アクションは使っていない。`actions/checkout` のみ。CIの `supply-chain` ジョブがタグ参照を見つけると失敗する。

## ビルドに必要なもの

| 対象 | 必要なもの |
|---|---|
| Rust（一般） | Rust 1.77以降 |
| Rust（Linux・Cコンパイラなし） | 加えて `rustup target add x86_64-unknown-linux-musl` |
| C# | Windows同梱の `csc.exe`（`%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\`） |
| カバレッジ | `cargo-llvm-cov` |
