---
name: test-runner
description: テスト実行と結果分析。変更したクレートのテストを実行し、失敗の原因分析と修正提案を行う。実装完了後に使用。
tools: Read, Grep, Glob, Bash
model: haiku
maxTurns: 10
---

# Test Runner

変更されたクレートのテストを実行し、結果を分析する。

## 実行手順

### 1. 変更ファイルの特定

`git diff --name-only develop...HEAD` と `git diff --name-only` で変更されたファイルから、対象のクレートを特定する。

| パス | パッケージ |
|------|-----------|
| `crates/core/` | `galley-core` |
| `crates/web/` | `galley-web` |
| `crates/server/` | `galley-server` |

`galley-core` の変更は `galley-web` と `galley-server` にも影響するので、ワークスペース全体を実行する。

### 2. テスト実行

```bash
# ワークスペース全体
cargo test --workspace

# 特定のクレートだけ
cargo test -p galley-core

# 特定のテストだけ
cargo test -p galley-core test_name
```

CI と同じ検査も実行する:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

### 3. 失敗分析と修正提案

- 失敗テストのエラーメッセージを分析
- 変更されたコードとの関連を特定
- 修正が必要な箇所を具体的に指摘

## 出力フォーマット

```
## テスト結果

- 実行コマンド: `cargo test --workspace`
- 結果: ✅ 42 passed, ❌ 2 failed, ⏭ 1 ignored
- fmt: ✅ / clippy: ❌ 3 warnings

### 失敗テスト

1. `upload::tests::rejects_external_script` (crates/core/src/upload.rs:120)
   - 原因: `srcset` の検出を追加した際に戻り値の型を変えた
   - 修正: テストの期待値を新しい `Violation` 型に合わせる
```
