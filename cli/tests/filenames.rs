//! ファイル名の文字コードに関する検査。
//!
//! 全角の日本語、濁点・半濁点（合成済み／結合文字）、半角カタカナ、絵文字、
//! 空白や記号、日本語のフォルダー名、そして UTF-8 ではないファイル名まで扱えることを、
//! 公式エンジンを実際に起動して確認する。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_deepfilter-tool");
const FRAMES: usize = 4_800;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli/ の親フォルダー")
        .to_path_buf()
}

fn engine_file() -> PathBuf {
    repo_root().join("runtime").join(if cfg!(windows) {
        "deep-filter.exe"
    } else {
        "deep-filter"
    })
}

fn model_file() -> PathBuf {
    repo_root()
        .join("runtime")
        .join("DeepFilterNet3_onnx.tar.gz")
}

fn skip_unless_ready() -> bool {
    if engine_file().is_file() && model_file().is_file() {
        return false;
    }
    eprintln!("スキップ: runtime/ が未導入です。`deepfilter-tool setup` で導入してください。");
    true
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("deepfilter-name-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("作業フォルダー作成");
    dir
}

fn write_test_wav(path: &Path) {
    let mut data = vec![0u8; FRAMES * 2];
    let mut seed: u32 = 7;
    for frame in 0..FRAMES {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((seed >> 16) as i32 - 32_768) / 20;
        let tone =
            (2_000.0 * (frame as f64 * 2.0 * std::f64::consts::PI * 220.0 / 48_000.0).sin()) as i32;
        let value = (noise + tone).clamp(-32_768, 32_767) as i16;
        data[frame * 2..frame * 2 + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&96_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    std::fs::write(path, bytes).expect("検査用 WAV の書き出し");
}

/// 出力が 48 kHz / モノラル / PCM16 で、元と同じ長さになっているか。
fn assert_is_clean_wav(path: &Path) {
    let bytes = std::fs::read(path).expect("出力の読み取り");
    assert_eq!(&bytes[0..4], b"RIFF", "RIFF で始まる");
    assert_eq!(&bytes[8..12], b"WAVE", "WAVE である");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        48_000,
        "48 kHz"
    );
    assert_eq!(
        u16::from_le_bytes(bytes[34..36].try_into().unwrap()),
        16,
        "PCM 16bit"
    );
    assert_eq!(bytes.len(), 44 + FRAMES * 2, "長さが保たれている");
}

fn run_with_home(home: &Path, input: &Path, output: &Path) -> Output {
    Command::new(BIN)
        .env("DEEPFILTER_HOME", home)
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("-q")
        .output()
        .expect("deepfilter-tool の起動")
}

/// さまざまな表記のファイル名を、実エンジンを通して処理する。
#[test]
fn handles_japanese_and_other_non_ascii_file_names() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("names");

    let names: [(&str, &str); 9] = [
        ("全角の漢字とかな", "音声ファイル.wav"),
        ("濁点と半濁点（合成済み）", "がぎぐげごぱぴ.wav"),
        // 結合文字による濁点。macOS の正規化（NFD）と同じ並び。
        ("濁点（結合文字）", "か\u{3099}き\u{3099}は\u{309A}.wav"),
        ("半角カタカナ", "ﾃｽﾄｵﾝｾｲ.wav"),
        ("空白と記号", "日本語 & テスト (1).wav"),
        ("絵文字", "🎵音声🎤.wav"),
        (
            "長い名前",
            "とても長い日本語のファイル名で拡張子まで届くかどうかを確かめる.wav",
        ),
        ("中国語・韓国語", "测试_테스트.wav"),
        ("拡張子前が全角の点", "音声。テスト.wav"),
    ];

    for (label, name) in names {
        let input = dir.join(name);
        write_test_wav(&input);
        assert!(input.is_file(), "{}: 入力を作れる", label);
        let before = std::fs::read(&input).expect("入力の読み取り");

        // 出力名にも同じ文字を含める。
        let output = dir.join(format!("出力_{}", name));
        let out = run_with_home(&dir, &input, &output);
        assert!(
            out.status.success(),
            "{} ({}): 処理に失敗 -> {}",
            label,
            name,
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(output.is_file(), "{}: 出力が作られる", label);
        assert_is_clean_wav(&output);
        assert_eq!(
            std::fs::read(&input).expect("入力の再読み取り"),
            before,
            "{}: 入力は変更されない",
            label
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// 既定の出力名（入力名_clean.wav）が日本語のままになるか。
#[test]
fn default_output_name_keeps_the_japanese_stem() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("defaultname");
    let input = dir.join("会議の録音.wav");
    write_test_wav(&input);

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(&input)
        .arg("-q")
        .output()
        .expect("起動");
    assert!(
        out.status.success(),
        "処理に失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.join("会議の録音_clean.wav").is_file(),
        "日本語の語幹を保つ"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 日本語のフォルダー名の下でも動くか。作業フォルダーも日本語パスの下に作られる。
#[test]
fn handles_japanese_directory_names_including_the_work_folder() {
    if skip_unless_ready() {
        return;
    }
    let base = work_dir("dirs");
    let home = base.join("作業 フォルダー");
    let nested = home.join("録音").join("2026年").join("会議");
    std::fs::create_dir_all(&nested).expect("日本語フォルダーの作成");

    let input = nested.join("議事録.wav");
    write_test_wav(&input);
    let output = nested.join("議事録_きれい.wav");

    // DEEPFILTER_HOME が日本語なので、sessions/ も日本語パスの下に作られ、
    // 公式エンジンには日本語を含むパスが引数として渡る。
    let out = run_with_home(&home, &input, &output);
    assert!(
        out.status.success(),
        "日本語パスで失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(output.is_file());
    assert_is_clean_wav(&output);

    let _ = std::fs::remove_dir_all(&base);
}

/// 日本語パスのまま作業フォルダーを残せるか。
#[test]
fn keeps_a_session_under_a_japanese_home() {
    if skip_unless_ready() {
        return;
    }
    let base = work_dir("keepjp");
    let home = base.join("日本語のホーム");
    std::fs::create_dir_all(&home).unwrap();
    let input = home.join("入力音声.wav");
    write_test_wav(&input);

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &home)
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(&input)
        .arg("-o")
        .arg(home.join("結果.wav"))
        .arg("--keep-session")
        .arg("-q")
        .output()
        .expect("起動");
    assert!(
        out.status.success(),
        "失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let sessions: Vec<PathBuf> = std::fs::read_dir(home.join("sessions"))
        .expect("sessions が作られる")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(sessions.len(), 1, "作業フォルダーが 1 つ残る");
    let session = &sessions[0];
    assert!(
        session.join("engine.log").is_file(),
        "ログが日本語パス下に残る"
    );
    assert!(session.join("input.wav").is_file(), "中間ファイルが残る");
    assert!(
        session.join("filtered").join("input.wav").is_file(),
        "エンジンが日本語パス下に出力できている"
    );

    let _ = std::fs::remove_dir_all(&base);
}

/// UTF-8 ではないファイル名（例: Shift_JIS のバイト列）でもパニックしないこと。
///
/// Unix ではファイル名は任意のバイト列で、古い環境から持ち込んだファイルが
/// UTF-8 でないことがある。Windows のファイル名は UTF-16 なのでこの経路はない。
#[cfg(unix)]
#[test]
fn handles_file_names_that_are_not_valid_utf8() {
    if skip_unless_ready() {
        return;
    }
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = work_dir("sjis");
    // Shift_JIS の「日本語」+ ".wav"。UTF-8 としては不正なバイト列。
    let raw = b"\x93\xfa\x96\x7b\x8c\xea.wav";
    let input = dir.join(OsStr::from_bytes(raw));
    write_test_wav(&input);
    assert!(input.is_file(), "UTF-8 でない名前でファイルを作れる");

    let output = dir.join(OsStr::from_bytes(b"\x8c\x8b\x89\xca.wav"));
    let out = run_with_home(&dir, &input, &output);
    assert!(
        out.status.success(),
        "UTF-8 でない名前で失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(output.is_file(), "UTF-8 でない名前で出力できる");
    assert_is_clean_wav(&output);

    // 既定の出力名も、バイト列のまま組み立てられること。
    let plain = dir.join(OsStr::from_bytes(b"\x93\xfa\x96\x7b.wav"));
    write_test_wav(&plain);
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(&plain)
        .arg("-q")
        .output()
        .expect("起動");
    assert!(
        out.status.success(),
        "既定の出力名で失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.join(OsStr::from_bytes(b"\x93\xfa\x96\x7b_clean.wav"))
            .is_file(),
        "既定の出力名がバイト列のまま作られる"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// UTF-8 でない引数を渡してもパニックせず、エラーとして報告されること。
#[cfg(unix)]
#[test]
fn reports_rather_than_panics_on_non_utf8_arguments() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = work_dir("badargs");

    // 存在しない UTF-8 でない入力。
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg(OsStr::from_bytes(b"\x93\xfa\x96\x7b\x8c\xea-missing.wav"))
        .arg("-q")
        .output()
        .expect("起動");
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("入力が見つかりません"),
        "きちんと報告する: {}",
        text
    );
    assert!(!text.contains("panicked"), "パニックしない: {}", text);

    // UTF-8 でない値を数値として渡した場合。
    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("-a")
        .arg(OsStr::from_bytes(b"\x93\xfa"))
        .arg("x.wav")
        .output()
        .expect("起動");
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("解釈できません"),
        "値の不正を伝える: {}",
        text
    );
    assert!(!text.contains("panicked"), "パニックしない: {}", text);

    let _ = std::fs::remove_dir_all(&dir);
}

/// 日本語ファイル名でも、入力と同じ出力先はきちんと拒否されること。
#[test]
fn still_refuses_to_write_over_a_japanese_input() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("selfjp");
    let input = dir.join("上書き禁止.wav");
    write_test_wav(&input);
    let before = std::fs::read(&input).unwrap();

    let out = run_with_home(&dir, &input, &input);
    assert!(!out.status.success(), "入力と同じ出力先を拒否");
    assert!(String::from_utf8_lossy(&out.stderr).contains("別の名前"));
    assert_eq!(std::fs::read(&input).unwrap(), before, "入力は無傷");
    let _ = std::fs::remove_dir_all(&dir);
}

/// パスの書き方が違っても同じように扱えること（相対・`./`・`..`・絶対）。
#[test]
fn accepts_every_spelling_of_a_multibyte_path() {
    if skip_unless_ready() {
        return;
    }
    let base = work_dir("forms");
    let nested = base.join("録音").join("素材");
    std::fs::create_dir_all(&nested).unwrap();
    let input = nested.join("音声.wav");
    write_test_wav(&input);

    let forms: Vec<PathBuf> = vec![
        input.clone(),                                           // 絶対パス
        base.join("./録音/素材/音声.wav"),                       // ./ を含む
        base.join("録音").join("..").join("録音/素材/音声.wav"), // .. を含む
    ];

    for (i, form) in forms.iter().enumerate() {
        let output = base.join(format!("結果{}.wav", i));
        let out = run_with_home(&base, form, &output);
        assert!(
            out.status.success(),
            "{} の書き方で失敗: {}",
            form.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_is_clean_wav(&output);
    }

    let _ = std::fs::remove_dir_all(&base);
}

/// 入力と同じファイルを、別の書き方で出力先に指定しても上書きしないこと。
#[test]
fn detects_the_input_even_when_spelled_differently() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("alias");
    let input = dir.join("元の音声.wav");
    write_test_wav(&input);
    let before = std::fs::read(&input).unwrap();

    let alias = dir.join(".").join("元の音声.wav");
    let out = run_with_home(&dir, &input, &alias);

    assert!(
        !out.status.success(),
        "書き方が違っても同一ファイルと分かる"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("別の名前"));
    assert_eq!(std::fs::read(&input).unwrap(), before, "入力は無傷");
    let _ = std::fs::remove_dir_all(&dir);
}

/// どの OS でも持ち運べない出力名を、その場で断ること。
///
/// Windows の予約デバイス名に書くと内容がどこにも残らず、末尾の空白や点は
/// 黙って削られて別名になる。Linux や macOS でも同じ判断をして、
/// 後から Windows へ持っていけないファイルを作らないようにする。
#[test]
fn refuses_output_names_that_break_on_windows() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("badnames");
    let input = dir.join("入力.wav");
    write_test_wav(&input);

    for (name, expected) in [
        ("NUL", "予約名"),
        ("nul.wav", "予約名"),
        ("CON.wav", "予約名"),
        ("com1.wav", "予約名"),
        ("音声 .wav.", "末尾"),
        ("音声.wav ", "末尾"),
        ("音*声.wav", "使えない文字"),
        ("音?声.wav", "使えない文字"),
        ("音|声.wav", "使えない文字"),
    ] {
        let out = run_with_home(&dir, &input, &dir.join(name));
        assert!(!out.status.success(), "{} を断る", name);
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(
            text.contains(expected),
            "{} の理由を伝える（「{}」を含む）: {}",
            name,
            expected,
            text
        );
    }

    // 日本語や空白そのものは問題なく通ること。
    let ok = dir.join("日本語 の 出力.wav");
    assert!(
        run_with_home(&dir, &input, &ok).status.success(),
        "通常の日本語ファイル名は通す"
    );
    assert_is_clean_wav(&ok);

    let _ = std::fs::remove_dir_all(&dir);
}
