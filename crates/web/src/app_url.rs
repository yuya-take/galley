//! アプリの画面の URL（MCP が AI に返す資料ビューアのリンクなど）。
//!
//! 既定はリクエストの Host ヘッダーに `http://` を付ける。リバースプロキシで HTTPS にしたり
//! 別のホスト名にしたりするときは `GALLEY_APP_URL` で指定する。

use std::fmt;

use crate::host::{base_url, hostname};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppUrl {
    /// 指定した URL（`https://galley.example.com` など。末尾の `/` は除く）。
    Fixed(String),
    /// リクエストの Host ヘッダーから作る。
    FromRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "アプリの URL「{0}」は http(s)://<ホスト名>[:ポート] の形で指定してください（パスは付けない）"
)]
pub struct AppUrlError(String);

impl AppUrl {
    pub fn parse(value: &str) -> Result<Self, AppUrlError> {
        base_url(value)
            .map(Self::Fixed)
            .ok_or_else(|| AppUrlError(value.to_owned()))
    }

    /// 資料ビューアの URL（`/<プロジェクト>/<資料>`）。slug は英小文字・数字・ハイフンだけなので、
    /// そのまま URL に入れられる。`host` はリクエストの Host ヘッダーで、正しくなければ `None`。
    pub fn document_url(
        &self,
        host: Option<&str>,
        project_slug: &str,
        document_slug: &str,
    ) -> Option<String> {
        Some(format!(
            "{}/{project_slug}/{document_slug}",
            self.base(host)?
        ))
    }

    /// 資料の特定の版の URL（`/<プロジェクト>/<資料>/v/<番号>`）。
    pub fn revision_url(
        &self,
        host: Option<&str>,
        project_slug: &str,
        document_slug: &str,
        number: u32,
    ) -> Option<String> {
        Some(format!(
            "{}/v/{number}",
            self.document_url(host, project_slug, document_slug)?
        ))
    }

    fn base(&self, host: Option<&str>) -> Option<String> {
        match self {
            Self::Fixed(base) => Some(base.clone()),
            Self::FromRequest => {
                let host = host?;
                // ポートを残したいので、検査だけ hostname で行う
                hostname(host)?;
                Some(format!("http://{}", host.trim().to_ascii_lowercase()))
            }
        }
    }
}

impl fmt::Display for AppUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(base) => f.write_str(base),
            Self::FromRequest => f.write_str("http://<リクエストのホスト名>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_url_uses_request_host_by_default() {
        let url = AppUrl::FromRequest;
        assert_eq!(
            url.document_url(Some("Galley.Internal:8080"), "sales", "plan")
                .as_deref(),
            Some("http://galley.internal:8080/sales/plan")
        );
        assert_eq!(url.document_url(None, "sales", "plan"), None);
        assert_eq!(
            url.revision_url(Some("localhost:8080"), "sales", "plan", 3)
                .as_deref(),
            Some("http://localhost:8080/sales/plan/v/3")
        );
        assert_eq!(url.document_url(Some("evil/x"), "sales", "plan"), None);
    }

    #[test]
    fn fixed_url_ignores_request_host() {
        let url = AppUrl::parse("https://galley.example.com/");
        assert_eq!(
            url.ok()
                .and_then(|u| u.document_url(Some("localhost:8080"), "sales", "plan"))
                .as_deref(),
            Some("https://galley.example.com/sales/plan")
        );
        assert!(AppUrl::parse("https://galley.example.com/app").is_err());
    }
}
