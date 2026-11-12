//! AI から登録する（`/connect`。サイドバーの「AIから登録する」）。MCP の接続の手引き。
//!
//! 空のプロジェクトとプロジェクト設定の「接続の手引き」もここを指す。登録コマンドの組み立ては
//! [`mcp_command`]、コピーと名前の変更は `assets/app.js`（`data-copy-text`、`data-action=change-author`）。

use std::sync::Arc;

use galley_core::domain::{project::ProjectSummary, revision::AuthorName};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, request},
    view::{View, component, view},
};

use super::documents;
use crate::{WebApp, author, mcp_command};

/// 頼み方の例に入れるプロジェクト名（プロジェクトがまだ無いとき）。
const EXAMPLE_PROJECT: &str = "営業企画";

#[page("/connect")]
pub async fn connect(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let author = author::from_headers(request::headers(cx));
    let mcp_url = mcp_command::url(&app.app_url, documents::host(cx));
    let project = example_project(&app.sidebar_projects().await?);

    Ok(view! {
        <div class="settings connect">
            <header class="page-header">
                <div class="page-title">
                    <h1>"AI から登録する"</h1>
                    <p class="page-description">"Claude Code などの AI エージェントに Galley をつなぐと、作った資料を AI がそのまま登録・更新できます。"</p>
                </div>
            </header>
            claude_code_section(mcp_url: &mcp_url, author: author.as_ref().map(AuthorName::as_str))
            token_section(mcp_url: &mcp_url, author: author.as_ref().map(AuthorName::as_str))
            other_clients_section(mcp_url: &mcp_url, author: author.as_ref().map(AuthorName::as_str))
            requests_section(project: &project)
        </div>
    })
}

/// 頼み方の例に入れるプロジェクト名。サイドバーの最初のプロジェクト、無ければ [`EXAMPLE_PROJECT`]。
fn example_project(projects: &[ProjectSummary]) -> String {
    projects.first().map_or_else(
        || EXAMPLE_PROJECT.to_owned(),
        |summary| summary.project.name.to_string(),
    )
}

/// 「Claude Code に登録する」。コマンドと、最後の名前が更新者として残ることの説明。
#[component]
async fn claude_code_section<'a>(mcp_url: &'a str, author: Option<&'a str>) -> Result<impl View> {
    let command = mcp_command::claude_code(mcp_url, author);
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="connect-claude-code">
            <h2 id="connect-claude-code">"Claude Code に登録する"</h2>
            <p>"ターミナルで次のコマンドを実行します。URL はいま開いている Galley のものです。"</p>
            command_box(text: &command, label: "Claude Code に登録するコマンド")
            <div class="connect-author">
                match author {
                    Some(name) => {
                        <p>
                            "最後の名前（" <strong>(name)</strong> "）が、AI が登録した資料の更新者として残ります。"
                            "このブラウザに保存した名前を入れています。"
                        </p>
                        <button type="button" class="button button--outline button--small" data-action="change-author">"名前を変える"</button>
                    },
                    None => {
                        <p>
                            "最後の名前が、AI が登録した資料の更新者として残ります。"
                            "名前が未設定なので「" (mcp_command::AUTHOR_PLACEHOLDER) "」を入れています。設定すると、コマンドに入ります。"
                        </p>
                        <button type="button" class="button button--primary button--small" data-action="change-author">"名前を設定する"</button>
                    },
                }
            </div>
            <p class="field-hint">"名前を変えたときは、" <code>"claude mcp remove galley"</code> " で消してから登録し直します。"</p>
        </section>
    })
}

/// 「トークンを求めるサーバーにつなぐとき」（`MCP_TOKEN`）。トークンの値は画面に出さない。
#[component]
async fn token_section<'a>(mcp_url: &'a str, author: Option<&'a str>) -> Result<impl View> {
    let command = mcp_command::claude_code_with_token(mcp_url, author);
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="connect-token">
            <h2 id="connect-token">"トークンを求めるサーバーにつなぐとき"</h2>
            <p>
                "サーバーの環境変数 " <code>"MCP_TOKEN"</code> " を設定している場合は、"
                <code>(mcp_command::TOKEN_HEADER)</code> " ヘッダーでトークンを渡します。"
                "コマンドの " <code>"<トークン>"</code> " を、管理者から聞いた値に置き換えてください。"
            </p>
            command_box(text: &command, label: "トークン付きで登録するコマンド")
            <p class="field-hint">"トークンが無い・違うときは、AI の接続が「401 Unauthorized」で断られます。"</p>
        </section>
    })
}

/// 「ほかの MCP クライアントで使う」。URL とヘッダーの一覧と、設定ファイルの例。
#[component]
async fn other_clients_section<'a>(mcp_url: &'a str, author: Option<&'a str>) -> Result<impl View> {
    let json = mcp_command::json_config(mcp_url, author);
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="connect-others">
            <h2 id="connect-others">"ほかの MCP クライアントで使う"</h2>
            <p>"Streamable HTTP の MCP サーバーとして、次の URL とヘッダーを設定します。"</p>
            <table class="connect-table">
                <thead>
                    <tr>
                        <th scope="col">"項目"</th>
                        <th scope="col">"値"</th>
                        <th scope="col">"説明"</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <th scope="row">"種類"</th>
                        <td><code>"Streamable HTTP"</code></td>
                        <td>"設定ファイルでは " <code>"\"type\": \"http\""</code> " と書くものが多いです"</td>
                    </tr>
                    <tr>
                        <th scope="row">"URL"</th>
                        <td><code>(mcp_url)</code></td>
                        <td>"いま開いている Galley の " <code>"/mcp"</code></td>
                    </tr>
                    <tr>
                        <th scope="row">"ヘッダー"</th>
                        <td><code>(format!("{}: {}", mcp_command::AUTHOR_HEADER, mcp_command::author_value(author)))</code></td>
                        <td>"必須。AI が登録した資料の更新者として残ります"</td>
                    </tr>
                    <tr>
                        <th scope="row">"ヘッダー"</th>
                        <td><code>(format!("{}: {}", mcp_command::TOKEN_HEADER, mcp_command::TOKEN_VALUE))</code></td>
                        <td>"サーバーに " <code>"MCP_TOKEN"</code> " を設定しているときだけ"</td>
                    </tr>
                </tbody>
            </table>
            <p>"設定ファイル（JSON）に書くクライアントの例です。キーの名前はクライアントによって少し違うので、それぞれの説明に合わせてください。"</p>
            command_box(text: &json, label: "設定ファイルの例", is_block: true)
        </section>
    })
}

/// 「AI への頼み方」。登録できる HTML の条件も添える。
#[component]
async fn requests_section(project: &str) -> Result<impl View> {
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="connect-requests">
            <h2 id="connect-requests">"AI への頼み方"</h2>
            <p>"つないだあとは、普段の言葉で頼めます。プロジェクトは名前で伝えれば、AI が一覧から探します。"</p>
            <ul class="connect-examples">
                <li class="ai-example">(format!("「この資料を Galley の『{project}』に保存して」"))</li>
                <li class="ai-example">(format!("「Galley の『{project}』の提案書を、いまの内容で新しい版にして」"))</li>
                <li class="ai-example">"「Galley にある資料の一覧を見せて」"</li>
            </ul>
            <p>
                "登録できるのは、外部のファイル（CDN のスクリプト、Web フォント、外部の画像など）を読み込まない、1ファイルで完結した HTML だけです。"
                "読み込んでいるときは該当箇所が AI に伝わるので、直して登録し直してもらえます。"
            </p>
        </section>
    })
}

/// コマンドや設定の例と「コピー」。`label` はコピーボタンの読み上げに添える名前。
#[component]
async fn command_box<'a>(
    text: &'a str,
    label: &'a str,
    #[default] is_block: bool,
) -> Result<impl View> {
    Ok(view! {
        <div class=(if is_block { "command command--block" } else { "command" })>
            if is_block {
                <pre><code>(text)</code></pre>
            } else {
                <code>(text)</code>
            }
            <button type="button" class="button button--outline button--small" data-copy-text=(text)>
                <span data-copy-label="">"コピー"</span>
                <span class="visually-hidden">(label)</span>
            </button>
        </div>
    })
}

#[cfg(test)]
mod tests {
    use galley_core::app::project::ProjectInput;

    use super::*;
    use crate::pages::test_app;

    #[tokio::test]
    async fn example_uses_first_project_or_placeholder() -> std::result::Result<(), String> {
        let app = test_app::new().await;
        let projects = app.sidebar_projects().await.map_err(|e| e.to_string())?;
        assert_eq!(example_project(&projects), "営業企画");

        app.create_project
            .execute(&ProjectInput {
                slug: "design".to_owned(),
                name: "デザイン部".to_owned(),
                description: String::new(),
                color: "blue".to_owned(),
            })
            .await
            .map_err(|e| e.to_string())?;
        let projects = app.sidebar_projects().await.map_err(|e| e.to_string())?;
        assert_eq!(example_project(&projects), "デザイン部");
        Ok(())
    }
}
