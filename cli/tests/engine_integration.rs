//! 公式エンジンを実際に起動する統合テスト。
//!
//! `runtime/` にエンジンとモデルが導入されている場合のみ実行し、
//! 未導入の環境では理由を表示して何も検査せずに終了します。
//! 導入は `deepfilter-tool setup` で行えます。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_deepfilter-tool");
const FRAMES: usize = 48_001;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli/ の親フォルダー")
        .to_path_buf()
}

fn runtime_ready(root: &Path) -> bool {
    let engine = root.join("runtime").join(if cfg!(windows) {
        "deep-filter.exe"
    } else {
        "deep-filter"
    });
    engine.is_file()
        && root
            .join("runtime")
            .join("DeepFilterNet3_onnx.tar.gz")
            .is_file()
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deepfilter-it-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("作業フォルダー作成");
    dir
}

/// 決定的な擬似乱数ノイズと 180 Hz の正弦波を重ねた検査用 WAV を書く。
fn write_test_wav(path: &Path, channels: u16) {
    let align = channels as usize * 2;
    let mut data = vec![0u8; FRAMES * align];
    let mut seed: u32 = 42;
    for frame in 0..FRAMES {
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

    let count = data.len();
    let mut bytes = Vec::with_capacity(count + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + count) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000 * align as u32).to_le_bytes());
    bytes.extend_from_slice(&(align as u16).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(count as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    std::fs::write(path, bytes).expect("検査用 WAV の書き出し");
}

struct Parsed {
    channels: u16,
    rate: u32,
    bits: u16,
    data: Vec<u8>,
}

/// 検査用の最小 WAV 読み取り。本体の実装とは独立に書いて相互検証にする。
fn parse_wav(path: &Path) -> Parsed {
    let bytes = std::fs::read(path).expect("出力 WAV の読み取り");
    assert_eq!(&bytes[0..4], b"RIFF", "RIFF ヘッダー");
    assert_eq!(&bytes[8..12], b"WAVE", "WAVE ヘッダー");
    let mut at = 12;
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

fn engine_file() -> PathBuf {
    repo_root().join("runtime").join(if cfg!(windows) {
        "deep-filter.exe"
    } else {
        "deep-filter"
    })
}

/// テストごとに独立した作業フォルダーを DEEPFILTER_HOME にする。
///
/// テストは並列に走るため、sessions/ を共有すると数え間違える。エンジンとモデルは
/// リポジトリの runtime/ を明示的に指す。呼び出し側の引数を後ろに置くので、
/// テストが自分で --engine を渡した場合はそちらが優先される。
fn run_in(home: &Path, args: &[&str]) -> Output {
    let engine = engine_file();
    let model = repo_root()
        .join("runtime")
        .join("DeepFilterNet3_onnx.tar.gz");
    let mut all: Vec<&str> = vec![
        "--engine",
        engine.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
    ];
    all.extend_from_slice(args);
    Command::new(BIN)
        .env("DEEPFILTER_HOME", home)
        .args(&all)
        .output()
        .expect("deepfilter-tool の起動")
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .env("DEEPFILTER_HOME", repo_root())
        .args(args)
        .output()
        .expect("deepfilter-tool の起動")
}

fn skip_unless_ready() -> bool {
    if runtime_ready(&repo_root()) {
        return false;
    }
    eprintln!(
        "スキップ: runtime/ に公式エンジンとモデルがありません。\
         `deepfilter-tool setup` で導入すると実エンジン検証を実行します。"
    );
    true
}

#[test]
fn filters_mono_and_stereo_preserving_shape() {
    if skip_unless_ready() {
        return;
    }
    for channels in [1u16, 2] {
        let dir = work_dir(&format!("shape{}", channels));
        // 日本語と空白を含むパスでも動くことを併せて確認する。
        let input = dir.join(format!("日本語 & 検査-{}.wav", channels));
        let output = dir.join("結果.wav");
        write_test_wav(&input, channels);
        let before = std::fs::read(&input).unwrap();

        let out = run_in(
            &dir,
            &[
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
                "-q",
            ],
        );
        assert!(
            out.status.success(),
            "処理が失敗しました: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let result = parse_wav(&output);
        assert_eq!(result.channels, channels, "チャンネル数を保持");
        assert_eq!(result.rate, 48_000, "サンプルレートを保持");
        assert_eq!(result.bits, 16, "PCM 16bit で出力");
        assert_eq!(
            result.data.len(),
            FRAMES * channels as usize * 2,
            "フレーム数を保持"
        );
        assert_ne!(result.data, before[44..], "モデルが波形を変化させる");
        assert!(
            result.data.chunks_exact(2).any(|s| s != [0, 0]),
            "出力が無音でない"
        );
        assert_eq!(std::fs::read(&input).unwrap(), before, "入力は変更されない");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn accepts_post_filter_and_attenuation() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("options");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    let plain = dir.join("plain.wav");
    let strong = dir.join("strong.wav");
    assert!(run_in(
        &dir,
        &[input.to_str().unwrap(), "-o", plain.to_str().unwrap(), "-q"]
    )
    .status
    .success());
    let out = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            strong.to_str().unwrap(),
            "-a",
            "60",
            "--pf",
            "-q",
        ],
    );
    assert!(
        out.status.success(),
        "--pf / -a が失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_ne!(
        parse_wav(&plain).data,
        parse_wav(&strong).data,
        "設定を変えると結果も変わる"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removes_session_by_default_and_keeps_it_on_request() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("session");
    let sessions = dir.join("sessions");
    let count = || std::fs::read_dir(&sessions).map(|d| d.count()).unwrap_or(0);

    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    let out = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            dir.join("a.wav").to_str().unwrap(),
            "-q",
        ],
    );
    assert!(
        out.status.success(),
        "処理が失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(count(), 0, "既定では作業フォルダーを残さない");

    assert!(run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            dir.join("b.wav").to_str().unwrap(),
            "--keep-session",
            "-q",
        ]
    )
    .status
    .success());
    assert_eq!(count(), 1, "--keep-session で作業フォルダーが残る");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_bad_input_without_touching_the_original() {
    let dir = work_dir("errors");
    let good = dir.join("good.wav");
    write_test_wav(&good, 1);

    let garbage = dir.join("garbage.wav");
    std::fs::write(&garbage, vec![7u8; 2_000]).unwrap();
    assert!(
        !run_in(&dir, &[garbage.to_str().unwrap(), "-q"])
            .status
            .success(),
        "WAV でないファイルを拒否"
    );

    assert!(
        !run_in(&dir, &[dir.join("missing.wav").to_str().unwrap(), "-q"])
            .status
            .success(),
        "存在しない入力を拒否"
    );
    assert!(
        !run_in(&dir, &[good.to_str().unwrap(), "-a", "0", "-q"])
            .status
            .success(),
        "範囲外の抑制値を拒否"
    );
    assert!(
        !run_in(&dir, &[good.to_str().unwrap(), "-a", "101", "-q"])
            .status
            .success(),
        "範囲外の抑制値を拒否"
    );
    assert!(
        !run_in(
            &dir,
            &[good.to_str().unwrap(), "-o", good.to_str().unwrap(), "-q"]
        )
        .status
        .success(),
        "入力と同じ出力先を拒否"
    );
    assert!(
        !run_in(
            &dir,
            &[
                good.to_str().unwrap(),
                "--engine",
                dir.join("nope").to_str().unwrap(),
                "-o",
                dir.join("x.wav").to_str().unwrap(),
                "-q"
            ]
        )
        .status
        .success(),
        "エンジン未導入を拒否"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_to_overwrite_output_unless_forced() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("overwrite");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);
    let output = dir.join("out.wav");

    assert!(run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-q"
        ]
    )
    .status
    .success());
    assert!(
        !run_in(
            &dir,
            &[
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
                "-q"
            ]
        )
        .status
        .success(),
        "既存の出力先は上書きしない"
    );
    assert!(
        run_in(
            &dir,
            &[
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
                "--force",
                "-q"
            ]
        )
        .status
        .success(),
        "--force なら上書きする"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn manifest_output_matches_the_tracked_file() {
    let out = run(&["manifest"]);
    assert!(out.status.success());
    let tracked = std::fs::read(repo_root().join("runtime").join("manifest.json"))
        .expect("runtime/manifest.json");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&tracked).replace("\r\n", "\n"),
        "manifest 出力と runtime/manifest.json が一致すること"
    );
}

// ---- コマンドライン表層（main.rs）----

#[test]
fn shows_usage_when_given_no_arguments() {
    let out = run(&[]);
    assert!(out.status.success(), "使い方の表示は成功扱い");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("使い方:"), "使い方を出す");
    assert!(text.contains("setup") && text.contains("check") && text.contains("manifest"));
}

#[test]
fn help_is_available_from_every_spelling() {
    for args in [
        vec!["-h"],
        vec!["--help"],
        vec!["help"],
        vec!["setup", "--help"],
        vec!["filter", "--help"],
    ] {
        let out = run(&args);
        assert!(out.status.success(), "{:?} は成功する", args);
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("使い方:"),
            "{:?} が使い方を出す",
            args
        );
    }
}

#[test]
fn version_reports_both_the_tool_and_the_pinned_release() {
    for args in [vec!["-V"], vec!["--version"], vec!["version"]] {
        let out = run(&args);
        assert!(out.status.success(), "{:?} は成功する", args);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("deepfilter-tool"), "{:?}: {}", args, text);
        assert!(
            text.contains("DeepFilterNet v0.5.6"),
            "固定版を示す: {}",
            text
        );
    }
}

#[test]
fn check_reports_a_ready_runtime() {
    if skip_unless_ready() {
        return;
    }
    let out = run(&["check"]);
    assert!(
        out.status.success(),
        "導入済みなら成功: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("利用できます"), "{}", text);
    assert!(
        text.contains("固定版と一致"),
        "SHA-256 の照合結果を出す: {}",
        text
    );
    assert!(text.contains("プラットフォーム"), "対象環境を出す");
}

#[test]
fn check_fails_and_explains_when_the_runtime_is_absent() {
    let dir = work_dir("check-missing");
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("check")
        .output()
        .expect("起動");
    assert!(!out.status.success(), "未導入なら失敗する");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("未導入"),
        "未導入と表示する"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("setup"),
        "導入方法を案内する"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn setup_rejects_an_unknown_platform_and_unknown_arguments() {
    let dir = work_dir("setup-args");
    let call = |args: &[&str]| {
        Command::new(BIN)
            .env("DEEPFILTER_HOME", &dir)
            .args(args)
            .output()
            .expect("起動")
    };

    let bad_platform = call(&["setup", "--platform", "solaris-sparc"]);
    assert!(!bad_platform.status.success());
    let text = String::from_utf8_lossy(&bad_platform.stderr);
    assert!(text.contains("未知のプラットフォーム"), "{}", text);
    assert!(
        text.contains("linux-x86_64"),
        "選べるキーを案内する: {}",
        text
    );

    let unknown = call(&["setup", "--nope"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("不明な引数"));

    let missing_value = call(&["setup", "--platform"]);
    assert!(!missing_value.status.success());
    assert!(String::from_utf8_lossy(&missing_value.stderr).contains("値が必要"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_malformed_command_lines() {
    let dir = work_dir("cli-args");
    let good = dir.join("in.wav");
    write_test_wav(&good, 1);
    let call = |args: &[&str]| {
        Command::new(BIN)
            .env("DEEPFILTER_HOME", &dir)
            .args(args)
            .output()
            .expect("起動")
    };

    let unknown = call(&["--nope", good.to_str().unwrap()]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("不明なオプション"));

    for flag in ["-o", "-a", "--engine", "--model"] {
        let out = call(&[good.to_str().unwrap(), flag]);
        assert!(!out.status.success(), "{} の値なしを拒否", flag);
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("値が必要"),
            "{} の案内",
            flag
        );
    }

    let two_inputs = call(&[good.to_str().unwrap(), good.to_str().unwrap()]);
    assert!(!two_inputs.status.success());
    assert!(String::from_utf8_lossy(&two_inputs.stderr).contains("1 つだけ"));

    let no_input = call(&["-a", "50"]);
    assert!(!no_input.status.success());
    assert!(String::from_utf8_lossy(&no_input.stderr).contains("入力の WAV を指定"));

    let not_a_number = call(&[good.to_str().unwrap(), "-a", "つよく"]);
    assert!(!not_a_number.status.success());
    assert!(String::from_utf8_lossy(&not_a_number.stderr).contains("1〜100"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn accepts_options_before_the_input_and_the_explicit_filter_subcommand() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("argorder");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    let leading = run_in(
        &dir,
        &[
            "-a",
            "80",
            "-o",
            dir.join("a.wav").to_str().unwrap(),
            "-q",
            input.to_str().unwrap(),
        ],
    );
    assert!(
        leading.status.success(),
        "オプションが先頭でも動く: {}",
        String::from_utf8_lossy(&leading.stderr)
    );
    assert!(dir.join("a.wav").is_file());

    // サブコマンド名は先頭に置く決まりなので、ここは組み立てずに直接呼ぶ。
    let model = repo_root()
        .join("runtime")
        .join("DeepFilterNet3_onnx.tar.gz");
    let explicit = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .args([
            "filter",
            input.to_str().unwrap(),
            "--engine",
            engine_file().to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
            "-o",
            dir.join("b.wav").to_str().unwrap(),
            "-q",
        ])
        .output()
        .expect("起動");
    assert!(
        explicit.status.success(),
        "filter サブコマンドでも動く: {}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    assert!(dir.join("b.wav").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_output_sits_next_to_the_input() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("defaultout");
    let input = dir.join("録音.wav");
    write_test_wav(&input, 1);

    let out = run_in(&dir, &[input.to_str().unwrap(), "-q"]);
    assert!(
        out.status.success(),
        "既定の出力先で成功: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.join("録音_clean.wav").is_file(),
        "入力名_clean.wav を作る"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_messages_are_printed_unless_quiet_is_requested() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("verbosity");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    let loud = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            dir.join("a.wav").to_str().unwrap(),
            "--pf",
        ],
    );
    assert!(loud.status.success());
    let text = String::from_utf8_lossy(&loud.stdout);
    assert!(text.contains("入力"), "入力を表示: {}", text);
    assert!(
        text.contains("ポストフィルター有効"),
        "設定を表示: {}",
        text
    );
    assert!(text.contains("完了"), "完了を表示: {}", text);
    assert!(
        text.contains("48 kHz PCM 16bit"),
        "出力形式を表示: {}",
        text
    );

    let quiet = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            dir.join("b.wav").to_str().unwrap(),
            "-q",
        ],
    );
    assert!(quiet.status.success());
    assert!(quiet.stdout.is_empty(), "-q なら何も出さない");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keep_session_prints_where_the_work_and_log_were_left() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("keepmsg");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    let out = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            dir.join("a.wav").to_str().unwrap(),
            "--keep-session",
        ],
    );
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("作業フォルダー"), "{}", text);
    assert!(text.contains("エンジンログ"), "{}", text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn creates_missing_parent_directories_for_the_output() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("mkparent");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);
    let output = dir.join("出力").join("さらに下").join("clean.wav");

    let out = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-q",
        ],
    );
    assert!(
        out.status.success(),
        "親フォルダーを作って書く: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(output.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_run_does_not_leave_an_empty_work_folder() {
    let dir = work_dir("nolitter");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    // モデルだけ存在しない状態にして失敗させる。
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .args([
            "--engine",
            engine_file().to_str().unwrap(),
            "--model",
            dir.join("missing.tar.gz").to_str().unwrap(),
            input.to_str().unwrap(),
            "-o",
            dir.join("out.wav").to_str().unwrap(),
            "-q",
        ])
        .output()
        .expect("起動");
    assert!(!out.status.success());
    assert!(
        !dir.join("sessions").exists()
            || std::fs::read_dir(dir.join("sessions")).unwrap().count() == 0,
        "空の作業フォルダーを残さない"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn setup_without_a_platform_uses_the_running_environment() {
    if skip_unless_ready() {
        return;
    }
    // 固定版が揃っているので通信は起きず、すべて「導入済み」と報告される。
    let out = run(&["setup"]);
    assert!(
        out.status.success(),
        "導入済みの環境では成功: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("すべて導入済み"), "{}", text);
    assert!(text.contains("導入先"), "配置先を示す: {}", text);
    assert!(!text.contains("取得中"), "再取得はしない: {}", text);
}

#[test]
fn setup_accepts_force_alongside_a_platform() {
    let dir = work_dir("setup-force");
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .args(["setup", "--force", "--platform", "solaris-sparc"])
        .output()
        .expect("起動");
    // --force を解釈したうえで、未知のプラットフォームとして断る。
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("未知のプラットフォーム"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_flags_a_runtime_file_that_is_not_the_pinned_build() {
    let dir = work_dir("check-mismatch");
    let runtime = dir.join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let engine = runtime.join(if cfg!(windows) {
        "deep-filter.exe"
    } else {
        "deep-filter"
    });
    std::fs::write(&engine, b"a different build").unwrap();
    std::fs::write(
        runtime.join("DeepFilterNet3_onnx.tar.gz"),
        b"a different model",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("check")
        .output()
        .expect("起動");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("固定版と不一致"),
        "差し替えを見抜く: {}",
        text
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_the_result_even_when_the_output_is_on_another_volume() {
    if skip_unless_ready() {
        return;
    }
    // 作業フォルダーはリポジトリ側、出力は一時フォルダー側に置く。
    // 別ボリュームになる環境では rename が使えず、コピーで置く経路を通る。
    let dir = work_dir("crossvolume");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);
    let output = dir.join("out.wav");

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", repo_root())
        .args([
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-q",
        ])
        .output()
        .expect("起動");
    assert!(
        out.status.success(),
        "別ボリュームでも書ける: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(parse_wav(&output).rate, 48_000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_shows_concrete_examples_and_where_files_are_kept() {
    let text = String::from_utf8_lossy(&run(&["--help"]).stdout).into_owned();
    assert!(text.contains("使用例:"), "使用例の節がある");
    for expected in ["deepfilter-tool setup", "-a 60 --pf", "--force", "check"] {
        assert!(text.contains(expected), "例に {} がある", expected);
    }
    assert!(text.contains("runtime/"), "runtime の役割を書く");
    assert!(text.contains("sessions/"), "sessions の役割を書く");
    assert!(text.contains("DEEPFILTER_HOME"), "環境変数を案内する");
    // 使い方にはすべてのオプションが載っていること。
    for flag in [
        "-o, --output",
        "-a, --attenuation",
        "--pf",
        "--force",
        "--keep-session",
        "--engine",
        "--model",
        "-q, --quiet",
        "--platform",
        "-h, --help",
        "-V, --version",
    ] {
        assert!(text.contains(flag), "{} が説明されている", flag);
    }
    for command in ["setup", "check", "manifest"] {
        assert!(
            text.contains(command),
            "{} サブコマンドが載っている",
            command
        );
    }
}

#[test]
fn version_and_license_show_the_copyright_notice() {
    for args in [vec!["--version"], vec!["license"], vec!["--license"]] {
        let text = String::from_utf8_lossy(&run(&args).stdout).into_owned();
        assert!(
            text.contains("Copyright (c) 2026 bit2zero"),
            "{:?} が著作権表示を出す: {}",
            args,
            text
        );
        assert!(text.contains("MIT"), "{:?} がライセンスを示す", args);
        assert!(
            text.contains("Hendrik Schröter"),
            "{:?} が DeepFilterNet の著作権も示す",
            args
        );
    }
    let license = String::from_utf8_lossy(&run(&["license"]).stdout).into_owned();
    assert!(license.contains("LICENSE"), "全文の場所を案内する");
    assert!(license.contains("NOTICE.md"), "第三者表示の場所を案内する");
}

#[test]
fn the_repository_carries_its_license_files() {
    let root = repo_root();
    let license = std::fs::read_to_string(root.join("LICENSE")).expect("LICENSE がある");
    assert!(license.contains("MIT License"), "MIT ライセンス本文");
    assert!(
        license.contains("Copyright (c) 2026 bit2zero"),
        "著作権表示"
    );
    assert!(
        license.contains("WITHOUT WARRANTY OF ANY KIND"),
        "無保証条項まで含む全文"
    );

    let notice = std::fs::read_to_string(root.join("NOTICE.md")).expect("NOTICE.md がある");
    assert!(notice.contains("Hendrik Schröter"), "上流の著作権者を明記");
    assert!(notice.contains("DeepFilterNet"), "上流の名称を明記");

    // 上流のライセンス本文は再配布時に必要なので、消えていないことを確かめる。
    let upstream = std::fs::read_to_string(root.join("runtime").join("LICENSE-MIT.txt"))
        .expect("runtime/LICENSE-MIT.txt がある");
    assert!(
        upstream.contains("Hendrik Schröter"),
        "上流のMIT本文が同梱されている"
    );
}

#[test]
fn reports_the_input_format_for_float32_sources() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("float32");
    let frames = 4_800usize;
    // IEEE Float 32bit の入力を用意する。PCM とは別の経路を通る。
    let mut body = Vec::with_capacity(frames * 4);
    for frame in 0..frames {
        let value = 0.2f32 * (frame as f32 * 2.0 * std::f32::consts::PI * 220.0 / 48_000.0).sin();
        body.extend_from_slice(&value.to_le_bytes());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + body.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE Float
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);

    let input = dir.join("float.wav");
    std::fs::write(&input, bytes).unwrap();
    let output = dir.join("out.wav");

    let out = run_in(
        &dir,
        &[
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--debug",
            "-q",
        ],
    );
    assert!(
        out.status.success(),
        "Float32 入力で失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = String::from_utf8_lossy(&out.stderr);
    assert!(
        log.contains("IEEE Float"),
        "入力形式を IEEE Float と出す:\n{}",
        log
    );
    assert!(log.contains("32 bit"), "ビット深度を出す");

    // 出力は再生互換性のため PCM 16bit に変換される。
    let result = parse_wav(&output);
    assert_eq!(result.bits, 16, "PCM 16bit で出力");
    assert_eq!(result.data.len(), frames * 2, "長さは保たれる");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tells_where_the_work_folder_was_left_when_the_engine_fails() {
    let dir = work_dir("leftover");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    // 必ず失敗するエンジンの代役を置く。ログが残るので場所を知らせるはず。
    let fake = dir.join(if cfg!(windows) { "fake.cmd" } else { "fake.sh" });
    if cfg!(windows) {
        std::fs::write(
            &fake,
            "@echo off\r\necho 失敗しました 1>&2\r\nexit /b 7\r\n",
        )
        .unwrap();
    } else {
        std::fs::write(&fake, "#!/bin/sh\necho 失敗しました >&2\nexit 7\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .args([
            "--engine",
            fake.to_str().unwrap(),
            "--model",
            model_or_placeholder(&dir).to_str().unwrap(),
            input.to_str().unwrap(),
            "-o",
            dir.join("out.wav").to_str().unwrap(),
            "-q",
        ])
        .output()
        .expect("起動");

    assert!(!out.status.success(), "エンジンが失敗したら失敗として返す");
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("エンジン終了コード 7"),
        "終了コードを伝える: {}",
        text
    );
    assert!(
        text.contains("作業フォルダーを残しました"),
        "調べられるよう場所を伝える: {}",
        text
    );

    let sessions: Vec<PathBuf> = std::fs::read_dir(dir.join("sessions"))
        .expect("sessions が作られる")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(sessions.len(), 1, "作業フォルダーが残る");
    assert!(sessions[0].join("engine.log").is_file(), "ログが残る");

    let _ = std::fs::remove_dir_all(&dir);
}

/// モデルは中身を見ないので、なければ置き場所だけ用意して代用する。
fn model_or_placeholder(dir: &Path) -> PathBuf {
    let real = model_file_path();
    if real.is_file() {
        return real;
    }
    let placeholder = dir.join("model.tar.gz");
    std::fs::write(&placeholder, b"placeholder").unwrap();
    placeholder
}

fn model_file_path() -> PathBuf {
    repo_root()
        .join("runtime")
        .join("DeepFilterNet3_onnx.tar.gz")
}

#[test]
fn rejects_output_paths_without_a_file_name() {
    let dir = work_dir("nofilename");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1);

    for target in ["..", "."] {
        let out = Command::new(BIN)
            .env("DEEPFILTER_HOME", &dir)
            .args([input.to_str().unwrap(), "-o", target, "-q"])
            .output()
            .expect("起動");
        assert!(!out.status.success(), "{} を出力先として断る", target);
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(
            text.contains("ファイル名") || text.contains("別の名前"),
            "{} の理由を伝える: {}",
            target,
            text
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
