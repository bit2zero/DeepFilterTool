//! assets.rs の単体テスト。固定版一覧そのものの整合性を検査する。

use super::*;

#[test]
fn engine_for_resolves_every_listed_platform() {
    for key in platforms() {
        let asset = engine_for(key).unwrap_or_else(|| panic!("{} が引けること", key));
        assert!(!asset.url.is_empty());
        assert!(asset.bytes > 1_000_000, "{} のサイズが小さすぎる", key);
        assert_eq!(asset.sha256.len(), 64, "{} の SHA-256 は 64 桁", key);
        assert!(
            asset.sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "{} の SHA-256 は 16 進数",
            key
        );
        let expected = if key.starts_with("windows") {
            "deep-filter.exe"
        } else {
            "deep-filter"
        };
        assert_eq!(asset.file, expected, "{} の保存名", key);
    }
}

#[test]
fn engine_for_returns_none_for_an_unknown_platform() {
    assert!(engine_for("solaris-sparc").is_none());
    assert!(engine_for("").is_none());
    assert!(engine_for("linux").is_none(), "アーキテクチャなしは不可");
}

#[test]
fn platforms_covers_the_three_operating_systems() {
    let keys = platforms();
    assert_eq!(keys.len(), ENGINES.len());
    for expected in [
        "windows-x86_64",
        "linux-x86_64",
        "linux-aarch64",
        "macos-x86_64",
        "macos-aarch64",
    ] {
        assert!(keys.contains(&expected), "{} が載っている", expected);
    }
}

#[test]
fn shared_assets_are_pinned_to_the_release() {
    assert_eq!(SHARED.len(), 2, "モデルとライセンス");
    for asset in SHARED {
        assert!(
            asset.url.contains(RELEASE),
            "{} の URL に版が固定されている",
            asset.file
        );
        assert_eq!(asset.sha256.len(), 64);
    }
}

#[test]
fn every_url_is_https_and_points_at_the_official_repository() {
    let owner = REPOSITORY.trim_start_matches("https://github.com/");
    for asset in SHARED.iter().chain(ENGINES.iter().map(|(_, a)| a)) {
        assert!(asset.url.starts_with("https://"), "{} は HTTPS", asset.file);
        assert!(
            asset.url.contains(owner),
            "{} は公式リポジトリ由来: {}",
            asset.file,
            asset.url
        );
        assert!(
            asset.url.contains(RELEASE),
            "{} は版が固定されている",
            asset.file
        );
    }
}

#[test]
fn manifest_json_lists_every_asset_exactly_once() {
    let json = manifest_json();
    assert!(
        json.starts_with("{\n") && json.ends_with("}\n"),
        "JSON 文書の形"
    );
    assert!(json.contains(&format!("\"release\": \"{}\"", RELEASE)));
    assert!(json.contains(&format!("\"repository\": \"{}\"", REPOSITORY)));
    for key in platforms() {
        assert_eq!(
            json.matches(&format!("\"{}\": {{", key)).count(),
            1,
            "{} がちょうど 1 回",
            key
        );
    }
    for asset in SHARED.iter().chain(ENGINES.iter().map(|(_, a)| a)) {
        assert!(
            json.contains(asset.sha256),
            "{} の SHA-256 が載る",
            asset.file
        );
        assert!(
            json.contains(&asset.bytes.to_string()),
            "{} のサイズが載る",
            asset.file
        );
        assert!(json.contains(asset.url), "{} の URL が載る", asset.file);
    }
    // 末尾要素にカンマが付かないこと。
    assert!(!json.contains(",\n  ]"), "shared の末尾にカンマがない");
    assert!(!json.contains(",\n  }"), "engines の末尾にカンマがない");
}

#[test]
fn manifest_json_is_stable_across_calls() {
    assert_eq!(manifest_json(), manifest_json());
}
