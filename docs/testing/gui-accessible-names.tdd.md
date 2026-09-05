# TDD 記録: 画面の読み上げ対応（AccessibleName）

日付: 2026-09-05
ブランチ: `test/gui-accessible-names`
対象: `gui/App.cs`（`FilterForm`）

## 出典

計画ファイル（`*.plan.md`）は使っていない。このセッション中の調査から導いた。
`Verify.exe` が UI を経由せずリフレクションで private メンバーを叩いていること、
`gui/App.cs` に `AccessibleName` が 1 つも設定されていないことが発端。

## 利用者の筋道（User Journey）

> スクリーンリーダー利用者として、画面の各操作部品が安定した名前を読み上げてほしい。
> 目で見なくてもツールを操作できるようにするため。

表示文字列ではなく `AccessibleName` を検査対象にした。表示文字列は見た目の都合で
変わりうるが、`AccessibleName` は支援技術と UI Automation に公開される識別子で、
安定していなければならない。

## テストランナー（Step 0 の解決結果）

このスキルが例示する `node scripts/setup-package-manager.js --detect` は使えない。
Node も `package.json` も存在しないため。実際の対応は次のとおり。

| スキルの記法 | Rust (`cli/`) | C# (`gui/`) |
|---|---|---|
| `<test>` | `cargo test --all-targets` | `gui\build-tests.cmd && Tests.exe` |
| `<test-watch>` | なし（cargo-watch 未導入） | なし |
| `<coverage>` | `cargo llvm-cov --all-targets --fail-under-lines 85` | **なし**（下記参照） |
| `<lint>` | `cargo fmt --check && cargo clippy --all-targets -- -D warnings` | なし |

今回の変更は C# のみ。`<test>` は C# 側を指す。

## 作業の記録

### 1. 検査を追加し、RED を確認

`gui/Tests.cs` に `FormAccessibilityTests` を追加。`FilterForm` を生成し、
操作できる部品（Button / TextBox / CheckBox / NumericUpDown）を再帰的に集めて
`AccessibleName` を検査する。対話部品の内部実装には降りない（`NumericUpDown` は
内部に自前の `TextBox` とボタンを持つため）。

置き場所は `Tests.exe`。**CI が実際に実行している唯一の C# テスト**であり、
画面を表示せず生成するだけなのでエンジンもモデルも要らないため。
これに伴い `gui/build-tests.cmd` が `App.cs` も取り込むようにし、`Main` が
2 つになるため `/main:TestRunner` を指定、WinForms のため `[STAThread]` を付けた。

```
> gui\build-tests.cmd && Tests.exe Form_

== FormAccessibilityTests ==
  PASS Form_ExposesEveryInteractiveControl
  FAIL Form_EveryInteractiveControlHasAnAccessibleName
       すべての操作部品に AccessibleName がある
       無い部品: TextBox(""), Button("WAVを選択"), NumericUpDown("100"),
       CheckBox("強めの除去（ポストフィルター）"), Button("ノイズを除去"),
       Button("中止"), Button("元の音声を再生"), Button("処理後を再生"),
       Button("名前を付けて保存"), Button("再生停止")
  PASS Form_AccessibleNamesAreUnique

成功 2 / 失敗 1
```

RED はコンパイルエラーではなく、意図した業務上の欠落によるもの。
見張り役の `Form_ExposesEveryInteractiveControl` が通っているため、
**空振りの合格ではない**ことが確認できている（10 個を正しく発見している）。

checkpoint: `afc44cf test: 画面の読み上げ対応を検査するテストを追加（現時点で失敗する）`

### 2. 実装し、GREEN を確認

`gui/App.cs` の 11 部品に `AccessibleName` を追加（宣言初期化子への追記のみ）。

```
> gui\build-tests.cmd && Tests.exe Form_

== FormAccessibilityTests ==
  PASS Form_ExposesEveryInteractiveControl
  PASS Form_EveryInteractiveControlHasAnAccessibleName
  PASS Form_AccessibleNamesAreUnique

成功 3 / 失敗 0
```

退行の確認:

```
> Tests.exe
成功 63 / 失敗 0        （従来の 60 件 + 新規 3 件）

> gui\build-verify.cmd
Built: ...\Verify.exe   （App.cs のもう一方の利用側もビルドできる）
```

checkpoint: `f21c9fc feat: 操作部品に AccessibleName を付け、画面を読み上げ対応にする`

### 3. リファクタリング

なし。実装は宣言初期化子への追記のみで、整理の余地がない。

## 保証していること

| # | 保証内容 | テスト | 種別 | 結果 | 根拠 |
|---|---|---|---|---|---|
| 1 | 画面が公開する操作部品はちょうど 10 個（探索の見張り） | `gui/Tests.cs:Form_ExposesEveryInteractiveControl` | 単体 | PASS | `Tests.exe Form_` |
| 2 | すべての操作部品が空でない `AccessibleName` を持つ | `gui/Tests.cs:Form_EveryInteractiveControlHasAnAccessibleName` | 単体 | PASS | `Tests.exe Form_` |
| 3 | `AccessibleName` が部品間で重複しない | `gui/Tests.cs:Form_AccessibleNamesAreUnique` | 単体 | PASS | `Tests.exe Form_` |
| 4 | `WaveData` の従来の振る舞いが変わっていない | `gui/Tests.cs`（60 件） | 単体 | PASS | `Tests.exe` = 63/0 |

## カバレッジと、意図的に埋めていない穴

**C# 側のカバレッジは測っていない。測る手段がない。** このプロジェクトは
Windows 同梱の `csc.exe` だけでビルドしており、.NET SDK も NuGet も使わない方針のため、
coverlet 等のカバレッジ計測器を導入できない。スキルが求める「80% 以上」を
C# 側で数値として示すことはできない。Rust 側は CI が `--fail-under-lines 85` で
担保しているが、**今回の変更は C# のみなので Rust の数値は再測定していない**。

意図的に残した穴:

- **UI とロジックの結線は依然として未検査。** 今回のテストは部品の属性を見るだけで、
  ボタンを押していない。`run.Click += …` を消しても 3 件とも通る。ここを埋めるには
  UI Automation（pywinauto 等）が必要で、第三者パッケージの導入を意味する。
- **`Verify.exe` は CI で実行されていない。** `ci.yml` の C# ジョブが走らせるのは
  `Tests.exe` だけ。今回追加した 3 件は `Tests.exe` にあるため CI で走るが、
  `Verify.exe` 側の統合テストは引き続き手動実行のみ。
- **`Label`・`ProgressBar` は検査対象外。** 利用者が操作する部品ではないため。
  ただし `status`（状態表示）には実装側で `AccessibleName` を付けてある。

## マージ時の注意

checkpoint コミットを squash する場合、上の RED / GREEN の記録を PR 本文か
squash コミット本文に写すこと。この文書はリポジトリに残るため、それ自体が索引になる。

## CI での未確認事項

`windows-latest` ランナー上で `FilterForm` の生成が通るかは**まだ確認していない**。
GitHub Actions の Windows ランナーにはデスクトップセッションがあるため通る見込みだが、
push して CI が回るまでは実証されていない。もし落ちる場合、原因は WinForms の
生成がヘッドレス環境で失敗することであり、その際は当該テストを
`Verify.exe` 側へ移すのが対処になる。
