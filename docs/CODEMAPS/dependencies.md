<!-- Generated: 2026-09-05 | Files scanned: Cargo.lock, Cargo.toml, assets.rs, ci.yml, readme-translate.yml, translate-readme.sh | Token estimate: ~700 -->

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
| `jq` | README翻訳のときだけ | 保守時のみ | なし。JSON組み立てに使う |

`deep-filter` は `runtime/` に置き、別プロセスとして起動する。ライブラリとしてリンクはしていない。

`jq` は製品側では使わない。`.github/scripts/translate-readme.sh` 専用で、ビルドにもテストにも実行にも不要。

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

| 宛先 | いつ | どこから |
|---|---|---|
| `github.com` | `setup` でのエンジン取得 | 製品 |
| `raw.githubusercontent.com` | `setup` でのモデル・ライセンス取得 | 製品 |
| `api.anthropic.com` | `README.en.md` の再生成 | **保守ツールのみ** |

**製品が通信するのは上2つだけ**で、しかも `setup` のときに限られる。音声処理はすべてローカル。取得前にURLのスキームを検査し、HTTPS以外は取得しない。

`api.anthropic.com` は `.github/scripts/translate-readme.sh` からの呼び出しで、**本プロジェクトで唯一、外部サービスに依存する箇所**。利用者の実行経路には存在せず、`ANTHROPIC_API_KEY` が未設定なら何もせず素通りする。ビルドとテストには一切関係しないため、`ci.yml` とは別のワークフローに分けてある。

## CI で使う外部のもの

| 対象 | 固定方法 |
|---|---|
| `actions/checkout` | コミットSHA `3d3c42e5…`（v7.0.1）。`ci.yml`・`readme-translate.yml` の両方で同じSHA |
| `cargo-llvm-cov` | バージョン 0.9.0 + SHA-256 照合 |
| rustup / rustc | ランナー同梱のものを使用 |

第三者製アクションは使っていない。`actions/checkout` のみ。CIの `supply-chain` ジョブがタグ参照を見つけると失敗する。

## Secrets

| 名前 | 用途 | 未設定のとき |
|---|---|---|
| `ANTHROPIC_API_KEY` | `README.en.md` の自動再生成 | 警告だけ出して素通りする。CIは壊れず、pull request での同期検査も動き続ける |

ビルドとテストに必要な Secrets はない。フォークからの pull request でも CI は完走する。

## ビルドに必要なもの

| 対象 | 必要なもの |
|---|---|
| Rust（一般） | Rust 1.77以降 |
| Rust（Linux・Cコンパイラなし） | 加えて `rustup target add x86_64-unknown-linux-musl` |
| C# | Windows同梱の `csc.exe`（`%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\`） |
| カバレッジ | `cargo-llvm-cov` |
