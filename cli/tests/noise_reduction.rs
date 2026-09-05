//! 実録音での効果測定。
//!
//! ノイズのない音声（clean）と、それに雑音を重ねた音声（noisy）の対を用意し、
//! noisy を処理した結果が clean にどれだけ近づいたかを数値で確かめる。
//!
//! 音声は次のいずれかから読み込みます。
//!
//! 1. リポジトリ直下の `samples/clean.wav` と `samples/noisy.wav`
//! 2. 環境変数 `DEEPFILTER_CLEAN` と `DEEPFILTER_NOISY` で指すファイル
//!
//! どちらもなければ、理由を表示して何も検査せずに終了します。
//!
//! 2つの音声は 48 kHz モノラル PCM16 で、**同じ長さかつ時間が揃っている**
//! 必要があります（noisy = clean + 雑音）。ずれていると測定になりません。

mod common;

use common::{filter_paths, read_mono_pcm16, repo_root, runtime_ready};
use std::path::{Path, PathBuf};

fn work_dir(name: &str) -> PathBuf {
    common::work_dir("snr", name)
}

/// 検査に使う音声の対。見つからなければ None。
fn reference_pair() -> Option<(PathBuf, PathBuf)> {
    let from_env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    let clean =
        from_env("DEEPFILTER_CLEAN").unwrap_or_else(|| repo_root().join("samples/clean.wav"));
    let noisy =
        from_env("DEEPFILTER_NOISY").unwrap_or_else(|| repo_root().join("samples/noisy.wav"));
    if clean.is_file() && noisy.is_file() {
        Some((clean, noisy))
    } else {
        None
    }
}

fn skip_unless_ready() -> Option<(PathBuf, PathBuf)> {
    if !runtime_ready() {
        eprintln!("スキップ: runtime/ が未導入です。`deepfilter-tool setup` で導入してください。");
        return None;
    }
    match reference_pair() {
        Some(pair) => Some(pair),
        None => {
            eprintln!(
                "スキップ: 効果測定用の音声がありません。\n  \
                 samples/clean.wav と samples/noisy.wav を置くか、\n  \
                 DEEPFILTER_CLEAN と DEEPFILTER_NOISY で指定してください。"
            );
            None
        }
    }
}

fn energy(samples: &[i16]) -> f64 {
    samples.iter().map(|s| (*s as f64) * (*s as f64)).sum()
}

fn residual(reference: &[i16], signal: &[i16]) -> Vec<f64> {
    reference
        .iter()
        .zip(signal.iter())
        .map(|(r, s)| (*s as f64) - (*r as f64))
        .collect()
}

fn residual_energy(reference: &[i16], signal: &[i16]) -> f64 {
    residual(reference, signal).iter().map(|d| d * d).sum()
}

/// 正解 reference に対する signal の SN 比（dB）。
fn snr_db(reference: &[i16], signal: &[i16]) -> f64 {
    let noise = residual_energy(reference, signal);
    if noise == 0.0 {
        return f64::INFINITY;
    }
    10.0 * (energy(reference) / noise).log10()
}

/// clean がほぼ無音の区間での実効値。雑音の底がどれだけ下がったかを見る。
fn noise_floor(reference: &[i16], signal: &[i16]) -> f64 {
    let picked: Vec<f64> = reference
        .iter()
        .zip(signal.iter())
        .filter(|(r, _)| r.unsigned_abs() < 200)
        .map(|(_, s)| *s as f64)
        .collect();
    if picked.is_empty() {
        return 0.0;
    }
    (picked.iter().map(|v| v * v).sum::<f64>() / picked.len() as f64).sqrt()
}

/// 何サンプルずらすと最も相関するか。0 でなければ時間がずれている。
fn best_offset(a: &[i16], b: &[i16], search: i64) -> i64 {
    let mut best = 0i64;
    let mut best_score = f64::NEG_INFINITY;
    for offset in -search..=search {
        let mut acc = 0.0f64;
        let mut i = 0usize;
        while i < a.len() {
            let j = i as i64 + offset;
            if j >= 0 && (j as usize) < b.len() {
                acc += (a[i] as f64) * (b[j as usize] as f64);
            }
            i += 97; // 全点見なくても位置は十分決まる
        }
        if acc > best_score {
            best_score = acc;
            best = offset;
        }
    }
    best
}

fn filter(dir: &Path, input: &Path, output: &Path, extra: &[&str]) {
    let out = filter_paths(dir, input, output, extra);
    assert!(
        out.status.success(),
        "処理に失敗しました: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn reduces_real_recorded_noise_measurably() {
    let (clean_path, noisy_path) = match skip_unless_ready() {
        Some(pair) => pair,
        None => return,
    };
    let dir = work_dir("measure");
    let output = dir.join("filtered.wav");

    let clean = read_mono_pcm16(&clean_path);
    let noisy = read_mono_pcm16(&noisy_path);
    assert_eq!(
        clean.len(),
        noisy.len(),
        "clean と noisy は同じ長さにしてください（noisy = clean + 雑音）"
    );

    filter(&dir, &noisy_path, &output, &[]);
    let filtered = read_mono_pcm16(&output);

    // 長さと時間の整合。ここが崩れると以降の測定に意味がない。
    assert_eq!(filtered.len(), clean.len(), "処理で長さが変わらない");
    assert_eq!(
        best_offset(&clean, &filtered, 64),
        0,
        "処理で時間がずれない（遅延補正が効いている）"
    );

    let before = snr_db(&clean, &noisy);
    let after = snr_db(&clean, &filtered);
    let improvement = after - before;

    let noise_before = residual_energy(&clean, &noisy);
    let noise_after = residual_energy(&clean, &filtered);
    let noise_change_db = 10.0 * (noise_after / noise_before).log10();

    let floor_before = noise_floor(&clean, &noisy);
    let floor_after = noise_floor(&clean, &filtered);
    let floor_change_db = 20.0 * (floor_after / floor_before).log10();

    println!("  処理前 SNR      : {:6.2} dB", before);
    println!("  処理後 SNR      : {:6.2} dB", after);
    println!("  SNR 改善        : {:+6.2} dB", improvement);
    println!("  残留ノイズ      : {:+6.2} dB", noise_change_db);
    println!(
        "  ノイズ床(無音部): {:+6.2} dB （{:.1} → {:.1}）",
        floor_change_db, floor_before, floor_after
    );

    // 閾値は実測値より十分低く取り、環境差で揺れないようにする。
    assert!(
        improvement >= 1.5,
        "SNR が 1.5 dB 以上改善すること。実測 {:+.2} dB",
        improvement
    );
    assert!(
        noise_change_db <= -1.5,
        "残留ノイズが 1.5 dB 以上減ること。実測 {:+.2} dB",
        noise_change_db
    );
    assert!(
        floor_change_db <= -3.0,
        "無音区間のノイズ床が 3 dB 以上下がること。実測 {:+.2} dB",
        floor_change_db
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_the_speech_while_removing_noise() {
    let (clean_path, noisy_path) = match skip_unless_ready() {
        Some(pair) => pair,
        None => return,
    };
    let dir = work_dir("speech");
    let output = dir.join("filtered.wav");

    let clean = read_mono_pcm16(&clean_path);
    filter(&dir, &noisy_path, &output, &[]);
    let filtered = read_mono_pcm16(&output);

    // 声のある区間のエネルギーが保たれていること。
    // ノイズを消すあまり声まで削っていないかを見る。
    let loud: Vec<usize> = (0..clean.len())
        .filter(|i| clean[*i].unsigned_abs() >= 2000)
        .collect();
    assert!(!loud.is_empty(), "clean に十分な音量の区間が必要です");

    let e_clean: f64 = loud.iter().map(|i| (clean[*i] as f64).powi(2)).sum();
    let e_filtered: f64 = loud.iter().map(|i| (filtered[*i] as f64).powi(2)).sum();
    let kept = 10.0 * (e_filtered / e_clean).log10();

    println!(
        "  声の区間 {} サンプル ({:.1}%) のエネルギー差: {:+.2} dB",
        loud.len(),
        100.0 * loud.len() as f64 / clean.len() as f64,
        kept
    );

    assert!(
        (-6.0..=6.0).contains(&kept),
        "声の区間のエネルギーが大きく変わらないこと。実測 {:+.2} dB",
        kept
    );

    // 出力が無音や入力そのままになっていないこと。
    let noisy = read_mono_pcm16(&noisy_path);
    assert!(filtered.iter().any(|s| *s != 0), "出力が無音でない");
    assert_ne!(filtered, noisy, "入力をそのまま返していない");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stronger_settings_do_not_make_the_result_worse() {
    let (clean_path, noisy_path) = match skip_unless_ready() {
        Some(pair) => pair,
        None => return,
    };
    let dir = work_dir("settings");
    let clean = read_mono_pcm16(&clean_path);
    let baseline = snr_db(&clean, &read_mono_pcm16(&noisy_path));

    for (label, options) in [
        ("既定", &[][..]),
        ("--pf", &["--pf"][..]),
        ("-a 60", &["-a", "60"][..]),
        ("-a 60 --pf", &["-a", "60", "--pf"][..]),
        ("-a 20", &["-a", "20"][..]),
    ] {
        let output = dir.join(format!("out{}.wav", options.len()));
        let _ = std::fs::remove_file(&output);
        filter(&dir, &noisy_path, &output, options);
        let snr = snr_db(&clean, &read_mono_pcm16(&output));
        println!(
            "  {:<12} SNR {:6.2} dB ({:+.2} dB)",
            label,
            snr,
            snr - baseline
        );
        assert!(
            snr > baseline,
            "{} でも処理前より良くなること。処理前 {:.2} dB / 処理後 {:.2} dB",
            label,
            baseline,
            snr
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
