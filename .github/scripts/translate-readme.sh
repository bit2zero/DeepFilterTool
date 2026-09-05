#!/usr/bin/env bash
#
# README.md（日本語・正本）から README.en.md を再生成する。
#
#   使い方: ANTHROPIC_API_KEY=... .github/scripts/translate-readme.sh
#
# 生成結果を README.en.md に書き出し、内容が変わったかどうかを終了コードで返す。
#   0 = 変更あり（書き出した）
#   9 = 変更なし（既存と同一）
#   1 = 失敗
#
# JSON の組み立てには jq を使う。README には引用符・バックティック・改行・
# 日本語が含まれるため、文字列連結で JSON を作ると必ず壊れる。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SOURCE="$ROOT/README.md"
TARGET="$ROOT/README.en.md"
MODEL="claude-opus-5"

if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "エラー: ANTHROPIC_API_KEY が設定されていません。" >&2
  exit 1
fi
for cmd in curl jq; do
  command -v "$cmd" >/dev/null || { echo "エラー: $cmd が必要です。" >&2; exit 1; }
done
[ -f "$SOURCE" ] || { echo "エラー: $SOURCE がありません。" >&2; exit 1; }

INSTRUCTIONS=$(cat <<'PROMPT'
あなたは技術文書の翻訳者です。日本語の README.md を英語に翻訳し、README.en.md
の中身だけを出力してください。

守ること:

1. 出力は Markdown 本文のみ。前置き・後書き・コードフェンスで囲むことはしない。
2. リンクとパスは一切変更しない。`docs/CLI.md` などの相対パスはそのまま。
3. バッジ（先頭の 2 行）はそのまま複製する。
4. バッジの直後に、次の 2 行をこの通りに置く。

   **English** | [日本語](README.md)

   > This is a translation of [README.md](README.md), which is the authoritative
   > version. If the two disagree, the Japanese version is correct.

   （引用行は 1 行にまとめてよい）
5. 日本語版にあった「このファイルが正本です」の注記は、英語版では 4 の注記に
   置き換わるので重複させない。
6. 数値・単位・コマンド・オプション名・ファイル名は変更しない。
7. 画面に表示される日本語の文言（ボタン名など）を訳すときは、原文を残したうえで
   英訳を添える。例: `「ノイズを除去」(Remove noise)`
8. ドキュメント一覧の節には、リンク先が日本語であることを 1 文で断る。
9. 技術文書として自然な英語にする。逐語訳にしない。日本語版の簡潔さと
   断定的な語調を保つ。
PROMPT
)

payload=$(jq -n \
  --arg model "$MODEL" \
  --arg system "$INSTRUCTIONS" \
  --rawfile readme "$SOURCE" \
  '{
     model: $model,
     max_tokens: 16000,
     system: $system,
     fallbacks: "default",
     messages: [
       { role: "user",
         content: ("次の README.md を英語に翻訳してください。\n\n---\n\n" + $readme) }
     ]
   }')

echo "翻訳中… ($MODEL, 入力 $(wc -c < "$SOURCE") バイト)" >&2

body=$(mktemp)
trap 'rm -f "$body"' EXIT
status=$(curl -sS -o "$body" -w '%{http_code}' \
  --proto '=https' --tlsv1.2 --max-time 300 \
  https://api.anthropic.com/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "anthropic-beta: server-side-fallback-2026-07-01" \
  -d "$payload")

if [ "$status" != "200" ]; then
  echo "エラー: API が HTTP $status を返しました。" >&2
  jq -r '.error.message // .' < "$body" >&2 || cat "$body" >&2
  exit 1
fi

stop_reason=$(jq -r '.stop_reason // "unknown"' < "$body")
case "$stop_reason" in
  end_turn) ;;
  max_tokens)
    echo "エラー: 出力が max_tokens で打ち切られました。翻訳が不完全です。" >&2
    exit 1 ;;
  refusal)
    echo "エラー: リクエストが拒否されました（stop_reason=refusal）。" >&2
    jq -r '.stop_details.explanation // "理由の記載なし"' < "$body" >&2
    exit 1 ;;
  *)
    echo "エラー: 想定外の stop_reason=$stop_reason" >&2
    exit 1 ;;
esac

translated=$(jq -r '[.content[] | select(.type == "text") | .text] | join("")' < "$body")
if [ -z "$translated" ]; then
  echo "エラー: 応答に本文が含まれていません。" >&2
  exit 1
fi

# 明らかにおかしい出力を配置しない
case "$translated" in
  '# '*) ;;
  *) echo "エラー: 出力が Markdown 見出しで始まっていません。" >&2
     printf '%s\n' "$translated" | head -3 >&2
     exit 1 ;;
esac

printf '%s\n' "$translated" > "$TARGET.new"
tokens=$(jq -r '"入力 \(.usage.input_tokens) / 出力 \(.usage.output_tokens) トークン"' < "$body")
echo "完了: $tokens" >&2

if [ -f "$TARGET" ] && cmp -s "$TARGET" "$TARGET.new"; then
  rm -f "$TARGET.new"
  echo "変更なし: $TARGET は最新です。" >&2
  exit 9
fi

mv "$TARGET.new" "$TARGET"
echo "更新しました: $TARGET" >&2
exit 0
