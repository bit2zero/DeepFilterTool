<!-- Generated: 2026-09-05 | Files scanned: cli/src/wave.rs, cli/src/engine.rs, AudioCore.cs | Token estimate: ~800 -->

# 音声データの扱い

データベースはない。扱うデータは WAV ファイルとその中間表現だけ。

## データ構造

`Wave`（Rust `cli/src/wave.rs`）／ `WaveData`（C# `AudioCore.cs`）— 同じ形。

| 項目 | 型 | 意味 |
|---|---|---|
| `format` | u16 | 1 = PCM、3 = IEEE Float |
| `channels` | u16 | 1 または 2 |
| `rate` | u32 | 48000 固定 |
| `bits` | u16 | 16（PCM）または 32（Float） |
| `align` | u16 | 1フレームのバイト数 = channels × bits/8 |
| `data` | Vec\<u8\> | 生のサンプル列 |

`frames = data.len() / align`

## 受け入れ条件

読み取り時にすべて検査し、1つでも外れたら断る。

```
RIFF ヘッダー   先頭12バイト。riff+8 ≤ ファイル長
チャンク走査    next = pos+8+n+(n%2)。next > end または n > 512 MB は不正
fmt             16バイト以上
data            存在し、空でない
rate            48000
channels        1 または 2
format × bits   (1,16) または (3,32)
align           channels × bits/8 と一致
data 長         align の倍数（端数フレームを持たない）
```

## 変換の流れ

```
入力（PCM16 か Float32）
  │
  │ ① パディング              write(staged, padded, pad=true)
  │    padded = ceil(frames/480)×480 + 4800
  │    追加分はゼロ（無音）
  ▼
staged.wav ──▶ 公式エンジン ──▶ filtered.wav
  │                                │
  │  エンジンは -D（遅延補正）付き。│
  │  先読み分を吐き出させるために  │
  │  末尾パディングが要る。        │
  │                                ▼
  │ ② PCM16化                 convert_to_pcm16()
  │    Float32 のみ実行。NaN/Inf は拒否。
  │    clamp(-1.0, 1.0) × 32767、最近接偶数丸め
  │
  │ ③ 切り詰め                write(clean, frames, pad=false)
  ▼
clean.wav（元と同じフレーム数・チャンネル数・48 kHz・PCM16）
```

### 定数

| 名前 | 値 | 理由 |
|---|---|---|
| `HOP` | 480 | モデルのホップ長。入力をこの境界へ切り上げる |
| `TAIL_PAD` | 4800 | 先読みと端数を吐き出させる余白（0.1秒） |

実測では 48001 フレームの入力が 53280 へパディングされ、エンジンが 51840 を返し、48001 へ切り詰められる。

## 書き出しの規則

- **既存ファイルを上書きしない**（`create_new` / `FileMode.CreateNew`）。上書きは呼び出し側が明示的に消してから行う。
- ヘッダーは 44 バイト固定（RIFF 12 + fmt 24 + data 8）。
- 4 GB を超える場合と掛け算があふれる場合は書き出す前に断る。
- 無音埋めは 8 KiB 単位で書く。

## 丸めを揃えている理由

C#の `Math.Round` は最近接偶数丸め。Rust側も `round_ties_even()` を使う。これにより、同じ入力に対して**両実装のパディング済み中間ファイルがバイト完全一致**する（実測で確認済み）。

## 効果の測定

`cli/tests/noise_reduction.rs` が、雑音なし音声（clean）と雑音入り音声（noisy）の対から次を求める。

```
SNR      = 10·log10( Σclean² / Σ(signal−clean)² )
ノイズ床  = clean が微小な区間での signal の RMS
時間ずれ  = 相互相関が最大になるオフセット（0 であるべき）
```

測定が成り立つには `noisy = clean + 雑音` で長さと時間位置が一致している必要がある。テストはこれを検査する。
