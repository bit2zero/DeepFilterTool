//! 公式エンジンとモデルの導入。SHA-256 とサイズを照合してから runtime/ に配置する。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::assets::{self, Asset};
use crate::error::{Context, Error, Result};
use crate::sha256;

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE".into())
            .split(';')
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{}{}", program, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// PowerShell の単引用符文字列に安全に埋め込む。単引用符は 2 個重ねると literal になる。
///
/// これをしないと、導入先のパスに `'` が含まれるだけで（Windows のフォルダー名には
/// 使える）文字列から抜け出し、任意のコマンドが実行されてしまう。
fn powershell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// HTTPS 取得。TLS 検証は各ツールの既定（有効）のまま使い、HTTPS 以外へ落ちないよう明示する。
///
/// 取得ツールは `which` が返した絶対パスで起動する。名前だけで起動すると、
/// 探索順序によっては意図しない場所の同名プログラムを実行しうる。
fn download(url: &str, dest: &Path) -> Result<()> {
    crate::dlog!("取得: {} -> {}", url, dest.display());
    if !url.starts_with("https://") {
        return Err(Error::new(format!("HTTPS 以外は取得しません: {}", url)));
    }
    let status = if let Some(curl) = which("curl") {
        crate::dlog!("取得ツール: {}", curl.display());
        Command::new(curl)
            .args(["--proto", "=https", "--tlsv1.2", "-sSfL", "-o"])
            .arg(dest)
            .arg(url)
            .status()
            .context("curl を実行できません")?
    } else if let Some(wget) = which("wget") {
        crate::dlog!("取得ツール: {}", wget.display());
        Command::new(wget)
            .args(["-q", "--https-only", "--secure-protocol=TLSv1_2", "-O"])
            .arg(dest)
            .arg(url)
            .status()
            .context("wget を実行できません")?
    } else if let Some(shell) = which("powershell") {
        crate::dlog!("取得ツール: {}", shell.display());
        Command::new(shell)
            .args(["-NoProfile", "-Command"])
            .arg(format!(
                "$ProgressPreference='SilentlyContinue'; \
                 [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
                 Invoke-WebRequest -Uri {} -OutFile {}",
                powershell_quote(url),
                powershell_quote(&dest.display().to_string())
            ))
            .status()
            .context("PowerShell を実行できません")?
    } else {
        return Err(Error::new(
            "curl か wget が必要です。どちらかを導入してから再実行してください。",
        ));
    };
    if !status.success() {
        let _ = fs::remove_file(dest);
        return Err(Error::new(format!("ダウンロードに失敗しました: {}", url)));
    }
    Ok(())
}

fn verify(path: &Path, asset: &Asset) -> Result<()> {
    let size = path.metadata()?.len();
    crate::dlog!(
        "照合: {} (期待 {} バイト / 実際 {} バイト)",
        asset.file,
        asset.bytes,
        size
    );
    if size != asset.bytes {
        return Err(Error::new(format!(
            "{} のサイズが一致しません（期待 {} / 実際 {}）。",
            asset.file, asset.bytes, size
        )));
    }
    let digest = sha256::file_hex(path)?;
    crate::dlog!("  sha256 期待 {}", asset.sha256);
    crate::dlog!("  sha256 実際 {}", digest);
    if digest != asset.sha256 {
        return Err(Error::new(format!(
            "{} の SHA-256 が一致しません。\n  期待: {}\n  実際: {}",
            asset.file, asset.sha256, digest
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = path.metadata()?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// macOS は Gatekeeper の検疫属性が付くと起動できないため取り除く。
fn clear_quarantine(path: &Path) {
    if cfg!(target_os = "macos") {
        if let Some(xattr) = which("xattr") {
            let _ = Command::new(xattr)
                .arg("-d")
                .arg("com.apple.quarantine")
                .arg(path)
                .status();
        }
    }
}

fn install(runtime: &Path, asset: &Asset, executable: bool, force: bool) -> Result<bool> {
    let target = runtime.join(asset.file);
    crate::dlog!("導入対象: {} (force={})", target.display(), force);
    if target.is_file() && !force {
        match verify(&target, asset) {
            Ok(()) => return Ok(false),
            Err(e) => {
                return Err(Error::new(format!(
                    "{}\n既存ファイル {} が固定版と異なります。入れ替える場合は --force を付けてください。",
                    e,
                    target.display()
                )))
            }
        }
    }

    let staging = runtime.join(format!("{}.download", asset.file));
    let _ = fs::remove_file(&staging);
    println!("取得中: {} ({} バイト)", asset.file, asset.bytes);
    download(asset.url, &staging)?;
    if let Err(e) = verify(&staging, asset) {
        let _ = fs::remove_file(&staging);
        return Err(e);
    }
    if executable {
        make_executable(&staging)?;
    }
    // まず rename を試す。Unix では既存ファイルを不可分に置き換えるので、
    // 途中でエンジンが存在しない瞬間を作らない。Windows は既存があると失敗するため、
    // そのときだけ消してから置き直す。
    if fs::rename(&staging, &target).is_err() {
        let _ = fs::remove_file(&target);
        fs::rename(&staging, &target).context(format!("配置できません: {}", target.display()))?;
    }
    if executable {
        clear_quarantine(&target);
    }
    println!("  検証OK: {}", asset.sha256);
    Ok(true)
}

pub fn run(root: &Path, platform: &str, force: bool) -> Result<()> {
    let engine = assets::engine_for(platform).ok_or_else(|| {
        Error::new(format!(
            "未知のプラットフォーム {} です。指定できるのは: {}",
            platform,
            assets::platforms().join(", ")
        ))
    })?;
    crate::dlog!(
        "プラットフォーム: {} / 固定版 {}",
        platform,
        assets::RELEASE
    );
    let runtime = root.join("runtime");
    fs::create_dir_all(&runtime).context(format!(
        "runtime フォルダーを作れません: {}",
        runtime.display()
    ))?;

    println!("導入先: {}", runtime.display());
    println!("対象   : {} / DeepFilterNet {}", platform, assets::RELEASE);

    let mut installed = 0;
    for asset in assets::SHARED {
        if install(&runtime, asset, false, force)? {
            installed += 1;
        } else {
            println!("導入済み: {}", asset.file);
        }
    }
    if install(&runtime, engine, true, force)? {
        installed += 1;
    } else {
        println!("導入済み: {}", engine.file);
    }

    if installed == 0 {
        println!("すべて導入済みで、SHA-256 も一致しました。");
    } else {
        println!("完了しました。{} 件を導入しました。", installed);
    }
    Ok(())
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
