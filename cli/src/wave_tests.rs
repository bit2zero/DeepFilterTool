//! wave.rs の単体テスト。ここでは公式エンジンを起動しない。

use super::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deepfilter-wave-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Wave は音声データを持つため Debug を導出していない。失敗時のダンプを避ける。
fn read_err(path: &Path) -> Error {
    match Wave::read(path) {
        Ok(_) => panic!("読み取りが成功してしまった: {}", path.display()),
        Err(e) => e,
    }
}

fn pcm16(channels: u16, frames: usize) -> Wave {
    let align = channels * 2;
    let mut data = vec![0u8; frames * align as usize];
    for i in 0..frames * channels as usize {
        let sample = ((i as i32 % 4001) - 2000) as i16;
        data[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes());
    }
    Wave {
        format: 1,
        channels,
        rate: 48000,
        align,
        bits: 16,
        data,
    }
}

#[test]
fn round_trips_mono_and_stereo() {
    for channels in [1u16, 2] {
        let frames = 1234;
        let source = pcm16(channels, frames);
        let path = scratch(&format!("roundtrip-{}.wav", channels));
        let _ = std::fs::remove_file(&path);
        source.write(&path, frames, false).unwrap();

        let back = Wave::read(&path).unwrap();
        assert_eq!(back.frames(), frames);
        assert_eq!(back.channels, channels);
        assert_eq!(back.rate, 48000);
        assert_eq!(back.format, 1);
        assert_eq!(back.bits, 16);
        assert_eq!(back.data, source.data);
    }
}

#[test]
fn pads_with_silence_and_crops_back() {
    let frames = 1000;
    let source = pcm16(1, frames);
    let padded_path = scratch("padded.wav");
    let _ = std::fs::remove_file(&padded_path);
    source.write(&padded_path, frames + 480, true).unwrap();

    let padded = Wave::read(&padded_path).unwrap();
    assert_eq!(padded.frames(), frames + 480);
    assert!(padded.data[frames * 2..].iter().all(|b| *b == 0));

    let cropped_path = scratch("cropped.wav");
    let _ = std::fs::remove_file(&cropped_path);
    padded.write(&cropped_path, frames, false).unwrap();
    assert_eq!(Wave::read(&cropped_path).unwrap().data, source.data);
}

#[test]
fn refuses_to_overwrite_existing_file() {
    let path = scratch("existing.wav");
    let _ = std::fs::remove_file(&path);
    let source = pcm16(1, 10);
    source.write(&path, 10, false).unwrap();
    assert!(source.write(&path, 10, false).is_err());
}

#[test]
fn refuses_crop_longer_than_content() {
    let path = scratch("tooshort.wav");
    let _ = std::fs::remove_file(&path);
    assert!(pcm16(1, 10).write(&path, 11, false).is_err());
}

#[test]
fn converts_float32_to_pcm16_with_clamping() {
    let values: [f32; 6] = [0.0, 1.0, -1.0, 2.0, -2.0, 0.5];
    let mut data = Vec::new();
    for v in values {
        data.extend_from_slice(&v.to_le_bytes());
    }
    let mut wave = Wave {
        format: 3,
        channels: 1,
        rate: 48000,
        align: 4,
        bits: 32,
        data,
    };
    wave.convert_to_pcm16().unwrap();
    assert_eq!(wave.format, 1);
    assert_eq!(wave.bits, 16);
    assert_eq!(wave.align, 2);
    let got: Vec<i16> = wave
        .data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    // クランプ後に 32767 倍。0.5 は 16383.5 → 最近接偶数丸めで 16384。
    assert_eq!(got, vec![0, 32767, -32767, 32767, -32767, 16384]);
}

#[test]
fn rejects_nan_and_infinity() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut wave = Wave {
            format: 3,
            channels: 1,
            rate: 48000,
            align: 4,
            bits: 32,
            data: bad.to_le_bytes().to_vec(),
        };
        assert!(wave.convert_to_pcm16().is_err());
    }
}

#[test]
fn rejects_unsupported_sample_rate() {
    let path = scratch("rate44100.wav");
    let _ = std::fs::remove_file(&path);
    let mut wave = pcm16(1, 100);
    wave.rate = 44100;
    wave.write(&path, 100, false).unwrap();
    assert!(Wave::read(&path).is_err());
}

#[test]
fn rejects_broken_header() {
    let path = scratch("broken.wav");
    let _ = std::fs::remove_file(&path);
    let mut file = std::fs::File::create(&path).unwrap();
    // RIFF が申告するサイズが実ファイルより大きい。
    file.write_all(b"RIFF").unwrap();
    file.write_all(&9_999_999u32.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    drop(file);
    assert!(Wave::read(&path).is_err());
}

#[test]
fn rejects_non_riff_input() {
    let path = scratch("notwav.bin");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, b"this is not a wave file at all").unwrap();
    assert!(Wave::read(&path).is_err());
}

#[test]
fn skips_unknown_chunks_before_data() {
    // LIST チャンクを挟んでも fmt / data を読めること。
    let body_frames = 8usize;
    let audio = vec![0x11u8; body_frames * 2];
    let list: [u8; 4] = *b"INFO";
    let count = 4 + 24 + (8 + list.len()) + (8 + audio.len());
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(count as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48000u32.to_le_bytes());
    bytes.extend_from_slice(&96000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"LIST");
    bytes.extend_from_slice(&(list.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&list);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(audio.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&audio);

    let path = scratch("withlist.wav");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, &bytes).unwrap();
    let wave = Wave::read(&path).unwrap();
    assert_eq!(wave.frames(), body_frames);
    assert_eq!(wave.data, audio);
}

#[test]
fn handles_non_ascii_paths() {
    let path = scratch("日本語 & テスト.wav");
    let _ = std::fs::remove_file(&path);
    let source = pcm16(2, 64);
    source.write(&path, 64, false).unwrap();
    assert_eq!(Wave::read(&path).unwrap().data, source.data);
}

#[test]
fn reports_a_missing_file_with_its_path() {
    let path = scratch("does-not-exist.wav");
    let _ = std::fs::remove_file(&path);
    let err = read_err(&path);
    assert!(err.0.contains("WAV を開けません"), "{}", err.0);
    assert!(
        err.0.contains("does-not-exist.wav"),
        "パスを示す: {}",
        err.0
    );
}

#[test]
fn rejects_a_file_shorter_than_a_riff_header() {
    let path = scratch("tiny.wav");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, b"RIFF").unwrap();
    assert!(Wave::read(&path).is_err(), "12 バイト未満を拒否");
}

/// チャンクを任意に並べた RIFF を組み立てる。RIFF サイズは実体に合わせる。
fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (id, data) in chunks {
        body.extend_from_slice(*id);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((4 + body.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(&body);
    out
}

fn fmt_chunk(format: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let align = channels * (bits / 8);
    let mut out = Vec::new();
    out.extend_from_slice(&format.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * align as u32).to_le_bytes());
    out.extend_from_slice(&align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out
}

fn write_bytes(name: &str, bytes: &[u8]) -> PathBuf {
    let path = scratch(name);
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn rejects_a_chunk_that_runs_past_the_end_of_the_riff() {
    // data チャンクが申告するサイズを、RIFF に収まらない値へ書き換える。
    let mut bytes = riff(&[
        (b"fmt ", fmt_chunk(1, 1, 48000, 16)),
        (b"data", vec![0u8; 16]),
    ]);
    let at = bytes.len() - 16 - 4;
    bytes[at..at + 4].copy_from_slice(&9_000u32.to_le_bytes());
    let path = write_bytes("overrun.wav", &bytes);
    let err = read_err(&path);
    assert!(err.0.contains("WAV が不正"), "{}", err.0);
}

#[test]
fn rejects_a_truncated_fmt_chunk() {
    let bytes = riff(&[(b"fmt ", vec![1, 0, 1, 0]), (b"data", vec![0u8; 16])]);
    let path = write_bytes("shortfmt.wav", &bytes);
    let err = read_err(&path);
    assert!(err.0.contains("fmt チャンクが不正"), "{}", err.0);
}

#[test]
fn rejects_a_file_with_no_fmt_chunk() {
    let bytes = riff(&[(b"data", vec![0u8; 16])]);
    let path = write_bytes("nofmt.wav", &bytes);
    let err = read_err(&path);
    assert!(err.0.contains("音声データがありません"), "{}", err.0);
}

#[test]
fn rejects_a_file_with_an_empty_data_chunk() {
    let bytes = riff(&[(b"fmt ", fmt_chunk(1, 1, 48000, 16)), (b"data", Vec::new())]);
    let path = write_bytes("nodata.wav", &bytes);
    let err = read_err(&path);
    assert!(err.0.contains("音声データがありません"), "{}", err.0);
}

#[test]
fn rejects_unsupported_channel_counts_and_bit_depths() {
    let cases: [(u16, u16, u32, u16, &str); 4] = [
        (1, 6, 48000, 16, "5.1ch"),
        (1, 1, 48000, 24, "PCM 24bit"),
        (3, 1, 48000, 16, "Float 16bit"),
        (2, 1, 48000, 16, "未対応のフォーマット番号"),
    ];
    for (format, channels, rate, bits, label) in cases {
        let frames = 8usize;
        let data = vec![0u8; frames * channels as usize * (bits as usize / 8)];
        let bytes = riff(&[
            (b"fmt ", fmt_chunk(format, channels, rate, bits)),
            (b"data", data),
        ]);
        let path = write_bytes(&format!("unsupported-{}.wav", label), &bytes);
        assert!(Wave::read(&path).is_err(), "{} を拒否", label);
    }
}

#[test]
fn rejects_a_data_chunk_that_is_not_a_whole_number_of_frames() {
    // ステレオ 16bit なら 1 フレーム 4 バイト。6 バイトは端数になる。
    let bytes = riff(&[
        (b"fmt ", fmt_chunk(1, 2, 48000, 16)),
        (b"data", vec![0u8; 6]),
    ]);
    let path = write_bytes("ragged.wav", &bytes);
    assert!(
        Wave::read(&path).is_err(),
        "フレーム境界に揃わない data を拒否"
    );
}

#[test]
fn accepts_float32_input_and_reads_it_back() {
    let frames = 32usize;
    let mut data = Vec::new();
    for i in 0..frames {
        data.extend_from_slice(&((i as f32) / 100.0).to_le_bytes());
    }
    let bytes = riff(&[
        (b"fmt ", fmt_chunk(3, 1, 48000, 32)),
        (b"data", data.clone()),
    ]);
    let path = write_bytes("float32.wav", &bytes);
    let mut wave = Wave::read(&path).unwrap();
    assert_eq!(wave.format, 3);
    assert_eq!(wave.bits, 32);
    assert_eq!(wave.frames(), frames);
    wave.convert_to_pcm16().unwrap();
    assert_eq!(wave.frames(), frames, "変換してもフレーム数は変わらない");
}

#[test]
fn write_rejects_a_frame_count_below_one() {
    let path = scratch("zero.wav");
    let _ = std::fs::remove_file(&path);
    assert!(
        pcm16(1, 10).write(&path, 0, true).is_err(),
        "0 フレームを拒否"
    );
    assert!(!path.exists(), "拒否したらファイルを作らない");
}

#[test]
fn write_rejects_sizes_that_overflow_or_exceed_four_gigabytes() {
    let path = scratch("huge.wav");
    let _ = std::fs::remove_file(&path);
    let source = pcm16(2, 10);

    // 掛け算があふれる。
    let err = source.write(&path, usize::MAX, true).unwrap_err();
    assert!(
        err.0.contains("あふれ") || err.0.contains("4 GB"),
        "{}",
        err.0
    );

    // あふれないが 4 GB を超える。
    let err = source.write(&path, 1_500_000_000, true).unwrap_err();
    assert!(err.0.contains("4 GB"), "{}", err.0);
    assert!(!path.exists(), "拒否したらファイルを作らない");
}

#[test]
fn write_pads_beyond_the_internal_zero_buffer() {
    // 無音埋めは 8 KiB 単位で回すため、その境界をまたぐ長さを検査する。
    let frames = 4usize;
    let padded = 20_000usize;
    let path = scratch("bigpad.wav");
    let _ = std::fs::remove_file(&path);
    let source = pcm16(1, frames);
    source.write(&path, padded, true).unwrap();

    let back = Wave::read(&path).unwrap();
    assert_eq!(back.frames(), padded);
    assert!(
        back.data[frames * 2..].iter().all(|b| *b == 0),
        "全域が無音"
    );
}
