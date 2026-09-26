//! Galley のドメイン処理。
//!
//! アップロード検査、リビジョン作成、ストレージを扱う。画面（`galley-web`）と
//! MCP（`galley-server`）の両方から呼ばれるため、Topcoat には依存しない。
