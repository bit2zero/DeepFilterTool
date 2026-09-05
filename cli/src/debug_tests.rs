//! debug.rs の単体テスト。
//!
//! 有効・無効はプロセス全体で共有する状態なので、実際の出力内容は
//! 別プロセスを起動する統合テスト（tests/debug_log.rs）で確認する。

use super::*;

#[test]
fn starts_disabled_and_turns_on_once_enabled() {
    // 他のテストが先に有効化している可能性があるため、状態を保存して戻す。
    let before = enabled();
    ENABLED.store(false, Ordering::Relaxed);
    assert!(!enabled(), "既定では無効");
    enable();
    assert!(enabled(), "enable() で有効になる");
    enable();
    assert!(enabled(), "重ねて呼んでも有効のまま");
    ENABLED.store(before, Ordering::Relaxed);
}

#[test]
fn emitting_never_panics_whether_enabled_or_not() {
    // 出力先が閉じられていても処理を止めないこと。
    emit(format_args!("検査用の行 {}", 1));
    emit_block("空の出力", "");
    emit_block("複数行", "1 行目\n2 行目\n");
}

#[test]
fn emit_block_handles_empty_and_multiline_bodies_when_enabled() {
    // emit_block は無効時に何もせず戻るため、有効にした状態でも通しておく。
    let before = enabled();
    enable();
    emit_block("空の出力", "");
    emit_block("空白だけ", "   \n\n  ");
    emit_block("複数行", "1 行目\n2 行目\n3 行目\n");
    emit_block("末尾の改行なし", "最後の行");
    ENABLED.store(before, Ordering::Relaxed);
}

#[test]
fn environment_switch_reads_the_variable() {
    // 値を書き換えずに、判定関数が例外なく動くことだけを確かめる。
    // 実際の DEEPFILTER_DEBUG=1 での挙動は統合テストで確認する。
    let _ = enabled_by_environment();
}
