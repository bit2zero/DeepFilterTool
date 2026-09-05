//! DeepFilter 音声ノイズ除去ツール（コマンドライン版）。
//!
//! Windows / Linux / macOS で同じ動作をします。音声処理はすべてローカルで、
//! 通信は `setup` での公式ファイル取得のみです。
//!
//! Copyright (c) 2026 bit2zero
//! MIT ライセンス。詳細はリポジトリの LICENSE と NOTICE.md を参照してください。
//! ノイズ除去の実体は DeepFilterNet (c) 2021 Hendrik Schröter, MIT/Apache-2.0 です。

mod assets;
#[macro_use]
mod debug;
mod engine;
mod error;
mod setup;
mod sha256;
mod wave;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use error::{Error, Result};

const COPYRIGHT: &str = "\
Copyright (c) 2026 bit2zero — MIT ライセンス
ノイズ除去の実体は DeepFilterNet (c) 2021 Hendrik Schröter — MIT または Apache-2.0
";

const USAGE: &str = "\
DeepFilter 音声ノイズ除去ツール

使い方:
  deepfilter-tool <入力.wav> [オプション]     ノイズを除去する
  deepfilter-tool setup [オプション]          公式エンジンとモデルを導入する
  deepfilter-tool check                       導入状態を確認する
  deepfilter-tool manifest                    固定版の一覧を JSON で出力する
  deepfilter-tool license                     著作権とライセンスを表示する

ノイズ除去のオプション:
  -o, --output <ファイル>   出力先。既定は 入力名_clean.wav（入力と同じ場所）
  -a, --attenuation <1-100> 最大ノイズ抑制 dB。既定は 100（実質制限なし）
      --pf                  強めの除去（ポストフィルター）を有効にする
      --force               出力先が既にある場合に上書きする
      --keep-session        作業フォルダー sessions/... を残す
      --engine <ファイル>   エンジンの場所を指定する
      --model <ファイル>    モデルの場所を指定する
  -q, --quiet               処理中のメッセージを出さない

setup のオプション:
      --platform <キー>     導入対象。既定は実行中の環境
      --force               既存ファイルを固定版で入れ替える

共通:
  -h, --help                この使い方を表示する
  -V, --version             バージョンを表示する
      --debug               詳細なログを標準エラーへ出す。環境変数 DEEPFILTER_DEBUG=1
                            でも有効になる。ノイズ除去では公式エンジンにも -v を渡し、
                            作業フォルダーを残す（--quiet と併用できる）

使用例:
  deepfilter-tool setup                       最初に一度だけ。エンジンとモデルを導入する
  deepfilter-tool 会議の録音.wav               会議の録音_clean.wav を作る
  deepfilter-tool 録音.wav -o きれい.wav       出力先を指定する
  deepfilter-tool 録音.wav -a 60 --pf         抑制を 60 dB に抑え、強めの除去を使う
  deepfilter-tool 録音.wav --force            既にある出力先を上書きする
  deepfilter-tool 録音.wav --debug            うまくいかないときに詳細ログを見る
  deepfilter-tool check                       導入状態と SHA-256 を確認する

対応する入力: 48 kHz、モノラル/ステレオ、PCM 16bit または IEEE Float 32bit の WAV。
出力は再生互換性のため 48 kHz PCM 16bit です。元のファイルは変更しません。
ファイル名・フォルダー名に日本語や空白を含んでいても、そのまま扱えます。

作業フォルダーと保存先:
  runtime/  … 公式エンジンとモデル。setup が置く
  sessions/ … 処理の作業フォルダー。既定では成功後に削除する
  どちらも実行ファイルの位置から探します。環境変数 DEEPFILTER_HOME で変更できます。
";

fn main() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<()> {
    // args() は UTF-8 でない引数でパニックするため args_os() を使う。
    // ファイル名の文字コードは OS ごとに異なり、UTF-8 とは限らない。
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    // --debug はどのサブコマンドでも使えるよう、位置に関係なく先に見る。
    if debug::enabled_by_environment() || args.iter().any(|a| a == "--debug") {
        debug::enable();
        dlog!(
            "deepfilter-tool {} (DeepFilterNet {})",
            env!("CARGO_PKG_VERSION"),
            assets::RELEASE
        );
        dlog!(
            "実行環境: {} / {}",
            engine::platform_key().unwrap_or("未対応の環境"),
            std::env::consts::OS
        );
        let shown: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        dlog!("引数    : {:?}", shown);
    }
    if args.is_empty() {
        print!("{}", USAGE);
        return Ok(());
    }
    // サブコマンド名とオプション名は ASCII。UTF-8 でなければ該当なしとして扱う。
    match args[0].to_str().unwrap_or_default() {
        "-h" | "--help" | "help" => {
            print!("{}", USAGE);
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!(
                "deepfilter-tool {} (DeepFilterNet {})",
                env!("CARGO_PKG_VERSION"),
                assets::RELEASE
            );
            print!("{}", COPYRIGHT);
            Ok(())
        }
        "license" | "--license" => {
            print!("{}", COPYRIGHT);
            println!();
            println!("ライセンス全文: LICENSE / runtime/LICENSE-MIT.txt");
            println!("第三者ソフトウェアの表示: NOTICE.md");
            Ok(())
        }
        "manifest" => {
            print!("{}", assets::manifest_json());
            Ok(())
        }
        "setup" => setup_command(&args[1..]),
        "check" => check_command(),
        "filter" => filter_command(&args[1..]),
        // サブコマンド名がなければノイズ除去。オプションが先頭に来ても受け付ける。
        _ => filter_command(&args),
    }
}

/// 値をそのまま（文字コードを変換せずに）取り出す。ファイル名はここを通る。
fn value_of(args: &[OsString], index: &mut usize, flag: &str) -> Result<OsString> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| Error::new(format!("{} には値が必要です。", flag)))
}

/// 数値やプラットフォーム名など、UTF-8 であることが前提の値を取り出す。
fn text_value_of(args: &[OsString], index: &mut usize, flag: &str) -> Result<String> {
    let raw = value_of(args, index, flag)?;
    raw.into_string().map_err(|bad| {
        Error::new(format!(
            "{} の値を解釈できません: {}",
            flag,
            bad.to_string_lossy()
        ))
    })
}

fn setup_command(args: &[OsString]) -> Result<()> {
    let mut platform: Option<String> = None;
    let mut force = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].to_str().unwrap_or_default() {
            "--platform" => platform = Some(text_value_of(args, &mut i, "--platform")?),
            "--force" => force = true,
            // 先に処理済み。ここでは受け付けるだけ。
            "--debug" => {}
            "-h" | "--help" => {
                print!("{}", USAGE);
                return Ok(());
            }
            _ => {
                return Err(Error::new(format!(
                    "setup で不明な引数 {} が指定されました。",
                    args[i].to_string_lossy()
                )))
            }
        }
        i += 1;
    }
    let platform = match platform {
        Some(p) => p,
        None => engine::platform_key()?.to_string(),
    };
    setup::run(&engine::find_root(), &platform, force)
}

fn check_command() -> Result<()> {
    let root = engine::find_root();
    let engine_file = engine::engine_path(&root);
    let model_file = engine::model_path(&root);
    println!("作業フォルダー  : {}", root.display());
    println!(
        "プラットフォーム: {}",
        engine::platform_key()
            .map(|s| s.to_string())
            .unwrap_or_else(|e| e.0)
    );
    // 日本語は全角のため、桁揃えは書式指定ではなく空白込みのラベルで行う。
    report(&engine_file, "エンジン        ");
    report(&model_file, "モデル          ");
    engine::check_runtime(&engine_file, &model_file)?;
    println!("結果            : 利用できます。");
    Ok(())
}

fn report(path: &Path, label: &str) {
    if !path.is_file() {
        println!("{}: 未導入 ({})", label, path.display());
        return;
    }
    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
    let digest = sha256::file_hex(path).unwrap_or_else(|_| "(読めません)".into());
    let known = assets::SHARED
        .iter()
        .map(|a| a.sha256)
        .chain(assets::ENGINES.iter().map(|(_, a)| a.sha256))
        .any(|s| s == digest);
    println!("{}: {} ({} バイト)", label, path.display(), size);
    println!(
        "                  sha256 {} {}",
        digest,
        if known {
            "[固定版と一致]"
        } else {
            "[固定版と不一致]"
        }
    );
}

fn filter_command(args: &[OsString]) -> Result<()> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut attenuation: u32 = 100;
    let mut post_filter = false;
    let mut force = false;
    let mut keep_session = false;
    let mut quiet = false;
    let mut engine_override: Option<PathBuf> = None;
    let mut model_override: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].to_str().unwrap_or_default() {
            "-o" | "--output" => output = Some(PathBuf::from(value_of(args, &mut i, "--output")?)),
            "-a" | "--attenuation" => {
                let raw = text_value_of(args, &mut i, "--attenuation")?;
                attenuation = raw
                    .parse()
                    .ok()
                    .filter(|v| (1..=100).contains(v))
                    .ok_or_else(|| {
                        Error::new(format!(
                            "最大ノイズ抑制は 1〜100 dB で指定してください: {}",
                            raw
                        ))
                    })?;
            }
            "--pf" | "--post-filter" => post_filter = true,
            "--force" => force = true,
            // 先に処理済み。ここでは受け付けるだけ。
            "--debug" => {}
            "--keep-session" => keep_session = true,
            "-q" | "--quiet" => quiet = true,
            "--engine" => {
                engine_override = Some(PathBuf::from(value_of(args, &mut i, "--engine")?))
            }
            "--model" => model_override = Some(PathBuf::from(value_of(args, &mut i, "--model")?)),
            "-h" | "--help" => {
                print!("{}", USAGE);
                return Ok(());
            }
            // オプション名は必ず ASCII なので、UTF-8 でない引数はファイル名として扱う。
            other if other.starts_with('-') => {
                return Err(Error::new(format!("不明なオプション {} です。", other)))
            }
            _ => {
                if input.is_some() {
                    return Err(Error::new("入力ファイルは 1 つだけ指定してください。"));
                }
                input = Some(PathBuf::from(&args[i]));
            }
        }
        i += 1;
    }

    let input = input.ok_or_else(|| Error::new("入力の WAV を指定してください。"))?;
    if !input.is_file() {
        return Err(Error::new(format!(
            "入力が見つかりません: {}",
            input.display()
        )));
    }

    let output = match output {
        Some(path) => path,
        None => default_output(&input)?,
    };
    check_output_name(&output)?;
    if same_file(&input, &output) {
        return Err(Error::new("元ファイルとは別の名前を指定してください。"));
    }
    if output.exists() && !force {
        return Err(Error::new(format!(
            "出力先が既にあります: {}\n上書きする場合は --force を付けてください。",
            output.display()
        )));
    }

    // --debug のときは、後から調べられるよう作業フォルダーを残す。
    let keep_session = keep_session || debug::enabled();

    let root = engine::find_root();
    dlog!("作業フォルダーの基点: {}", root.display());
    let engine_file = engine_override.unwrap_or_else(|| engine::engine_path(&root));
    let model_file = model_override.unwrap_or_else(|| engine::model_path(&root));
    // 作業フォルダーを作る前に導入状態を確かめ、失敗時に空フォルダーを残さない。
    engine::check_runtime(&engine_file, &model_file)?;
    let session = engine::new_session(&root)?;
    dlog!("入力    : {}", input.display());
    dlog!("出力    : {}", output.display());
    dlog!(
        "設定    : 抑制 {} dB / ポストフィルター {} / 上書き {} / 作業フォルダー保持 {}",
        attenuation,
        post_filter,
        force,
        keep_session
    );

    if !quiet {
        println!("入力     : {}", input.display());
        println!(
            "設定     : 最大 {} dB{}",
            attenuation,
            if post_filter {
                " / ポストフィルター有効"
            } else {
                ""
            }
        );
        println!("処理中...（元のファイルは変更しません）");
    }

    let job = engine::Job {
        input: &input,
        engine: &engine_file,
        model: &model_file,
        session: &session,
        attenuation,
        post_filter,
        verbose: debug::enabled(),
    };
    let outcome = match engine::run(&job) {
        Ok(outcome) => outcome,
        Err(e) => {
            // 何も書けていなければ空フォルダーを残さない。ログがあるときだけ場所を伝える。
            let empty = std::fs::read_dir(&session)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if empty {
                let _ = std::fs::remove_dir(&session);
            } else {
                eprintln!("作業フォルダーを残しました: {}", session.display());
            }
            return Err(e);
        }
    };

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if output.exists() {
        std::fs::remove_file(&output)?;
    }
    // 別ボリュームでも動くよう、rename が使えなければコピーで置く。
    if std::fs::rename(&outcome.result, &output).is_err() {
        dlog!("移動できないためコピーで配置します（別ボリュームの可能性）");
        std::fs::copy(&outcome.result, &output)?;
    } else {
        dlog!("移動で配置しました");
    }

    if keep_session {
        if !quiet {
            println!("作業フォルダー: {}", session.display());
            println!("エンジンログ  : {}", outcome.log.display());
        }
    } else {
        let _ = std::fs::remove_dir_all(&session);
    }

    if !quiet {
        let seconds = outcome.frames as f64 / 48_000.0;
        println!(
            "完了     : {} （{:.2} 秒 / {} ch / 48 kHz PCM 16bit）",
            output.display(),
            seconds,
            outcome.channels
        );
    }
    Ok(())
}

/// Windows が特別扱いするデバイス名。拡張子を付けても同じ扱いになる。
///
/// これらに書き込むと内容がどこにも残らないまま成功したように見えるため、
/// 出力先として指定された時点で断る。判定はどの OS でも同じにして、
/// Windows で開けないファイル名を他の OS で作ってしまわないようにする。
const WINDOWS_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 出力先がどの OS でも素直に作れる名前かを確かめる。
///
/// 日本語などのマルチバイト文字はそのまま通す。弾くのは、Windows で
/// 意図しない挙動になる名前だけ。
fn check_output_name(output: &Path) -> Result<()> {
    let name = match output.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => return Err(Error::new("出力先がファイル名になっていません。")),
    };

    let stem = name.split('.').next().unwrap_or("");
    if WINDOWS_DEVICE_NAMES
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device))
    {
        return Err(Error::new(format!(
            "{} は Windows の予約名のため出力先にできません。別の名前にしてください。",
            name
        )));
    }
    // Windows は末尾の空白と点を黙って落とすため、別名のファイルができてしまう。
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(Error::new(format!(
            "ファイル名の末尾に空白や「.」は使えません: {}",
            name
        )));
    }
    // Windows のファイル名に使えない文字。他の OS で作ると持ち運べなくなる。
    if let Some(bad) = name
        .chars()
        .find(|c| "<>:\"|?*".contains(*c) || (*c as u32) < 0x20)
    {
        return Err(Error::new(format!(
            "ファイル名に使えない文字が含まれています（{:?}）: {}",
            bad, name
        )));
    }
    Ok(())
}

fn default_output(input: &Path) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .ok_or_else(|| Error::new("入力のファイル名を判別できません。"))?;
    let mut name = stem.to_os_string();
    name.push("_clean.wav");
    Ok(input.with_file_name(name))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}
