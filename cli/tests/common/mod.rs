//! 統合テストで共通に使う道具。
//!
//! Rust の統合テストはファイルごとに別々の実行ファイルになるため、
//! ここを取り込まないと同じ補助関数を各ファイルに書くことになる。
//!
//! 各テストファイルはこのうち一部しか使わない。使わない項目に対する
//! 警告を止めるため、このモジュールだけ dead_code を許可している。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// ビルドされた本体の場所。cargo が渡してくる。
pub const BIN: &str = env!("CARGO_BIN_EXE_deepfilter-tool");

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli/ の親フォルダー")
        .to_path_buf()
}

pub fn engine_file() -> PathBuf {
    repo_root().join("runtime").join(if cfg!(windows) {
        "deep-filter.exe"
    } else {
        "deep-filter"
    })
}

pub fn model_file() -> PathBuf {
    repo_root()
        .join("runtime")
        .join("DeepFilterNet3_onnx.tar.gz")
}

pub fn runtime_ready() -> bool {
    engine_file().is_file() && model_file().is_file()
}

/// 公式エンジンが未導入なら理由を表示して true を返す。呼び出し側はそのまま return する。
pub fn skip_unless_ready() -> bool {
    if runtime_ready() {
        return false;
    }
    eprintln!(
        "スキップ: runtime/ に公式エンジンとモデルがありません。\
         `deepfilter-tool setup` で導入すると実エンジン検証を実行します。"
    );
    true
}

/// テストごとに使い捨てる一時フォルダー。prefix でテストファイルを見分ける。
pub fn work_dir(prefix: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deepfilter-{}-{}-{}",
        prefix,
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("作業フォルダー作成");
    dir
}

/// 決定的な擬似乱数ノイズと 180 Hz の正弦波を重ねた検査用 WAV を書く。
///
/// モデルが確実に何かを変える程度の雑音を含み、毎回同じ内容になる。
pub fn write_test_wav(path: &Path, channels: u16, frames: usize) {
    let align = channels as usize * 2;
    let mut data = vec![0u8; frames * align];
    let mut seed: u32 = 42;
    for frame in 0..frames {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 16) as i32 - 32_768;
        let tone =
            (2_500.0 * (frame as f64 * 2.0 * std::f64::consts::PI * 180.0 / 48_000.0).sin()) as i32;
        let value = ((noise * 1_800 / 32_768) + tone).clamp(-32_768, 32_767) as i16;
        for ch in 0..channels as usize {
            let at = frame * align + ch * 2;
            data[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    write_wav_bytes(path, channels, &data);
}

/// 48 kHz PCM16 の WAV としてバイト列を書き出す。
pub fn write_wav_bytes(path: &Path, channels: u16, data: &[u8]) {
    let align = channels * 2;
    let mut bytes = Vec::with_capacity(data.len() + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000 * align as u32).to_le_bytes());
    bytes.extend_from_slice(&align.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    std::fs::write(path, bytes).expect("検査用 WAV の書き出し");
}

pub struct Parsed {
    pub channels: u16,
    pub rate: u32,
    pub bits: u16,
    pub data: Vec<u8>,
}

impl Parsed {
    pub fn frames(&self) -> usize {
        self.data.len() / (self.channels as usize * (self.bits as usize / 8))
    }

    /// PCM16 として符号付きサンプル列に直す。
    pub fn samples(&self) -> Vec<i16> {
        self.data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }
}

/// 検査用の最小 WAV 読み取り。本体の実装とは独立に書いて相互検証にする。
pub fn parse_wav(path: &Path) -> Parsed {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("{} を読めません: {}", path.display(), e));
    assert_eq!(&bytes[0..4], b"RIFF", "{} が RIFF でない", path.display());
    assert_eq!(&bytes[8..12], b"WAVE", "{} が WAVE でない", path.display());

    let mut at = 12usize;
    let (mut channels, mut rate, mut bits) = (0u16, 0u32, 0u16);
    let mut data = Vec::new();
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let n = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = at + 8;
        if id == b"fmt " {
            channels = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().unwrap());
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
            bits = u16::from_le_bytes(bytes[body + 14..body + 16].try_into().unwrap());
        } else if id == b"data" {
            data = bytes[body..body + n].to_vec();
        }
        at = body + n + (n % 2);
    }
    Parsed {
        channels,
        rate,
        bits,
        data,
    }
}

/// 出力が 48 kHz / PCM16 / 指定の長さになっているか。
pub fn assert_is_clean_wav(path: &Path, channels: u16, frames: usize) {
    let wav = parse_wav(path);
    assert_eq!(wav.rate, 48_000, "48 kHz であること");
    assert_eq!(wav.bits, 16, "PCM 16bit であること");
    assert_eq!(wav.channels, channels, "チャンネル数が保たれていること");
    assert_eq!(
        wav.data.len(),
        frames * channels as usize * 2,
        "長さが保たれていること"
    );
}

/// 48 kHz モノラル PCM16 の WAV からサンプル列を取り出す。形式が違えば落とす。
pub fn read_mono_pcm16(path: &Path) -> Vec<i16> {
    let wav = parse_wav(path);
    assert_eq!(
        wav.channels,
        1,
        "{} はモノラルにしてください",
        path.display()
    );
    assert_eq!(
        wav.rate,
        48_000,
        "{} は 48 kHz にしてください",
        path.display()
    );
    assert_eq!(
        wav.bits,
        16,
        "{} は PCM 16bit にしてください",
        path.display()
    );
    wav.samples()
}

/// 本体を起動する。DEEPFILTER_HOME だけ指定し、引数はそのまま渡す。
pub fn run_with_home(home: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .env("DEEPFILTER_HOME", home)
        .env_remove("DEEPFILTER_DEBUG")
        .args(args)
        .output()
        .expect("deepfilter-tool の起動")
}

/// エンジンとモデルをリポジトリの runtime/ に固定して起動する。
///
/// テストは並列に走るため、sessions/ を共有すると数え間違える。呼び出し側の
/// 引数を後ろに置くので、テストが自分で --engine を渡した場合はそちらが優先される。
pub fn run_in(home: &Path, args: &[&str]) -> Output {
    let engine = engine_file();
    let model = model_file();
    let mut all: Vec<&str> = vec![
        "--engine",
        engine.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
    ];
    all.extend_from_slice(args);
    run_with_home(home, &all)
}

/// 1 ファイルを処理する。よく使う「入力と出力だけ指定して静かに実行」の形。
///
/// パスは `&Path` のまま渡す。`to_str()` を通すと UTF-8 でないファイル名で
/// 失敗するため、ここを文字列経由にしてはいけない。
pub fn filter_paths(home: &Path, input: &Path, output: &Path, extra: &[&str]) -> Output {
    Command::new(BIN)
        .env("DEEPFILTER_HOME", home)
        .env_remove("DEEPFILTER_DEBUG")
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(extra)
        .arg("-q")
        .output()
        .expect("deepfilter-tool の起動")
}
