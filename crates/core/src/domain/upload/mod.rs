//! アップロードされた HTML の検査。
//!
//! 資料は「外部リソースを一切読み込まない、1ファイルで完結した HTML」に統一する。
//! 画面と MCP の両方がこの検査を通す。検査は利便性のためで、最終的な防御は表示側の CSP が担う。

mod css;
mod resource;
mod scan;
mod script;
mod url;

pub use resource::{ExternalResource, ExternalResources, REJECTION_REASON, ResourceKind};

/// 1ファイルのサイズ上限（バイト）。値は #21 で見直す。
pub const MAX_HTML_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UploadError {
    #[error("ファイルが空です")]
    Empty,
    #[error("ファイルが大きすぎます（{size} バイト。上限は {max} バイト）")]
    TooLarge { size: usize, max: usize },
    #[error("UTF-8 の HTML ではありません")]
    NotUtf8,
    #[error("外部リソースを {} か所で読み込んでいます。{REJECTION_REASON}", .0.total())]
    ExternalResources(ExternalResources),
}

/// 検査を通った HTML。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlDocument {
    html: String,
    title: Option<String>,
}

impl HtmlDocument {
    /// サイズ・文字コード・外部リソースの読み込みを検査する。
    ///
    /// 上限いっぱいの 10MB で 0.1 秒ほど CPU を使うので、非同期の処理からはブロックする処理として呼ぶ。
    pub fn parse(bytes: Vec<u8>) -> Result<Self, UploadError> {
        if bytes.len() > MAX_HTML_BYTES {
            return Err(UploadError::TooLarge {
                size: bytes.len(),
                max: MAX_HTML_BYTES,
            });
        }
        let html = String::from_utf8(bytes).map_err(|_| UploadError::NotUtf8)?;
        if html.trim().is_empty() {
            return Err(UploadError::Empty);
        }

        let scan = scan::scan(&html);
        if !scan.findings.is_empty() {
            return Err(UploadError::ExternalResources(external_resources(
                &html,
                scan.findings,
            )));
        }
        Ok(Self {
            html,
            title: scan.title,
        })
    }

    /// `<title>` の中身（空白を詰めたもの）。新しい資料の資料名の初期値に使う。
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// アップロードされたままの中身。
    pub fn as_bytes(&self) -> &[u8] {
        self.html.as_bytes()
    }

    /// アップロードされたままの中身（ブロブとして保存する）。
    pub fn into_bytes(self) -> Vec<u8> {
        self.html.into_bytes()
    }
}

fn external_resources(html: &str, findings: Vec<scan::Finding>) -> ExternalResources {
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(html.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |offset: usize| line_starts.partition_point(|&start| start <= offset);

    let total = findings.len();
    let mut resources: Vec<ExternalResource> = findings
        .into_iter()
        .map(|finding| ExternalResource {
            line: line_of(finding.offset) + finding.extra_lines,
            kind: finding.kind,
            code: finding.code,
            url: finding.url,
        })
        .collect();
    resources.sort_by_key(|r| r.line);
    resources.truncate(ExternalResources::MAX_LISTED);
    ExternalResources::new(resources, total)
}
