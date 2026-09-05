//! DeepFilterNet3 公式 CLI の起動と、その前後のパディング/切り詰め処理。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Context, Error, Result};
use crate::wave::Wave;

/// モデルの先読みと端数ホップを吐き出させるための末尾パディング。
const HOP: usize = 480;
const TAIL_PAD: usize = 4800;

pub const ENGINE_FILE: &str = if cfg!(windows) {
    "deep-filter.exe"
} else {
    "deep-filter"
};
pub const MODEL_FILE: &str = "DeepFilterNet3_onnx.tar.gz";

/// manifest のエンジンキー（例: linux-x86_64）。
pub fn platform_key() -> Result<&'static str> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        return Err(Error::new("未対応の OS です。"));
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(Error::new("未対応の CPU アーキテクチャです。"));
    };
    Ok(match (os, arch) {
        ("windows", "x86_64") => "windows-x86_64",
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        ("macos", "aarch64") => "macos-aarch64",
        _ => {
            return Err(Error::new(
                "この OS と CPU の組み合わせ用の公式バイナリはありません。",
            ))
        }
    })
}

/// runtime/ と sessions/ を置くフォルダーを決める。
///
/// 環境変数 DEEPFILTER_HOME か、実行ファイルの位置（cargo の target/ 配下も遡る）
/// から探す。見つからなければ実行ファイルの位置。
///
/// カレントディレクトリは**意図的に探索対象から外している**。ここを含めると、
/// 細工した `runtime/deep-filter` を仕込んだフォルダーに移動して実行するだけで
/// 任意のプログラムが起動してしまう。置き場所を変えたいときは DEEPFILTER_HOME
/// で明示する。
pub fn find_root() -> PathBuf {
    if let Some(home) = std::env::var_os("DEEPFILTER_HOME") {
        return PathBuf::from(home);
    }
    choose_root(search_candidates())
}

/// 実行ファイルの位置から 4 階層上まで。
fn search_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let mut here = exe.parent();
        for _ in 0..4 {
            match here {
                Some(p) => {
                    candidates.push(p.to_path_buf());
                    here = p.parent();
                }
                None => break,
            }
        }
    }
    candidates
}

/// エンジンがある候補を最優先、次に runtime/ がある候補、どちらもなければ先頭。
fn choose_root(candidates: Vec<PathBuf>) -> PathBuf {
    for candidate in &candidates {
        if candidate.join("runtime").join(ENGINE_FILE).is_file() {
            return candidate.clone();
        }
    }
    for candidate in &candidates {
        if candidate.join("runtime").is_dir() {
            return candidate.clone();
        }
    }
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn engine_path(root: &Path) -> PathBuf {
    root.join("runtime").join(ENGINE_FILE)
}

pub fn model_path(root: &Path) -> PathBuf {
    root.join("runtime").join(MODEL_FILE)
}

/// エンジンとモデルが使える状態か調べ、問題があれば対処方法を添えて返す。
pub fn check_runtime(engine: &Path, model: &Path) -> Result<()> {
    if !engine.is_file() {
        return Err(Error::new(format!(
            "エンジンが見つかりません: {}\n`deepfilter-tool setup` を実行して導入してください。",
            engine.display()
        )));
    }
    if !model.is_file() {
        return Err(Error::new(format!(
            "モデルが見つかりません: {}\n`deepfilter-tool setup` を実行して導入してください。",
            model.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = engine.metadata()?.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(Error::new(format!(
                "エンジンに実行権限がありません。次を実行してください: chmod +x {}",
                engine.display()
            )));
        }
    }
    Ok(())
}

/// UTC の yyyyMMdd-HHmmss を、外部クレートなしで組み立てる。
fn utc_stamp(now: &SystemTime) -> String {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let time = secs.rem_euclid(86_400);
    // Howard Hinnant の civil_from_days。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year,
        m,
        d,
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

/// 衝突しにくい短い ID。乱数クレートを使わず時刻とプロセス ID から作る。
fn session_id(now: &SystemTime) -> String {
    let nanos = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as u64;
    let mut state = nanos ^ (std::process::id() as u64) << 32;
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    format!("{:08x}", state as u32)
}

pub fn new_session(root: &Path) -> Result<PathBuf> {
    let now = SystemTime::now();
    let dir = root
        .join("sessions")
        .join(format!("{}-{}", utc_stamp(&now), session_id(&now)));
    fs::create_dir_all(&dir).context(format!("作業フォルダーを作れません: {}", dir.display()))?;
    Ok(dir)
}

pub struct Job<'a> {
    pub input: &'a Path,
    pub engine: &'a Path,
    pub model: &'a Path,
    pub session: &'a Path,
    pub attenuation: u32,
    pub post_filter: bool,
    /// 公式エンジンにも -v を渡し、その出力を画面に流す。
    pub verbose: bool,
}

#[derive(Debug)]
pub struct Outcome {
    /// セッション内の clean.wav。
    pub result: PathBuf,
    pub log: PathBuf,
    pub frames: usize,
    pub channels: u16,
}

/// 入力を検証してパディングし、公式エンジンを実行し、元の長さへ切り詰める。
pub fn run(job: &Job) -> Result<Outcome> {
    if !(1..=100).contains(&job.attenuation) {
        return Err(Error::new(
            "最大ノイズ抑制は 1〜100 dB で指定してください。",
        ));
    }
    check_runtime(job.engine, job.model)?;

    crate::dlog!("エンジン: {}", job.engine.display());
    crate::dlog!("モデル  : {}", job.model.display());

    let source = Wave::read(job.input)?;
    let frames = source.frames();
    let channels = source.channels;
    crate::dlog!(
        "入力WAV : {} フレーム / {} ch / {} Hz / {} ({} bit) / {} バイト",
        frames,
        source.channels,
        source.rate,
        if source.format == 1 {
            "PCM"
        } else {
            "IEEE Float"
        },
        source.bits,
        source.data.len()
    );
    crate::dlog!("再生時間: {:.3} 秒", frames as f64 / source.rate as f64);

    let staged = job.session.join("input.wav");
    let padded = frames
        .div_ceil(HOP)
        .checked_mul(HOP)
        .and_then(|v| v.checked_add(TAIL_PAD))
        .ok_or_else(|| Error::new("入力が長すぎます。"))?;
    crate::dlog!(
        "パディング: {} -> {} フレーム（ホップ {} に切り上げ + 末尾 {}）",
        frames,
        padded,
        HOP,
        TAIL_PAD
    );
    source.write(&staged, padded, true)?;
    crate::dlog!("中間ファイル: {}", staged.display());

    let out_dir = job.session.join("filtered");
    let mut command = Command::new(job.engine);
    command
        .current_dir(job.session)
        .arg("-m")
        .arg(job.model)
        .arg("-D")
        .arg("-a")
        .arg(job.attenuation.to_string());
    if job.post_filter {
        command.arg("--pf");
    }
    if job.verbose {
        command.arg("-v");
    }
    command.arg("-o").arg(&out_dir).arg(&staged);

    if crate::debug::enabled() {
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        crate::dlog!("実行     : {} {}", job.engine.display(), args.join(" "));
        crate::dlog!("作業ディレクトリ: {}", job.session.display());
    }

    let started = std::time::Instant::now();
    let output = command.output().context(format!(
        "エンジンを実行できません: {}",
        job.engine.display()
    ))?;
    crate::dlog!(
        "エンジン終了: {} / {:.3} 秒",
        output.status,
        started.elapsed().as_secs_f64()
    );

    let log = job.session.join("engine.log");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    fs::write(&log, text.as_bytes())?;
    crate::debug::emit_block("エンジンの出力", &text);

    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "シグナルによる終了".to_string());
        return Err(Error::new(format!(
            "エンジン終了コード {}。詳細: {}",
            code,
            log.display()
        )));
    }

    let mut filtered = Wave::read(&out_dir.join("input.wav"))?;
    crate::dlog!(
        "エンジン出力: {} フレーム / {} ({} bit)",
        filtered.frames(),
        if filtered.format == 1 {
            "PCM"
        } else {
            "IEEE Float"
        },
        filtered.bits
    );
    filtered.convert_to_pcm16()?;
    let result = job.session.join("clean.wav");
    filtered.write(&result, frames, false)?;
    crate::dlog!("{} フレームへ切り詰め: {}", frames, result.display());
    Ok(Outcome {
        result,
        log,
        frames,
        channels,
    })
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
