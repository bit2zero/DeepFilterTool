//! setup.rs の単体テスト。外部への通信は行わない。
//!
//! ダウンロード自体は取得ツールに任せているため、ここでは失敗経路だけを
//! 到達不能なローカルポート宛てで確認する。

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deepfilter-setup-tests-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 実ファイルの内容から、それに一致する Asset を組み立てる。
fn asset_for(path: &Path, file: &'static str) -> Asset {
    let digest = sha256::file_hex(path).unwrap();
    Asset {
        file,
        url: "https://127.0.0.1:1/使われない",
        bytes: path.metadata().unwrap().len(),
        sha256: Box::leak(digest.into_boxed_str()),
    }
}

#[test]
fn which_finds_a_program_on_path_and_reports_absent_ones() {
    let present = if cfg!(windows) { "cmd" } else { "sh" };
    let found = which(present).unwrap_or_else(|| panic!("{} が PATH にあること", present));
    assert!(found.is_file(), "実ファイルを返す: {}", found.display());
    assert!(
        which("deepfilter-definitely-not-installed-xyz").is_none(),
        "存在しないプログラムは None"
    );
}

#[test]
fn verify_accepts_a_matching_file() {
    let dir = scratch("verify-ok");
    let path = dir.join("payload.bin");
    fs::write(&path, b"DeepFilterNet3").unwrap();
    assert!(verify(&path, &asset_for(&path, "payload.bin")).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verify_rejects_a_size_mismatch() {
    let dir = scratch("verify-size");
    let path = dir.join("payload.bin");
    fs::write(&path, b"DeepFilterNet3").unwrap();
    let mut asset = asset_for(&path, "payload.bin");
    asset.bytes += 1;

    let err = verify(&path, &asset).unwrap_err();
    assert!(err.0.contains("サイズが一致しません"), "{}", err.0);
    assert!(
        err.0.contains("期待") && err.0.contains("実際"),
        "両方の値を示す"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verify_rejects_a_hash_mismatch() {
    let dir = scratch("verify-hash");
    let path = dir.join("payload.bin");
    fs::write(&path, b"DeepFilterNet3").unwrap();
    let mut asset = asset_for(&path, "payload.bin");
    // サイズは同じで中身だけ違う場合を作る。
    asset.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";

    let err = verify(&path, &asset).unwrap_err();
    assert!(err.0.contains("SHA-256 が一致しません"), "{}", err.0);
    assert!(
        err.0.contains(&sha256::file_hex(&path).unwrap()),
        "実際の値を示す"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_skips_a_file_that_already_matches() {
    let dir = scratch("install-skip");
    let target = dir.join("payload.bin");
    fs::write(&target, "すでに導入済み".as_bytes()).unwrap();
    let asset = asset_for(&target, "payload.bin");

    let installed = install(&dir, &asset, false, false).unwrap();
    assert!(!installed, "再取得しない");
    assert_eq!(
        fs::read(&target).unwrap(),
        "すでに導入済み".as_bytes(),
        "既存ファイルに触れない"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_refuses_to_replace_a_differing_file_without_force() {
    let dir = scratch("install-differs");
    let target = dir.join("payload.bin");
    fs::write(&target, "元の内容".as_bytes()).unwrap();
    let asset = asset_for(&target, "payload.bin");
    fs::write(&target, "別の内容".as_bytes()).unwrap();

    let err = install(&dir, &asset, false, false).unwrap_err();
    assert!(err.0.contains("--force"), "対処方法を伝える: {}", err.0);
    assert_eq!(
        fs::read(&target).unwrap(),
        "別の内容".as_bytes(),
        "拒否したときは既存ファイルを消さない"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn download_failure_leaves_no_partial_file() {
    let dir = scratch("download-fail");
    let dest = dir.join("partial.bin");
    // 到達不能なローカルポート。外部への通信は発生しない。
    let err = download("https://127.0.0.1:1/nothing", &dest).unwrap_err();
    assert!(
        err.0.contains("ダウンロードに失敗しました") || err.0.contains("curl か wget"),
        "失敗を伝える: {}",
        err.0
    );
    assert!(!dest.exists(), "途中まで書いたファイルを残さない");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_reports_a_download_failure_without_touching_the_target() {
    let dir = scratch("install-download-fail");
    let asset = Asset {
        file: "payload.bin",
        url: "https://127.0.0.1:1/nothing",
        bytes: 3,
        sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    };
    assert!(
        install(&dir, &asset, false, false).is_err(),
        "取得に失敗する"
    );
    assert!(!dir.join("payload.bin").exists(), "配置しない");
    assert!(
        !dir.join("payload.bin.download").exists(),
        "作業用ファイルも残さない"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_rejects_an_unknown_platform_and_lists_the_valid_ones() {
    let dir = scratch("run-platform");
    let err = run(&dir, "solaris-sparc", false).unwrap_err();
    assert!(err.0.contains("未知のプラットフォーム"), "{}", err.0);
    for key in assets::platforms() {
        assert!(err.0.contains(key), "選べるキー {} を案内する", key);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_is_idempotent_when_every_file_already_matches() {
    let dir = scratch("run-idempotent");
    let runtime = dir.join("runtime");
    fs::create_dir_all(&runtime).unwrap();

    // 実際の runtime/ が揃っている環境でのみ意味がある検査。
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let platform = match platform_key_for_tests() {
        Some(key) => key,
        None => return,
    };
    let engine = assets::engine_for(platform).unwrap();
    let mut present = true;
    for asset in assets::SHARED.iter().chain(std::iter::once(engine)) {
        let source = repo.join("runtime").join(asset.file);
        if source.is_file() && sha256::file_hex(&source).ok().as_deref() == Some(asset.sha256) {
            fs::copy(&source, runtime.join(asset.file)).unwrap();
        } else {
            present = false;
        }
    }
    if !present {
        eprintln!("スキップ: runtime/ に固定版が揃っていません。");
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    // すべて一致しているので通信は起きない。
    assert!(run(&dir, platform, false).is_ok(), "再実行しても成功する");
    let _ = fs::remove_dir_all(&dir);
}

fn platform_key_for_tests() -> Option<&'static str> {
    crate::engine::platform_key().ok()
}

#[cfg(unix)]
#[test]
fn make_executable_adds_the_execute_bit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("chmod");
    let path = dir.join("prog");
    fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    make_executable(&path).unwrap();
    let mode = path.metadata().unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "実行ビットが立つ: {:o}", mode);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clear_quarantine_is_safe_on_every_platform() {
    let dir = scratch("quarantine");
    let path = dir.join("prog");
    fs::write(&path, b"x").unwrap();
    // macOS 以外では何もしない。macOS でも属性がなければ失敗しない。
    clear_quarantine(&path);
    assert!(path.is_file(), "ファイルは残る");
    let _ = fs::remove_dir_all(&dir);
}

/// 実際に公式配布物を取得する経路の検査。既定では走らせない。
///
/// `DEEPFILTER_NETWORK_TESTS=1` を設定したときだけ実行し、一覧のうち
/// 最小のファイル（MIT ライセンス本文 1 KB）だけを取得する。
#[test]
fn installs_a_pinned_asset_over_the_network_when_opted_in() {
    if std::env::var_os("DEEPFILTER_NETWORK_TESTS").is_none() {
        eprintln!("スキップ: 通信を伴う検査です。DEEPFILTER_NETWORK_TESTS=1 で実行してください。");
        return;
    }
    let dir = scratch("network-install");
    let asset = assets::SHARED
        .iter()
        .min_by_key(|a| a.bytes)
        .expect("共通ファイルが 1 つ以上ある");

    let installed = install(&dir, asset, true, false).expect("取得と検証に成功する");
    assert!(installed, "未導入なら取得する");

    let target = dir.join(asset.file);
    assert!(target.is_file(), "runtime に配置される");
    assert_eq!(
        target.metadata().unwrap().len(),
        asset.bytes,
        "サイズが一致"
    );
    assert_eq!(
        sha256::file_hex(&target).unwrap(),
        asset.sha256,
        "SHA-256 が固定版と一致"
    );
    assert!(
        !dir.join(format!("{}.download", asset.file)).exists(),
        "作業用ファイルを残さない"
    );

    // 2 回目は取得せずに済ませる。
    assert!(!install(&dir, asset, true, false).unwrap(), "再取得しない");

    // --force なら同じ内容で取り直す。
    assert!(
        install(&dir, asset, true, true).unwrap(),
        "--force で取り直す"
    );
    assert_eq!(sha256::file_hex(&target).unwrap(), asset.sha256);

    let _ = fs::remove_dir_all(&dir);
}

/// 取得できても中身が固定版と違えば配置しないこと。改ざんや配布物差し替えの検知。
#[test]
fn rejects_downloaded_content_that_fails_verification() {
    if std::env::var_os("DEEPFILTER_NETWORK_TESTS").is_none() {
        eprintln!("スキップ: 通信を伴う検査です。DEEPFILTER_NETWORK_TESTS=1 で実行してください。");
        return;
    }
    let dir = scratch("network-tampered");
    let real = assets::SHARED.iter().min_by_key(|a| a.bytes).unwrap();
    let wrong = Asset {
        file: "tampered.txt",
        url: real.url,
        bytes: real.bytes,
        sha256: "1111111111111111111111111111111111111111111111111111111111111111",
    };

    let err = install(&dir, &wrong, false, false).unwrap_err();
    assert!(err.0.contains("SHA-256 が一致しません"), "{}", err.0);
    assert!(!dir.join("tampered.txt").exists(), "配置しない");
    assert!(
        !dir.join("tampered.txt.download").exists(),
        "取得したファイルを破棄する"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 一部だけ導入済みの状態から、足りないものだけを取得すること。
#[test]
fn run_fills_in_only_the_missing_files() {
    if std::env::var_os("DEEPFILTER_NETWORK_TESTS").is_none() {
        eprintln!("スキップ: 通信を伴う検査です。DEEPFILTER_NETWORK_TESTS=1 で実行してください。");
        return;
    }
    let platform = match platform_key_for_tests() {
        Some(key) => key,
        None => return,
    };
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let engine = assets::engine_for(platform).unwrap();
    // 一覧のうち最小のものだけを欠けさせ、残りは実物を置く。
    let missing = assets::SHARED.iter().min_by_key(|a| a.bytes).unwrap();
    let mut sources: Vec<&Asset> = assets::SHARED
        .iter()
        .chain(std::iter::once(engine))
        .collect();
    sources.retain(|a| a.file != missing.file);
    if sources.iter().any(|a| {
        sha256::file_hex(&repo.join("runtime").join(a.file))
            .ok()
            .as_deref()
            != Some(a.sha256)
    }) {
        eprintln!("スキップ: runtime/ に固定版が揃っていません。");
        return;
    }

    let dir = scratch("network-partial");
    let runtime = dir.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    for asset in &sources {
        fs::copy(
            repo.join("runtime").join(asset.file),
            runtime.join(asset.file),
        )
        .unwrap();
    }

    run(&dir, platform, false).expect("足りないものだけ取得して成功する");
    let filled = runtime.join(missing.file);
    assert!(filled.is_file(), "欠けていたファイルが導入される");
    assert_eq!(
        sha256::file_hex(&filled).unwrap(),
        missing.sha256,
        "固定版と一致"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn powershell_quoting_closes_the_injection_hole() {
    assert_eq!(powershell_quote("plain"), "'plain'");
    // 単引用符を含むパスでも文字列から抜け出さない。
    assert_eq!(
        powershell_quote(r"C:\Users\o'brien\runtime\x.download"),
        r"'C:\Users\o''brien\runtime\x.download'"
    );
    // 実際に注入を狙う形。閉じ引用符が literal 化され、コマンドが分離されない。
    let hostile = r"C:\a'; Start-Process calc; '";
    let quoted = powershell_quote(hostile);
    assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
    // 開始と終了以外に、単独の単引用符が残っていないこと。
    let inner = &quoted[1..quoted.len() - 1];
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            assert_eq!(
                chars.next(),
                Some('\''),
                "単引用符は必ず 2 個組: {}",
                quoted
            );
        }
    }
}

#[test]
fn download_refuses_anything_that_is_not_https() {
    let dir = scratch("scheme");
    for url in [
        "http://example.invalid/x",
        "ftp://example.invalid/x",
        "file:///etc/passwd",
        "HTTPS://example.invalid/x",
    ] {
        let err = download(url, &dir.join("out.bin")).unwrap_err();
        assert!(
            err.0.contains("HTTPS 以外は取得しません"),
            "{} を拒否する: {}",
            url,
            err.0
        );
    }
    assert!(!dir.join("out.bin").exists(), "何も作らない");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn every_pinned_url_is_https() {
    for asset in assets::SHARED
        .iter()
        .chain(assets::ENGINES.iter().map(|(_, a)| a))
    {
        assert!(
            asset.url.starts_with("https://"),
            "{} が HTTPS でない: {}",
            asset.file,
            asset.url
        );
    }
}
