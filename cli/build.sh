#!/bin/sh
# Linux / macOS 向けビルド。外部クレートを使わないため、cargo だけで完結します。
#
# Linux で musl ターゲットが入っている場合は、システムの C コンパイラを使わずに
# 完全静的なバイナリを作ります（rustup 同梱の rust-lld と CRT を使用）。
#   rustup target add x86_64-unknown-linux-musl
set -eu

cd "$(dirname "$0")"

TARGET="${1:-}"
if [ -z "$TARGET" ] && [ "$(uname -s)" = "Linux" ]; then
    case "$(uname -m)" in
        x86_64) CANDIDATE=x86_64-unknown-linux-musl ;;
        aarch64|arm64) CANDIDATE=aarch64-unknown-linux-musl ;;
        *) CANDIDATE= ;;
    esac
    if [ -n "$CANDIDATE" ] && rustc --print target-libdir --target "$CANDIDATE" >/dev/null 2>&1; then
        TARGET="$CANDIDATE"
    fi
fi

if [ -n "$TARGET" ]; then
    echo "ターゲット: $TARGET"
    cargo build --release --target "$TARGET"
    OUT="target/$TARGET/release/deepfilter-tool"
else
    echo "ターゲット: ホスト既定"
    cargo build --release
    OUT="target/release/deepfilter-tool"
fi

echo "完成: $(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")"
echo "次に実行: $OUT setup"
