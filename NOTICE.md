# 著作権とライセンスの表示

## このソフトウェア

Copyright (c) 2026 bit2zero

MIT ライセンスで提供します。全文は [LICENSE](LICENSE) を参照してください。

対象は、このリポジトリで作成したファイルです。

- `cli/` — クロスプラットフォームCLI（Rust）
- `gui/` — Windows GUI版（C#）とそのビルドスクリプト
- `README.md`、`NOTICE.md`、`runtime/manifest.json`

このソフトウェアは外部のパッケージに依存していません。RustのCLIは標準ライブラリのみで書かれており、C#版はWindows標準の.NET Frameworkのみを使います。

## 同梱・利用する第三者のソフトウェア

### DeepFilterNet

音声のノイズ除去そのものは、DeepFilterNet の公式配布物が行います。本ソフトウェアはそれを別プロセスとして呼び出すだけで、コードを取り込んではいません。

| | |
|---|---|
| 名称 | DeepFilterNet |
| 版 | v0.5.6（固定） |
| 配布元 | https://github.com/Rikorose/DeepFilterNet |
| 著作権 | Copyright (c) 2021 Hendrik Schröter |
| ライセンス | MIT または Apache-2.0（利用者が選択可） |
| 該当ファイル | `runtime/deep-filter`（Windowsは `deep-filter.exe`）、`runtime/DeepFilterNet3_onnx.tar.gz` |

MITライセンス本文を `runtime/LICENSE-MIT.txt` に同梱しています。**再配布する場合は、この本文を必ず保持してください。**

`runtime/` の中身は `deepfilter-tool setup` が公式配布元から取得します。取得元URL・サイズ・SHA-256は `runtime/manifest.json` に記録しており、配置前に照合します。

### モデルの学習データ

DeepFilterNet3 のモデルの学習に用いられたデータセットの扱いについては、上記の公式リポジトリの記載に従ってください。本リポジトリは学習データを含みません。

## 再配布するときに必要なこと

1. `LICENSE`（このソフトウェアのMITライセンス本文と著作権表示）を含めること。
2. `runtime/LICENSE-MIT.txt`（DeepFilterNetのMITライセンス本文と著作権表示）を含めること。エンジンやモデルを同梱する場合は必須です。
3. この `NOTICE.md` を含めることを推奨します。

いずれのライセンスも、著作権表示とライセンス本文の保持を条件としています。
