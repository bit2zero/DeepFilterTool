<!-- Generated: 2026-09-05 | Files scanned: 4 (*.cs) | Token estimate: ~700 -->

# GUI（C# / WinForms）の内部

Windows専用。.NET Framework（Windows同梱）のみで動く。外部パッケージなし、本体175行。

## ファイル

| ファイル | 行 | 役割 |
|---|---|---|
| `App.cs` | 111 | `FilterForm`。画面とエンジン起動 |
| `AudioCore.cs` | 64 | `WaveData`。RIFF/WAVの読み書きと変換 |
| `Tests.cs` | 714 | `WaveData` の単体テスト60件 + xUnit相当の枠組み |
| `Verify.cs` | 57 | 実エンジンとGUIを通す統合テスト |

`App.cs` と `AudioCore.cs` の関係は、CLI版の `main.rs`+`engine.rs` と `wave.rs` に対応する。

## 画面の構成

```
FilterForm （縦積み FlowLayoutPanel）
  ├ 見出し「音声を、もっとクリアに。」
  ├ TextBox file          選択中のWAV（読み取り専用）
  ├ Button  choose        「WAVを選択」  → OpenFileDialog
  ├ 設定行
  │   ├ NumericUpDown strength   最大ノイズ抑制 1-100 dB
  │   └ CheckBox      pf         強めの除去（ポストフィルター）
  ├ 操作行
  │   ├ Button run       「ノイズを除去」 → StartFilter()
  │   └ Button cancel    「中止」         → CancelFilter()
  ├ ProgressBar progress  処理中は Marquee
  ├ Label       status    状態と結果の表示
  └ 再生行
      ├ Button before    「元の音声を再生」
      ├ Button after     「処理後を再生」
      ├ Button save      「名前を付けて保存」 → SaveResult()
      └ Button stop      「再生停止」
```

## 状態の遷移

```
起動
  │ runtime/ を確認 → 未導入なら status に案内
  ▼
待機 ──choose──▶ ファイル選択済み
  │                    │ run
  │                    ▼
  │              StartFilter()
  │                WaveData.Read → 検証
  │                sessions/日時-ID/ を作る
  │                入力をパディングして input.wav へ
  │                Process.Start(deep-filter …)  非同期で stdout/stderr 読み取り
  │                Busy(true) / timer.Start()
  │                    │
  │                    ▼ 200ms ごと
  │              Poll()  ← System.Windows.Forms.Timer
  │                プロセス終了と両ストリーム完了を待つ
  │                engine.log を書く
  │                成功 → WaveData.Read → ToPcm16 → 元の長さへ切り詰め
  │                     → clean.wav、after/save を有効化
  │                中止 → 結果は公開しない
  ▼
完了 ──save──▶ SaveResult()（元ファイルと同名は拒否）
```

`Poll()` はタイマー駆動。UIスレッドを止めないため、`Process.WaitForExit()` は使わない。処理中に閉じようとすると `FormClosing` が拒否する。

## GUI版とCLI版の違い

| | GUI | CLI |
|---|---|---|
| 作業フォルダー | 常に残す | 既定で削除（`--keep-session` / `--debug` で保持） |
| 再生 | あり（`System.Media.SoundPlayer`） | なし |
| 中止 | 「中止」ボタン | なし |
| 引数のパス | .NET の string（UTF-16） | `OsString`（変換しない） |
| エンジン引数 | 文字列を組み立てて `ProcessStartInfo` に渡す | 個別の値として渡す |

## テスト

`Tests.exe`（`Build-Tests.cmd` で生成）が `WaveData` の60件を検査。エンジン不要で数十ミリ秒。

`Verify.exe`（`Build-Verify.cmd`）は実エンジンとGUIを通す統合テスト。リフレクションで `FilterForm` の私有メンバーを操作し、モノラル／ステレオ両方で以下を確認する。

- 長さ・チャンネル数・サンプルレートの保持
- PCM16での出力
- 元ファイルが変更されないこと
- モデルが波形を変えること
- 中止時に結果を公開しないこと
- 画面を `verification/*/app-preview.png` に保存
