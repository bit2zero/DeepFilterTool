//! `--debug` の出力内容を、別プロセスとして起動して確認する。

mod common;

use common::{engine_file, model_file, repo_root, skip_unless_ready, write_test_wav, BIN};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FRAMES: usize = 4_800;

fn work_dir(name: &str) -> PathBuf {
    common::work_dir("dbg", name)
}

/// 1 回処理して、その出力と生成物の場所を返す。
fn filter(dir: &Path, extra: &[&str], debug_env: Option<&str>) -> (Output, PathBuf) {
    let input = dir.join("in.wav");
    write_test_wav(&input, 1, FRAMES);
    let output = dir.join("out.wav");
    let mut command = Command::new(BIN);
    command
        .env("DEEPFILTER_HOME", dir)
        .env_remove("DEEPFILTER_DEBUG")
        .arg("--engine")
        .arg(engine_file())
        .arg("--model")
        .arg(model_file())
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args(extra);
    if let Some(value) = debug_env {
        command.env("DEEPFILTER_DEBUG", value);
    }
    (command.output().expect("起動"), output)
}

#[test]
fn debug_output_traces_the_whole_pipeline() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("pipeline");
    let (out, output) = filter(&dir, &["--debug", "-q"], None);
    assert!(
        out.status.success(),
        "--debug 付きで失敗: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(output.is_file(), "出力は通常どおり作られる");

    let log = String::from_utf8_lossy(&out.stderr);
    for expected in [
        "deepfilter-tool",
        "実行環境",
        "引数",
        "作業フォルダーの基点",
        "入力WAV",
        "再生時間",
        "パディング",
        "中間ファイル",
        "実行     :",
        "エンジン終了",
        "エンジンの出力",
        "エンジン出力",
        "フレームへ切り詰め",
    ] {
        assert!(
            log.contains(expected),
            "ログに「{}」がある:\n{}",
            expected,
            log
        );
    }
    // 入力の性質が数値で分かること。
    assert!(log.contains("4800 フレーム"), "フレーム数を出す:\n{}", log);
    assert!(log.contains("48000 Hz"), "サンプルレートを出す");
    assert!(log.contains("0.100 秒"), "再生時間を出す");
    // ホップ境界への切り上げ + 末尾パディング。
    assert!(
        log.contains("4800 -> 9600 フレーム"),
        "パディング量を出す:\n{}",
        log
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn debug_lines_go_to_stderr_and_are_time_stamped() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("stderr");
    let (out, _) = filter(&dir, &["--debug", "-q"], None);
    assert!(out.status.success());

    // -q と併用しているので標準出力は空のまま。
    assert!(out.stdout.is_empty(), "標準出力は汚さない");
    let log = String::from_utf8_lossy(&out.stderr);
    let lines: Vec<&str> = log.lines().collect();
    assert!(lines.len() > 8, "十分な行数がある: {}", lines.len());
    for line in &lines {
        assert!(
            line.starts_with("[debug"),
            "すべての行に接頭辞が付く: {:?}",
            line
        );
    }
    assert!(lines[0].contains("s] "), "経過時間が付く: {:?}", lines[0]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn debug_is_silent_unless_requested() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("silent");
    let (out, _) = filter(&dir, &["-q"], None);
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "-q なので標準出力は空");
    assert!(
        out.stderr.is_empty(),
        "--debug なしでは標準エラーも空: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_environment_variable_turns_debug_on() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("env");
    let (out, _) = filter(&dir, &["-q"], Some("1"));
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("[debug"),
        "DEEPFILTER_DEBUG=1 で有効になる"
    );

    let dir_off = work_dir("env-off");
    let (off, _) = filter(&dir_off, &["-q"], Some("0"));
    assert!(off.status.success());
    assert!(off.stderr.is_empty(), "DEEPFILTER_DEBUG=0 では無効のまま");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir_off);
}

#[test]
fn debug_keeps_the_work_folder_for_inspection() {
    if skip_unless_ready() {
        return;
    }
    let dir = work_dir("keep");
    let (out, _) = filter(&dir, &["--debug", "-q"], None);
    assert!(out.status.success());

    let sessions: Vec<PathBuf> = std::fs::read_dir(dir.join("sessions"))
        .expect("sessions が作られる")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(sessions.len(), 1, "調査できるよう作業フォルダーを残す");
    assert!(
        sessions[0].join("engine.log").is_file(),
        "エンジンログが残る"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn debug_reports_why_a_failure_happened() {
    let dir = work_dir("failure");
    let input = dir.join("in.wav");
    write_test_wav(&input, 1, FRAMES);

    let out = Command::new(BIN)
        .env("DEEPFILTER_HOME", &dir)
        .arg("--debug")
        .arg("--engine")
        .arg(dir.join("存在しないエンジン"))
        .arg("--model")
        .arg(model_file())
        .arg(&input)
        .arg("-o")
        .arg(dir.join("out.wav"))
        .arg("-q")
        .output()
        .expect("起動");
    assert!(!out.status.success());
    let log = String::from_utf8_lossy(&out.stderr);
    assert!(log.contains("[debug"), "失敗時もログが出る");
    assert!(log.contains("引数"), "渡された引数が分かる");
    assert!(log.contains("エラー: "), "エラー本文も出る:\n{}", log);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn debug_works_for_setup_and_check_too() {
    if skip_unless_ready() {
        return;
    }
    let check = Command::new(BIN)
        .env("DEEPFILTER_HOME", repo_root())
        .env_remove("DEEPFILTER_DEBUG")
        .args(["check", "--debug"])
        .output()
        .expect("起動");
    // check は --debug を引数として受け取らないが、先読みで有効になる。
    assert!(
        String::from_utf8_lossy(&check.stderr).contains("[debug"),
        "check でもログが出る"
    );

    let setup = Command::new(BIN)
        .env("DEEPFILTER_HOME", repo_root())
        .env_remove("DEEPFILTER_DEBUG")
        .args(["setup", "--debug"])
        .output()
        .expect("起動");
    assert!(setup.status.success(), "導入済みなので成功する");
    let log = String::from_utf8_lossy(&setup.stderr);
    assert!(log.contains("プラットフォーム"), "対象環境を出す");
    assert!(log.contains("導入対象"), "配置先を出す");
    assert!(log.contains("sha256 期待"), "照合の内訳を出す:\n{}", log);
    assert!(log.contains("sha256 実際"), "実際の値も出す");
}

#[test]
fn help_documents_the_debug_option() {
    let out = Command::new(BIN)
        .env_remove("DEEPFILTER_DEBUG")
        .arg("--help")
        .output()
        .expect("起動");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--debug"), "ヘルプに載っている");
    assert!(text.contains("DEEPFILTER_DEBUG"), "環境変数も案内する");
    assert!(text.contains("標準エラー"), "出力先を説明する");
}
