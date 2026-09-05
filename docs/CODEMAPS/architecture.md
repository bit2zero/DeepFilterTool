<!-- Generated: 2026-09-05 | Files scanned: 42 | Token estimate: ~750 -->

# 全体構成

同じ処理を行う実装が2つある。どちらも**音声処理そのものは行わず**、DeepFilterNet の公式CLIを別プロセスとして起動する。

```
利用者
  ├─ DeepFilterTool.exe ── C# / WinForms ── Windows のみ
  └─ deepfilter-tool ───── Rust / CLI ───── Windows・Linux・macOS
                    │
                    ├── runtime/deep-filter        公式エンジン（別プロセス）
                    └── runtime/DeepFilterNet3_onnx.tar.gz   公式モデル
```

## 処理の流れ（両実装で共通）

```
入力.wav
  │ 読み取り・検証（48 kHz / 1-2ch / PCM16 か Float32）
  ▼
sessions/日時-ID/input.wav        末尾に無音を足す
  │                                frames→480境界へ切上げ + 4800
  ▼ 別プロセス起動
deep-filter -m <model> -D -a <dB> [--pf] -o <out> <in>
  │
  ▼
sessions/日時-ID/filtered/input.wav
  │ Float32 なら PCM16 へ変換 → 元のフレーム数へ切り詰め
  ▼
sessions/日時-ID/clean.wav → 出力先へ移動（不可ならコピー）
```

末尾のパディングは、モデルの先読み（lookahead）と端数ホップを吐き出させるために必要。`-D`（遅延補正）と組み合わせることで、入出力の長さと時間位置が完全に一致する。

## 境界

| 境界 | やり取り |
|---|---|
| 本体 → 公式エンジン | コマンドライン引数のみ。**入力は常に `input.wav` という固定名**で渡すため、利用者のファイル名の文字コードがエンジンに影響しない |
| 本体 → ネットワーク | `setup` サブコマンドのみ。curl / wget / PowerShell を起動して HTTPS で取得 |
| 本体 → ディスク | `runtime/`（読み取り）、`sessions/`（作業）、出力先 |

## ディレクトリ

| 場所 | 中身 |
|---|---|
| `cli/src/` | Rust本体。8モジュール、1726行 |
| `cli/tests/` | 統合テスト。実際に本体を起動する |
| リポジトリ直下 `*.cs` | C#実装。本体175行、テスト771行 |
| `runtime/` | 公式エンジンとモデル。`manifest.json` 以外は追跡しない |
| `sessions/` | 作業フォルダー。CLIは既定で削除、GUIは常に残す |
| `samples/` | 効果測定用の音声（clean / noisy の対） |

## 実装が2つある理由

C#版が先にあり、Windows向けGUIとして完成・検証済み。Rust版はLinux/macOS対応のために追加した。**両者は独立**で、コードを共有しない。

同じ結果になることは実測で確認している。パディング済み中間ファイルはバイト完全一致、最終出力は最大1 LSB差（エンジンのプラットフォーム別ビルドにおける浮動小数点演算の差）。

## 関連

- CLIの内部: [cli.md](cli.md)
- GUIの内部: [gui.md](gui.md)
- 音声データの扱い: [audio-pipeline.md](audio-pipeline.md)
- 外部依存: [dependencies.md](dependencies.md)
