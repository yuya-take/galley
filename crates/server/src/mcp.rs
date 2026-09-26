//! MCP サーバー。

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    service::RequestContext,
    tool, tool_handler, tool_router,
};

#[derive(Debug, Clone)]
pub struct GalleyMcp {
    tool_router: ToolRouter<Self>,
}

impl GalleyMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl GalleyMcp {
    // 試作（#5）: ツールから HTTP ヘッダー（X-Author-Name）を読めるか確認する。
    #[tool(description = "接続設定の X-Author-Name ヘッダーに入れた更新者名を返す")]
    async fn whoami(&self, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, ErrorData> {
        let author = ctx
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.headers.get("x-author-name"))
            // to_str() は ASCII 以外を受け付けないため、日本語の名前は UTF-8 として読む
            .and_then(|value| std::str::from_utf8(value.as_bytes()).ok());
        let text = author.unwrap_or("（X-Author-Name が未設定）");
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GalleyMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}
