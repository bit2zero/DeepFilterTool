//! engine.rs の単体テスト。公式エンジンの代わりに、意図した終了コードを返す
//! ごく小さな実行ファイルを作って失敗経路まで検査する。

use super::*;
use std::time::Duration;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deepfilter-engine-tests-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 48 kHz PCM16 の最小 WAV を書く。
fn write_wav(path: &Path, frames: usize) {
    let data = vec![0x20u8; frames * 2];
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
    fs::write(path, bytes).unwrap();
}

/// 公式エンジンの代役。`body` の中身を実行して終了する。
#[cfg(unix)]
fn fake_engine(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fake-engine");
    fs::write(&path, format!("#!/bin/sh\n{}\n", body)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(windows)]
fn fake_engine(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("fake-engine.cmd");
    fs::write(&path, format!("@echo off\r\n{}\r\n", body)).unwrap();
    path
}

fn dummy_model(dir: &Path) -> PathBuf {
    let path = dir.join("model.tar.gz");
    fs::write(&path, b"not a real model").unwrap();
    path
}

#[test]
fn platform_key_names_the_running_environment() {
    let key = platform_key().expect("対応環境で解決できること");
    assert!(
        crate::assets::platforms().contains(&key),
        "manifest に載っているキーであること: {}",
        key
    );
    let (os, arch) = key.split_once('-').expect("os-arch 形式");
    assert!(["windows", "linux", "macos"].contains(&os));
    assert!(["x86_64", "aarch64"].contains(&arch));
}

#[test]
fn engine_and_model_paths_live_under_runtime() {
    let root = Path::new("/some/root");
    assert_eq!(engine_path(root), root.join("runtime").join(ENGINE_FILE));
    assert_eq!(model_path(root), root.join("runtime").join(MODEL_FILE));
    assert!(ENGINE_FILE.starts_with("deep-filter"));
}

#[test]
fn choose_root_prefers_a_candidate_that_holds_the_engine() {
    let dir = scratch("root");
    let empty = dir.join("empty");
    let only_dir = dir.join("only-runtime-dir");
    let with_engine = dir.join("with-engine");
    fs::create_dir_all(&empty).unwrap();
    fs::create_dir_all(only_dir.join("runtime")).unwrap();
    fs::create_dir_all(with_engine.join("runtime")).unwrap();
    fs::write(with_engine.join("runtime").join(ENGINE_FILE), b"x").unwrap();

    // エンジンを持つ候補が後ろにあっても選ばれる。
    let picked = choose_root(vec![empty.clone(), only_dir.clone(), with_engine.clone()]);
    assert_eq!(picked, with_engine, "エンジンを持つ候補が最優先");

    // エンジンがなければ runtime/ を持つ候補。
    assert_eq!(
        choose_root(vec![empty.clone(), only_dir.clone()]),
        only_dir,
        "runtime/ がある候補が次点"
    );

    // どちらもなければ先頭。
    assert_eq!(choose_root(vec![empty.clone()]), empty, "該当なしなら先頭");
    assert_eq!(
        choose_root(Vec::new()),
        PathBuf::from("."),
        "候補が空なら ."
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_candidates_starts_at_the_executable() {
    let candidates = search_candidates();
    assert!(!candidates.is_empty(), "候補が必ず 1 つ以上ある");
    let exe_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(candidates[0], exe_dir, "先頭は実行ファイルのフォルダー");
    assert!(candidates.len() <= 5, "実行ファイルから 4 階層 + cwd まで");
}

#[test]
fn utc_stamp_matches_known_instants() {
    let at = |secs: u64| utc_stamp(&(UNIX_EPOCH + Duration::from_secs(secs)));
    assert_eq!(at(0), "19700101-000000", "エポック");
    assert_eq!(at(1_000_000_000), "20010909-014640");
    assert_eq!(at(1_700_000_000), "20231114-221320");
    // うるう年の 2 月 29 日をまたぐこと。
    assert_eq!(at(1_709_164_800), "20240229-000000");
    assert_eq!(at(951_782_400), "20000229-000000", "2000 年はうるう年");
    // 2100 年は 400 で割り切れないためうるう年ではない。2/28 の翌日は 3/1。
    assert_eq!(at(4_107_456_000), "21000228-000000");
    assert_eq!(at(4_107_542_400), "21000301-000000");
}

#[test]
fn session_id_is_eight_hex_digits() {
    let now = SystemTime::now();
    let id = session_id(&now);
    assert_eq!(id.len(), 8);
    assert!(
        id.chars().all(|c| c.is_ascii_hexdigit()),
        "16 進数のみ: {}",
        id
    );
}

#[test]
fn new_session_creates_a_uniquely_named_folder() {
    let dir = scratch("session");
    let a = new_session(&dir).unwrap();
    let b = new_session(&dir).unwrap();
    assert!(a.is_dir() && b.is_dir(), "どちらも作られる");
    assert_ne!(a, b, "同じ秒でも名前が衝突しない");
    assert!(a.starts_with(dir.join("sessions")), "sessions/ の下に作る");
    let name = a.file_name().unwrap().to_str().unwrap();
    assert_eq!(name.len(), "yyyymmdd-hhmmss".len() + 1 + 8, "日時-ID 形式");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn check_runtime_explains_what_is_missing() {
    let dir = scratch("check");
    let engine = fake_engine(&dir, "exit 0");
    let model = dummy_model(&dir);

    let missing_engine = check_runtime(&dir.join("nope"), &model).unwrap_err();
    assert!(
        missing_engine.0.contains("エンジンが見つかりません") && missing_engine.0.contains("setup"),
        "導入方法まで伝える: {}",
        missing_engine.0
    );

    let missing_model = check_runtime(&engine, &dir.join("nope.tar.gz")).unwrap_err();
    assert!(
        missing_model.0.contains("モデルが見つかりません"),
        "モデル不足を伝える: {}",
        missing_model.0
    );

    assert!(check_runtime(&engine, &model).is_ok(), "両方あれば通る");
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn check_runtime_reports_a_missing_execute_bit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("perm");
    let engine = fake_engine(&dir, "exit 0");
    let model = dummy_model(&dir);
    fs::set_permissions(&engine, fs::Permissions::from_mode(0o644)).unwrap();

    let err = check_runtime(&engine, &model).unwrap_err();
    assert!(
        err.0.contains("実行権限") && err.0.contains("chmod +x"),
        "対処方法を伝える: {}",
        err.0
    );
    let _ = fs::remove_dir_all(&dir);
}

fn job_in<'a>(
    dir: &'a Path,
    engine: &'a Path,
    model: &'a Path,
    input: &'a Path,
    session: &'a Path,
    attenuation: u32,
) -> Job<'a> {
    let _ = dir;
    Job {
        input,
        engine,
        model,
        session,
        attenuation,
        post_filter: false,
        verbose: false,
    }
}

#[test]
fn run_rejects_an_attenuation_outside_the_allowed_range() {
    let dir = scratch("range");
    let engine = fake_engine(&dir, "exit 0");
    let model = dummy_model(&dir);
    let input = dir.join("in.wav");
    write_wav(&input, 480);
    let session = new_session(&dir).unwrap();

    for bad in [0u32, 101, 10_000] {
        let job = job_in(&dir, &engine, &model, &input, &session, bad);
        let err = run(&job).unwrap_err();
        assert!(
            err.0.contains("1〜100"),
            "許容範囲を伝える ({}): {}",
            bad,
            err.0
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_reports_the_engine_exit_code_and_keeps_the_log() {
    let dir = scratch("exitcode");
    let engine = fake_engine(&dir, "echo 何かの出力 && exit 3");
    let model = dummy_model(&dir);
    let input = dir.join("in.wav");
    write_wav(&input, 960);
    let session = new_session(&dir).unwrap();

    let job = job_in(&dir, &engine, &model, &input, &session, 100);
    let err = run(&job).unwrap_err();
    assert!(
        err.0.contains("エンジン終了コード 3"),
        "終了コードを伝える: {}",
        err.0
    );
    let log = session.join("engine.log");
    assert!(
        err.0.contains(&log.display().to_string()),
        "ログの場所を伝える"
    );
    assert!(log.is_file(), "ログを残す");
    assert!(
        fs::read_to_string(&log).unwrap().contains("何かの出力"),
        "エンジンの出力を記録する"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_pads_the_staged_input_to_a_whole_hop_plus_tail() {
    let dir = scratch("padding");
    // 成功扱いで終了するが出力は書かないエンジン。中間ファイルだけを検査する。
    let engine = fake_engine(&dir, "exit 0");
    let model = dummy_model(&dir);
    let frames = 1_000usize;
    let input = dir.join("in.wav");
    write_wav(&input, frames);
    let session = new_session(&dir).unwrap();

    let job = job_in(&dir, &engine, &model, &input, &session, 100);
    let err = run(&job).unwrap_err();
    assert!(
        err.0.contains("WAV を開けません") || err.0.contains("音声データがありません"),
        "出力がなければ失敗として報告する: {}",
        err.0
    );

    let staged = Wave::read(&session.join("input.wav")).unwrap();
    let expected = frames.div_ceil(HOP) * HOP + TAIL_PAD;
    assert_eq!(staged.frames(), expected, "ホップ境界 + 末尾パディング");
    assert!(
        staged.data[frames * 2..].iter().all(|b| *b == 0),
        "追加部分は無音"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_reports_a_missing_engine_before_creating_any_output() {
    let dir = scratch("noengine");
    let model = dummy_model(&dir);
    let input = dir.join("in.wav");
    write_wav(&input, 480);
    let session = new_session(&dir).unwrap();

    let missing = dir.join("not-installed");
    let job = job_in(&dir, &missing, &model, &input, &session, 100);
    let err = run(&job).unwrap_err();
    assert!(err.0.contains("エンジンが見つかりません"), "{}", err.0);
    assert!(
        !session.join("input.wav").exists(),
        "検査に落ちたら中間ファイルも作らない"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_rejects_an_input_that_is_not_a_supported_wav() {
    let dir = scratch("badinput");
    let engine = fake_engine(&dir, "exit 0");
    let model = dummy_model(&dir);
    let input = dir.join("in.wav");
    fs::write(&input, vec![9u8; 512]).unwrap();
    let session = new_session(&dir).unwrap();

    let job = job_in(&dir, &engine, &model, &input, &session, 100);
    assert!(run(&job).is_err(), "非 WAV を拒否");
    let _ = fs::remove_dir_all(&dir);
}

/// カレントディレクトリを探索対象に含めると、細工した runtime/deep-filter を
/// 置いたフォルダーで実行するだけで任意のプログラムが起動してしまう。
#[test]
fn the_current_directory_is_never_part_of_the_search_path() {
    let candidates = search_candidates();
    let cwd = std::env::current_dir().unwrap();
    let exe_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // 実行ファイルの位置がたまたま cwd と一致する場合を除き、cwd は候補に入らない。
    if cwd != exe_dir && !exe_dir.starts_with(&cwd) {
        assert!(
            !candidates.contains(&cwd),
            "カレントディレクトリが探索対象に入っている: {:?}",
            candidates
        );
    }
    // 候補はすべて実行ファイルの位置か、その上位フォルダーであること。
    for candidate in &candidates {
        assert!(
            exe_dir.starts_with(candidate),
            "{} は実行ファイルの位置の上位ではない",
            candidate.display()
        );
    }
}
