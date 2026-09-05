//! error.rs の単体テスト。

use super::*;

#[test]
fn new_accepts_both_str_and_string() {
    assert_eq!(Error::new("文字列リテラル").0, "文字列リテラル");
    assert_eq!(Error::new(String::from("String")).0, "String");
}

#[test]
fn display_shows_the_message_verbatim() {
    let error = Error::new("ノイズ除去に失敗しました。");
    assert_eq!(error.to_string(), "ノイズ除去に失敗しました。");
    assert_eq!(format!("{}", error), "ノイズ除去に失敗しました。");
}

#[test]
fn debug_is_available_for_test_failures() {
    assert!(format!("{:?}", Error::new("中身")).contains("中身"));
}

#[test]
fn implements_the_standard_error_trait() {
    fn as_std(e: Error) -> Box<dyn std::error::Error> {
        Box::new(e)
    }
    assert_eq!(as_std(Error::new("標準エラー")).to_string(), "標準エラー");
}

#[test]
fn converts_from_io_errors() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "見つかりません");
    let error: Error = io.into();
    assert!(error.0.contains("見つかりません"));
}

#[test]
fn question_mark_converts_io_errors_automatically() {
    fn read_missing() -> Result<String> {
        Ok(std::fs::read_to_string("/この/パスは/存在しない")?)
    }
    assert!(read_missing().is_err());
}

#[test]
fn context_prefixes_the_failure_with_what_was_attempted() {
    let failed: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "拒否",
    ));
    let error = failed.context("エンジンを実行できません").unwrap_err();
    assert!(
        error.0.starts_with("エンジンを実行できません: "),
        "{}",
        error.0
    );
    assert!(error.0.ends_with("拒否"), "元の理由を残す: {}", error.0);
}

#[test]
fn context_passes_success_through_untouched() {
    let ok: std::result::Result<u32, std::io::Error> = Ok(42);
    assert_eq!(ok.context("何か").unwrap(), 42);
}

#[test]
fn context_accepts_owned_strings() {
    let failed: std::result::Result<(), Error> = Err(Error::new("元の理由"));
    let error = failed
        .context(format!("{} を処理中", "ファイル"))
        .unwrap_err();
    assert_eq!(error.0, "ファイル を処理中: 元の理由");
}
