//! ツールの引数。

use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ProjectParams {
    /// プロジェクトの slug（list_projects で調べる）。
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DocumentParams {
    /// プロジェクトの slug。
    pub project: String,
    /// 資料の slug（list_documents で調べる）。
    pub document: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetDocumentParams {
    /// プロジェクトの slug。
    pub project: String,
    /// 資料の slug。
    pub document: String,
    /// 版の番号（1 から）。省略すると最新の版。
    pub revision: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreateDocumentParams {
    /// 登録先のプロジェクトの slug。
    pub project: String,
    /// 資料名。
    pub title: String,
    /// 資料の HTML 本文。外部リソースを読み込まない1ファイルの HTML。
    pub html: String,
    /// 資料の URL に使う slug（英小文字・数字・ハイフン）。省略すると自動で決める。
    pub slug: Option<String>,
    /// 変更メモ。省略可。
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateDocumentParams {
    #[serde(flatten)]
    pub target: DocumentParams,
    /// 新しい版の HTML 本文。外部リソースを読み込まない1ファイルの HTML。
    pub html: String,
    /// 何を変えたか（変更メモ）。
    pub message: Option<String>,
}
