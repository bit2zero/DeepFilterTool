# DeepFilter 音声フィルターツール

Windows向けの日本語デスクトップアプリ。DeepFilterNet3の公式ONNXモデルを公式Rust CLIで実行する構成です。

**利用可能です。`DeepFilterTool.exe` をダブルクリックして起動してください。公式DeepFilterNet3モデルを導入し、実エンジンでの音声処理と画面表示を検証済みです。**

## 機能

- 48 kHz、モノラルまたはステレオのWAVを入力。PCM 16bit / IEEE Float 32bit対応。
- 最大ノイズ抑制を1〜100 dBで調整。100 dBは実質制限なし。
- ポストフィルター、中止、処理前後の再生、別名保存。
- 出力は再生互換性のため48 kHz PCM 16bit。Float入力を元音声として再生できない場合があります。
- 元音声を保持。末尾に無音を追加して遅延補正後に元のフレーム数へ整えます。
- MP3、動画、マイクのリアルタイム処理は対象外です。大容量ファイルはメモリ使用量が増えるため、短い音声から試してください。

## 導入済みの公式ファイル

保存先はこのフォルダー内の `runtime` のみです。

| 保存名 | 公式配布元・固定版 |
|---|---|
| deep-filter.exe | [v0.5.6 Windows x64 / 約25.7 MB](https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-x86_64-pc-windows-msvc.exe) |
| DeepFilterNet3_onnx.tar.gz | [v0.5.6に含まれる標準DeepFilterNet3](https://raw.githubusercontent.com/Rikorose/DeepFilterNet/v0.5.6/models/DeepFilterNet3_onnx.tar.gz) |

2026-09-05に公式GitHub APIでも最新安定版v0.5.6を確認しました。[標準モデルの履歴](https://github.com/Rikorose/DeepFilterNet/commits/main/models/DeepFilterNet3_onnx.tar.gz)の最終更新は2023-05-23、84d57ecです。導入した固定版モデルと公式mainブランチのモデルのSHA-256一致を確認済みです。取得URL・サイズ・SHA-256は `runtime/manifest.json` に記録しています。

ライセンスは公式リポジトリのMITまたはApache-2.0。公式MITライセンス本文を `runtime/LICENSE-MIT.txt` に同梱しています。再配布時にも保持してください。

導入時の通信先はgithub.com、raw.githubusercontent.comおよびGitHubのダウンロード配信先。音声処理はローカルです。管理者権限、PATH変更、サービス、フック、MCP、常駐処理は必要ありません。撤回時は追加したruntimeファイルのみを削除できます。

## ビルド

実行にビルドは不要です。再ビルド用の `Build.cmd` はWindows標準の .NET Framework C# コンパイラを直接使用します。外部パッケージは使いません。既存EXEがある場合は上書きを拒否します。

旧 `Build.ps1` は実行ポリシーで拒否されたため使用していません。ユーザー承認後、標準コンパイラの直接実行でEXE生成に成功しました。実行ポリシー・OS設定の変更はしていません。

## 利用手順

1. DeepFilterTool.exeを開く。
2. WAVを選択し、必要なら抑制上限とポストフィルターを調整。
3. 「ノイズを除去」を押す。
4. 元音声と処理後を再生して確認し、「名前を付けて保存」。

処理ファイルとエンジンログはアプリ隣の `sessions/日時-ID` に保存します。入力の作業用コピーも残るため、プライベートな音声を扱う場合はこのフォルダーも同様に管理してください。アプリは自動削除しません。

## 検証記録

- 実行して成功: AudioCore.csのコンパイル、WAV読み書き、日本語パス、無音パディング、元フレーム数への切り詰め、既存ファイル上書き拒否、不正サンプルレート拒否、壊れたヘッダー拒否、Float→PCM変換。
- 実行して成功: GUIのコンパイルと表示、公式CLIバージョン確認、公式現行モデルとのハッシュ一致。
- 実行して成功: Verify.csによる実エンジン統合テスト。日本語と空白を含むパス、48001フレームのモノラル/ステレオ入力、モデルによる波形変化、長さ・チャンネル数・サンプルレート保持、PCM16出力、元ファイル保持、結果ボタンの有効化、中止時の結果非公開を確認しました。
- 画面確認: `verification/20260905-165011/app-preview.png` を目視確認しました。
- 過去の失敗と解決: Windows PowerShellでのビルド拒否→承認後に標準コンパイラを直接使用。Windows TLS認証エラー→既存Pythonの標準TLS検証を有効にしたHTTPS取得で成功。
- 未確認: 人の声を含む実録音での聴感評価、スピーカーからの再生、保存ダイアログの手動操作。統合テストは合成音声を使用しています。

`Verify.cs` は検証用ソース、`Verify.exe` は検証用プログラムです。実行すると合成WAVを作り、画面を一時表示し、ローカルモデルを実行します。音声の外部送信や既存ファイルの削除は行いません。
