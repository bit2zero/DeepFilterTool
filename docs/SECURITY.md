# セキュリティと供給網対策

## 公式ファイルの固定版

保存先はこのフォルダー内の `runtime` のみです。取得URL・サイズ・SHA-256は `runtime/manifest.json` に記録しています。この内容は `deepfilter-tool manifest` の出力と同一で、CLIのソース (`cli/src/assets.rs`) が唯一の出典です。

| 対象 | ファイル |
|---|---|
| モデル（全環境共通） | [DeepFilterNet3_onnx.tar.gz (v0.5.6)](https://raw.githubusercontent.com/Rikorose/DeepFilterNet/v0.5.6/models/DeepFilterNet3_onnx.tar.gz) |
| Windows x86_64 | deep-filter-0.5.6-x86_64-pc-windows-msvc.exe |
| Linux x86_64 | deep-filter-0.5.6-x86_64-unknown-linux-musl |
| Linux aarch64 | deep-filter-0.5.6-aarch64-unknown-linux-gnu |
| macOS x86_64 | deep-filter-0.5.6-x86_64-apple-darwin |
| macOS aarch64 | deep-filter-0.5.6-aarch64-apple-darwin |

2026-09-05に公式GitHub APIで最新安定版v0.5.6と各配布物を確認しました。[標準モデルの履歴](https://github.com/Rikorose/DeepFilterNet/commits/main/models/DeepFilterNet3_onnx.tar.gz)の最終更新は2023-05-23、84d57ecです。導入した固定版モデルと公式mainブランチのモデルのSHA-256一致を確認済みです。

DeepFilterNet のライセンスは公式リポジトリのMITまたはApache-2.0。公式MITライセンス本文を `runtime/LICENSE-MIT.txt` に同梱しています。再配布時にも保持してください。

導入時の通信先はgithub.com、raw.githubusercontent.comおよびGitHubのダウンロード配信先。管理者権限、PATH変更、サービス、フック、MCP、常駐処理は必要ありません。撤回時は追加したruntimeファイルのみを削除できます。


## 供給網（サプライチェーン）対策

このプロジェクトには **npm も NuGet も存在せず、Rust の外部クレートもゼロ**です（`cli/Cargo.lock` のパッケージ数は1＝自分自身のみ）。したがってパッケージレジストリ経由の攻撃面がありません。実際の供給網は「公式配布物の取得」と「CIで使うGitHub Actions」の2つで、それぞれに対策しています。

**1. 公式配布物（DeepFilterNetのエンジンとモデル）**

- 版・取得元URL・バイト数・SHA-256を `cli/src/assets.rs` に固定。`runtime/manifest.json` はここから生成されます。
- 取得前にURLのスキームを検査し、**HTTPS以外は取得しません**。curl / wget にもプロトコルとTLSの下限を明示します。
- **配置前に必ずサイズとSHA-256を照合**し、一致しないものは破棄して `runtime/` に置きません。差し替えられた配布物を掴んでも実行されません。
- 取得ツールは絶対パスに解決してから起動し、同名プログラムの取り違えを防ぎます。

**2. GitHub Actions**

- 使用するアクションは**すべて40桁のコミットSHAで固定**します。タグは書き換え可能なため使いません。CIの `supply-chain` ジョブがタグ参照を見つけると失敗します。
- 第三者製アクションは使っていません（`actions/checkout` のみ）。ツールチェーンはランナー同梱の `rustup` を直接呼びます。
- CIで取得する `cargo-llvm-cov` はバージョンとSHA-256を固定し、照合してから使います。
- ワークフローの既定権限は `contents: read`、チェックアウトは `persist-credentials: false`。

**3. 依存ゼロの維持**

CIの `supply-chain` ジョブが、`Cargo.lock` のパッケージ数が1でなくなった時点、または `[dependencies]` に項目が増えた時点で失敗します。うっかり依存が入ることを仕組みで防ぎます。

> ご提案いただいた [setup-takumi-guard-npm](https://github.com/flatt-security/setup-takumi-guard-npm) は npm / pnpm / Yarn 専用で、公式に「Rust/Cargo、.NET/NuGet、任意のファイルダウンロードは対象外」と明記されています。本リポジトリには npm パッケージが1つもないため導入しても保護対象がなく、代わりに上記の対策を実装しました。将来 Node.js を使う部分を追加する場合は、そのワークフローに導入する価値があります。

## 本体の作りにおける対策

- **エンジンの探索にカレントディレクトリを使いません。** 実行ファイルの位置と `DEEPFILTER_HOME` のみです。
- **取得は HTTPS のみ。** 取得前にURLのスキームを検査し、curl / wget にもプロトコルとTLSの下限を明示します。取得ツールは絶対パスに解決してから起動します。
- **配置前に必ずサイズと SHA-256 を照合します。** 一致しないものは破棄し、`runtime/` には置きません。
- 外部クレートに依存しないため、第三者パッケージ経由の供給網リスクがありません（`Cargo.lock` のパッケージ数は1）。
- `unsafe` ブロックはありません。本番の処理経路に `unwrap` / `expect` / `panic!` はありません。
- 認証情報・トークンの類は扱いません。

既知の残存リスク:

- `runtime/` に書き込める者は、照合の直後から配置までのごく短い隙にファイルを差し替えうる（TOCTOU）。ただしその権限があれば配置後に直接置き換えることもできるため、実質的な追加リスクはありません。`runtime/` の書き込み権限は利用者本人に限ってください。
- `--debug` は絶対パスを標準エラーに出します。ログを共有する場合はご注意ください。

## プライバシー

処理ファイルとエンジンログは `sessions/日時-ID` に保存されます。入力の作業用コピーも含まれるため、プライベートな音声を扱う場合はこのフォルダーも同様に管理してください。GUI版は自動削除しません。CLI版は成功時に既定で削除します。

---


## 依存の脆弱性チェック

最終確認日: 2026-09-05

| 対象 | 結果 |
|---|---|
| Rust クレート | **0 件**（`Cargo.lock` のパッケージ数 1 = 自分自身のみ）。RustSec / cargo-audit の検査対象がない |
| npm パッケージ | **なし**（`package.json` が存在しない） |
| NuGet パッケージ | **なし**（Windows 同梱の .NET Framework のみ） |
| `unsafe` ブロック | **0 件** |
| DeepFilterNet v0.5.6 | 最新版。公開セキュリティ勧告 **0 件**（GitHub Advisory DB を `deepfilternet` / `deep-filter` で横断検索） |
| `actions/checkout` | 最新の v7.0.1 に相当するコミット SHA で固定 |
| Rust ツールチェーン | CI は stable（1.98.1）。標準ライブラリに該当する勧告なし |

**パッケージレジストリ経由の攻撃面がありません。** 依存が増えれば CI の `supply-chain` ジョブが失敗するため、この状態は仕組みで維持されます。

### プロセス起動に関する確認

Windows の標準ライブラリでは、バッチファイル（`.bat` / `.cmd`）を起動する際の引数エスケープに関する脆弱性が過去に報告されています。本ソフトウェアが起動するのは以下だけで、**バッチファイルは起動しません**。

| 起動対象 | 解決方法 |
|---|---|
| `deep-filter` | `runtime/` 配下の実行ファイル |
| `curl` / `wget` / `powershell` | `which` で解決した絶対パス |
| `xattr`（macOS のみ） | 同上 |

引数はシェルを介さず個別の値として渡します。唯一の例外である PowerShell 取得経路は、単引用符を二重化してエスケープします。

### 定期的に確認すべきこと

- DeepFilterNet に新しい版が出ていないか（`cli/src/assets.rs` の固定情報を更新し、`deepfilter-tool manifest > runtime/manifest.json` で再生成する）
- `actions/checkout` の新しい版が出ていないか（コミット SHA で固定し直す）
- GitHub のシークレットスキャンのアラート
