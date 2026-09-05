<!-- Generated: 2026-09-05 | Files scanned: 8 (cli/src/*.rs 本体のみ) | Token estimate: ~900 -->

# CLI（Rust）の内部

外部クレートなし。標準ライブラリのみ、本体1726行。

## サブコマンドの流れ

```
main() → dispatch()                                  main.rs
  │  args_os() で受ける（UTF-8でない引数でも落ちない）
  │  --debug は位置に関係なく先に見る
  │
  ├─ -h/--help/help        → USAGE を表示
  ├─ -V/--version/version  → 版 + COPYRIGHT
  ├─ license/--license     → COPYRIGHT + ライセンス所在
  ├─ manifest              → assets::manifest_json()
  ├─ setup   → setup_command()  → setup::run()        setup.rs
  ├─ check   → check_command()  → engine::check_runtime()
  ├─ filter  → filter_command() ┐
  └─ （その他）→ filter_command() ┘ → engine::run()     engine.rs
```

## モジュール依存

```
main ──┬─→ assets   （固定版一覧。依存なし）
       ├─→ debug    （dlog! マクロ。依存なし）
       ├─→ error    （Error 型。依存なし）
       ├─→ setup ───┬─→ assets
       │            ├─→ sha256
       │            └─→ error
       └─→ engine ──┬─→ wave ─→ error
                    ├─→ debug
                    └─→ error
```

循環なし。`sha256` と `assets` は他に依存しない葉。

## 各モジュール

| ファイル | 行 | 役割 | 主な公開項目 |
|---|---|---|---|
| `main.rs` | 483 | 引数解析、サブコマンド振り分け、出力先の検証 | （すべて私有） |
| `engine.rs` | 344 | 公式エンジンの起動、パディング／切り詰め、作業フォルダー | `run(&Job) -> Outcome`、`find_root`、`check_runtime`、`platform_key` |
| `sha256.rs` | 230 | SHA-256（FIPS 180-4）自前実装 | `Sha256`、`file_hex`、`hex` |
| `setup.rs` | 231 | 公式配布物の取得・照合・配置 | `run(root, platform, force)` |
| `wave.rs` | 200 | RIFF/WAV の読み書き、Float32→PCM16 | `Wave::read/write/convert_to_pcm16` |
| `assets.rs` | 124 | 版・URL・サイズ・SHA-256 の固定表 | `SHARED`、`ENGINES`、`engine_for`、`manifest_json` |
| `debug.rs` | 72 | `--debug` の詳細ログ（標準エラーへ） | `enable`、`emit`、`emit_block` |
| `error.rs` | 42 | 日本語メッセージを持つ Error 型 | `Error`、`Result`、`Context` |

## ノイズ除去の実処理

```
filter_command()                                     main.rs
  引数解析 → check_output_name() → same_file() 判定
  → engine::check_runtime()   ← 作業フォルダーを作る前に確認
  → engine::new_session()
  → engine::run(&Job)                                engine.rs
       Wave::read(input)                             wave.rs
       → source.write(staged, padded, pad=true)
       → Command::new(engine).args(...).output()     別プロセス
       → Wave::read(filtered) → convert_to_pcm16()
       → filtered.write(clean, frames, pad=false)
  → 出力先へ rename、失敗ならコピー
  → セッション削除（--keep-session / --debug なら保持）
```

## 設計上の要点

- **パスを文字列にしない。** `args_os()` で受け、`OsString` / `&Path` のまま子プロセスへ渡す。`to_str()` を通すとUTF-8でないファイル名で落ちる。
- **カレントディレクトリを探索しない。** `find_root()` は `DEEPFILTER_HOME` か実行ファイルの位置のみ。cwd を含めると、細工した `runtime/deep-filter` を置いたフォルダーで実行するだけで任意コード実行になる。
- **シェルを介さない。** 引数は個別の値として渡す。唯一の例外はWindowsのPowerShell取得経路で、そこは `powershell_quote()` で単引用符を二重化する。
- **配置前に必ず照合。** `setup::install()` はサイズとSHA-256を確認してから実行ビットを立て、`runtime/` へ移す。

## テスト

| 場所 | 件数 | 対象 |
|---|---|---|
| `cli/src/*_tests.rs` | 89 | 各モジュールの単体。`#[path]` で本体に接続 |
| `cli/tests/engine_integration.rs` | 29 | 実エンジンでの処理、CLI表層 |
| `cli/tests/filenames.rs` | 10 | 文字コード、パスの書き方、出力名の検証 |
| `cli/tests/debug_log.rs` | 8 | `--debug` の出力内容 |
| `cli/tests/noise_reduction.rs` | 3 | 実録音での効果測定（SNR） |
| `cli/tests/common/mod.rs` | — | 統合テスト共通の道具 |
