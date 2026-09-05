# 開発の手引き

このリポジトリには2つの実装があります。目的の側だけ用意すれば作業を始められます。

| | 場所 | 必要なもの |
|---|---|---|
| クロスプラットフォームCLI（Rust） | `cli/` | Rust 1.77 以降 |
| Windows GUI（C#） | リポジトリ直下 | Windows 同梱の .NET Framework のみ |

## 守っている方針

この3つはCIが機械的に検査します。破ると `supply-chain` ジョブが落ちます。

1. **外部パッケージを使わない。** Rust は標準ライブラリのみ、C# は Windows 同梱の .NET Framework のみ。`cli/Cargo.lock` のパッケージ数は常に1（自分自身）です。
2. **公式配布物は版・サイズ・SHA-256 で固定する。** 取得元は `cli/src/assets.rs` が唯一の出典で、`runtime/manifest.json` はそこから生成されます。
3. **GitHub Actions は40桁のコミットSHAで固定する。** タグは書き換え可能なため使いません。

## 環境の用意

### Rust側

```bash
rustup toolchain install stable --profile minimal --component clippy,rustfmt
```

Linuxで**システムのCコンパイラを入れられない環境**（`gcc` や `build-essential` が使えない場合）は、muslターゲットを追加してください。rustup同梱のリンカとCRTだけで完全静的バイナリを作れます。

```bash
rustup target add x86_64-unknown-linux-musl
```

`cli/build.sh` はこのターゲットが入っていれば自動的に選びます。

### C#側（Windowsのみ）

追加インストールは不要です。`%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe` を直接使います。.NET SDK も NuGet も使いません。

### 公式エンジンとモデル

テストの大半は実エンジンを起動します。最初に一度だけ導入してください。

```bash
cd cli
cargo run --release -- setup
cargo run --release -- check
```

未導入でもテストは失敗せず、理由を表示して該当項目を飛ばします。

<!-- AUTO-GENERATED: ビルドスクリプトと .github/workflows/ci.yml から生成。手で編集しないでください -->

## コマンド一覧

### Rust（`cli/` で実行）

| コマンド | 内容 |
|---|---|
| `cargo build --release` | リリースビルド |
| `./build.sh` | ビルド。Linuxではmuslターゲットがあれば自動選択 |
| `build.cmd` | Windowsでのビルド |
| `cargo fmt --check` | 書式検査。CIと同じ |
| `cargo clippy --all-targets -- -D warnings` | 静的解析。警告を1件も許さない |
| `cargo test --all-targets` | 全テスト |
| `cargo llvm-cov --all-targets --fail-under-lines 85` | カバレッジ計測。下限85% |
| `cargo run --release -- setup` | 公式エンジンとモデルの導入 |
| `cargo run --release -- check` | 導入状態の確認 |

### C#（リポジトリ直下で実行）

| コマンド | 生成物 | 内容 |
|---|---|---|
| `Build.cmd` | `DeepFilterTool.exe` | GUI本体。既存EXEがあると上書きを拒否します |
| `Build-Tests.cmd` | `Tests.exe` | `WaveData` の単体テスト |
| `Build-Verify.cmd` | `Verify.exe` | 実エンジンとGUIを通す統合テスト |

`Tests.exe` は引数に文字列を渡すと名前で絞り込めます（例: `Tests.exe Read_`）。

## 環境変数

`cli/src` と `cli/tests` の実際の参照箇所から抽出しています。

### 実行時

| 変数 | 必須 | 内容 | 例 |
|---|---|---|---|
| `DEEPFILTER_HOME` | いいえ | `runtime/` と `sessions/` を置く場所。既定は実行ファイルの位置から4階層上まで探索（**カレントディレクトリは意図的に含めません**） | `/opt/deepfilter` |
| `DEEPFILTER_DEBUG` | いいえ | 空でも `0` でもない値なら詳細ログを有効化。`--debug` と同じ | `1` |

### テスト時

| 変数 | 必須 | 内容 | 例 |
|---|---|---|---|
| `DEEPFILTER_NETWORK_TESTS` | いいえ | 設定すると、公式配布物を実際に取得する検査も走ります。既定では飛ばします | `1` |
| `DEEPFILTER_CLEAN` | いいえ | 効果測定に使う雑音なし音声。既定は `samples/clean.wav` | `/path/to/clean.wav` |
| `DEEPFILTER_NOISY` | いいえ | 効果測定に使う雑音入り音声。既定は `samples/noisy.wav` | `/path/to/noisy.wav` |

`PATH` と `PATHEXT` は取得ツール（curl / wget / powershell）の探索に読み取るだけで、設定は不要です。

<!-- /AUTO-GENERATED -->

## テスト

### 構成

| 場所 | 対象 |
|---|---|
| `cli/src/*_tests.rs` | 単体テスト。`#[cfg(test)] #[path = "..."] mod tests;` で本体に接続 |
| `cli/tests/*.rs` | 統合テスト。実際に本体を起動する |
| `cli/tests/common/mod.rs` | 統合テスト共通の道具。各ファイルはここから取り込む |
| `Tests.cs` | C#の単体テスト。xUnit相当の仕組みを内部に自作 |

### 走らせ方

```bash
cd cli
cargo test                                   # 全部
cargo test --test filenames                  # 1ファイルだけ
cargo test wave::                            # 名前で絞り込み
cargo test -- --nocapture                    # println! を見る
DEEPFILTER_NETWORK_TESTS=1 cargo test        # 通信を伴う検査も含める
```

### 書くときの約束

- **振る舞いを検査する。** 実装の内部ではなく、外から見える結果を確かめます。
- **名前で意図が分かるようにする。** `メソッド_期待する結果_条件` の形（例: `Read_Throws_WhenSampleRateIsNot48kHz`）。
- **前提が成り立たない環境では飛ばす。** エンジン未導入、参照音声なし、ファイルシステムの制約などは、失敗ではなく理由を表示して飛ばします。
- **並列実行を壊さない。** テストは同時に走ります。作業フォルダーはテストごとに分け、環境変数をプロセス全体で書き換えないでください。
- **パスを文字列にしない。** `to_str()` を通すとUTF-8でないファイル名で落ちます。`&Path` / `OsStr` のまま渡してください（`cli/tests/common/mod.rs` の `filter_paths` を参照）。

## 提出前の確認

CIと同じ内容をローカルで通してから出してください。

```bash
cd cli
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Windowsで C# を触った場合は追加で:

```
Build-Tests.cmd && Tests.exe
```

- [ ] `cargo fmt --check` が差分なし
- [ ] `cargo clippy -- -D warnings` が警告ゼロ
- [ ] テストがすべて通る
- [ ] 外部クレートを増やしていない（`Cargo.lock` のパッケージ数は1のまま）
- [ ] 公式配布物を変更した場合、`cargo run -- manifest > ../runtime/manifest.json` で再生成した
- [ ] 動作を変えた場合、README の該当箇所を直した
- [ ] 検証していないことを「検証済み」と書いていない

## CI

`.github/workflows/ci.yml` が push と pull request のたびに動きます。

| ジョブ | 内容 |
|---|---|
| 供給網の検査 | 依存ゼロの維持、取得元、アクションのSHA固定 |
| Rust | Ubuntu / Windows / macOS の3環境で fmt・clippy・エンジン導入・全テスト |
| カバレッジ | 下限85% |
| C# | Windows で `Tests.exe` |

`cargo-llvm-cov` はバージョンとSHA-256を固定して取得しています。更新する場合は `.github/workflows/ci.yml` の `LLVM_COV_VERSION` と `LLVM_COV_SHA256` を両方直してください。

## 困ったときは

| 症状 | 原因と対処 |
|---|---|
| `エンジンが見つかりません` | `cargo run --release -- setup` で導入する |
| `エンジンに実行権限がありません` | `chmod +x runtime/deep-filter` |
| `./build.sh: Permission denied` | `chmod +x cli/build.sh`。通常は実行ビット付きで取得されます |
| リンカが見つからずビルドできない（Linux） | `rustup target add x86_64-unknown-linux-musl` してから `./build.sh` |
| `48 kHz、モノラル/ステレオ…に対応しています` | 入力が対応形式ではありません。48 kHz へ変換してください |
| PowerShellで出力が文字化けする | Windows PowerShell 5.1 の既定コードページのためです。`[Console]::OutputEncoding = [Text.Encoding]::UTF8`、またはPowerShell 7以降を使ってください |
| うまくいかない理由が分からない | `--debug` を付けると、渡した引数・エンジンの起動行・終了状態・エンジン自身のログまで標準エラーに出ます。作業フォルダーも残ります |
