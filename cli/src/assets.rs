//! 公式配布物の固定版一覧。runtime/manifest.json はこの表から生成する
//! （`deepfilter-tool manifest` が同じ内容を出力する）。

pub const RELEASE: &str = "v0.5.6";
pub const REPOSITORY: &str = "https://github.com/Rikorose/DeepFilterNet";

pub struct Asset {
    /// runtime/ 配下の保存名。
    pub file: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// OS・CPU によらず必要なファイル。
pub const SHARED: &[Asset] = &[
    Asset {
        file: "DeepFilterNet3_onnx.tar.gz",
        url: "https://raw.githubusercontent.com/Rikorose/DeepFilterNet/v0.5.6/models/DeepFilterNet3_onnx.tar.gz",
        bytes: 7_983_136,
        sha256: "c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616",
    },
    Asset {
        file: "LICENSE-MIT.txt",
        url: "https://raw.githubusercontent.com/Rikorose/DeepFilterNet/v0.5.6/LICENSE-MIT",
        bytes: 1_083,
        sha256: "24e6bb09c928af8d8e56268082f87413247ce36b39dd5d33add2f9893968065e",
    },
];

/// プラットフォームごとの公式エンジン。保存名は Windows のみ .exe。
pub const ENGINES: &[(&str, Asset)] = &[
    (
        "windows-x86_64",
        Asset {
            file: "deep-filter.exe",
            url: "https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-x86_64-pc-windows-msvc.exe",
            bytes: 26_912_256,
            sha256: "75e11fa16445f560cb6b021521ddb89e89270d13b83089705d98776f58fd7915",
        },
    ),
    (
        "linux-x86_64",
        Asset {
            file: "deep-filter",
            url: "https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-x86_64-unknown-linux-musl",
            bytes: 36_417_296,
            sha256: "70775e251eee44c0f2451a1e833326cf8bcbbe304d3e7cd12851e6fce72ef7da",
        },
    ),
    (
        "linux-aarch64",
        Asset {
            file: "deep-filter",
            url: "https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-aarch64-unknown-linux-gnu",
            bytes: 39_238_496,
            sha256: "14e02a1c0028f3ca0bdf83b62b3336e56ba0556894ef295a95e8573f06557166",
        },
    ),
    (
        "macos-x86_64",
        Asset {
            file: "deep-filter",
            url: "https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-x86_64-apple-darwin",
            bytes: 29_933_512,
            sha256: "d3be84003acb7c23e738ad7f70a158ec779a8d233a82e7fa3e717d112eb5b50f",
        },
    ),
    (
        "macos-aarch64",
        Asset {
            file: "deep-filter",
            url: "https://github.com/Rikorose/DeepFilterNet/releases/download/v0.5.6/deep-filter-0.5.6-aarch64-apple-darwin",
            bytes: 27_877_081,
            sha256: "4601e7f4e4c03e59a4c5b5000216ef3add3e808799cfccd95e14e83ea4611081",
        },
    ),
];

pub fn engine_for(platform: &str) -> Option<&'static Asset> {
    ENGINES
        .iter()
        .find(|(key, _)| *key == platform)
        .map(|(_, asset)| asset)
}

pub fn platforms() -> Vec<&'static str> {
    ENGINES.iter().map(|(key, _)| *key).collect()
}

/// runtime/manifest.json と同一の内容を組み立てる。
pub fn manifest_json() -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"release\": \"{}\",\n", RELEASE));
    out.push_str(&format!("  \"repository\": \"{}\",\n", REPOSITORY));
    out.push_str("  \"shared\": [\n");
    for (i, asset) in SHARED.iter().enumerate() {
        out.push_str(&entry(asset, 4));
        out.push_str(if i + 1 == SHARED.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ],\n");
    out.push_str("  \"engines\": {\n");
    for (i, (key, asset)) in ENGINES.iter().enumerate() {
        out.push_str(&format!("    \"{}\": ", key));
        out.push_str(entry(asset, 4).trim_start());
        out.push_str(if i + 1 == ENGINES.len() { "\n" } else { ",\n" });
    }
    out.push_str("  }\n}\n");
    out
}

fn entry(asset: &Asset, indent: usize) -> String {
    let pad = " ".repeat(indent);
    format!(
        "{pad}{{\n{pad}  \"file\": \"{}\",\n{pad}  \"url\": \"{}\",\n{pad}  \"bytes\": {},\n{pad}  \"sha256\": \"{}\"\n{pad}}}",
        asset.file, asset.url, asset.bytes, asset.sha256,
        pad = pad
    )
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
