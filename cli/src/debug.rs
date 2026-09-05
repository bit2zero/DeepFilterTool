//! `--debug` 指定時の詳細ログ。
//!
//! 出力先は標準エラーなので、標準出力をパイプでつないでいても混ざらない。
//! 各行に処理開始からの経過時間が付くため、どこで時間がかかったか分かる。

use std::fmt::Arguments;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static START: OnceLock<Instant> = OnceLock::new();

/// 環境変数 DEEPFILTER_DEBUG が空でない値なら、指定がなくても有効にする。
pub fn enabled_by_environment() -> bool {
    std::env::var_os("DEEPFILTER_DEBUG")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

pub fn enable() {
    START.get_or_init(Instant::now);
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 有効・無効を戻すための入口。検査で一時的に切り替えたあと元に戻すのに使う。
#[cfg(test)]
pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
}

pub fn emit(args: Arguments<'_>) {
    let elapsed = START.get_or_init(Instant::now).elapsed();
    let mut err = std::io::stderr().lock();
    // ログ出力の失敗で処理そのものを止めない。
    let _ = writeln!(err, "[debug {:7.3}s] {}", elapsed.as_secs_f64(), args);
}

/// 複数行の出力を、行ごとに接頭辞を付けて見せる。
pub fn emit_block(label: &str, body: &str) {
    if !enabled() {
        return;
    }
    let trimmed = body.trim_end();
    if trimmed.is_empty() {
        emit(format_args!("{}: (出力なし)", label));
        return;
    }
    emit(format_args!("{}:", label));
    let mut err = std::io::stderr().lock();
    for line in trimmed.lines() {
        let _ = writeln!(err, "[debug        ] | {}", line);
    }
}

#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        if $crate::debug::enabled() {
            $crate::debug::emit(format_args!($($arg)*));
        }
    };
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
