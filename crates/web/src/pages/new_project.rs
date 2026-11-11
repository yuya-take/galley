//! プロジェクトを作る（`/new`。サイドバーの「＋」）。

use std::sync::Arc;

use galley_core::domain::project::CrestColor;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, request},
    runtime::{Event, procedure, signal},
    view::{View, view},
};

use super::project_form::{self, ProjectForm, crest_picker, form_error, name_field, slug_field};
use crate::WebApp;

#[page("/new")]
pub async fn new_project(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let initial_color = default_color(app.sidebar_projects().await?.len());
    let host = project_form::app_host(request::headers(cx));

    let name = signal(cx, String::new);
    let slug = signal(cx, String::new);
    let color = signal(cx, || initial_color.as_str().to_owned());
    let error = signal(cx, String::new);
    let is_saving = signal(cx, || false);

    Ok(view! {
        <div class="settings">
            <header class="page-header">
                <div class="page-title">
                    <h1>"プロジェクトを作る"</h1>
                    <p class="page-description">"資料をまとめる入れ物です。説明はあとからプロジェクト設定で書けます。"</p>
                </div>
            </header>
            <form
                class="panel settings-form"
                autocomplete="off"
                @submit=$(async |e: Event| {
                    e.prevent_default();
                    is_saving.set(true);
                    error.set("".to_owned());
                    let result = create_project(ProjectForm {
                        name: name.get(),
                        slug: slug.get(),
                        description: "".to_owned(),
                        color: color.get(),
                    }).await;
                    if result.is_ok() {
                        let url = result.unwrap();
                        raw!("location.assign(${url})", drop(url));
                    } else {
                        is_saving.set(false);
                        error.set(result.unwrap_err());
                    }
                })
            >
                name_field(name: &name, initial: "")
                slug_field(slug: &slug, initial: "", host: host.as_deref())
                crest_picker(color: &color, initial: initial_color)
                form_error(error: &error)
                <div class="form-actions">
                    <a class="button button--outline" href="/">"やめる"</a>
                    <button type="submit" class="button button--primary" :disabled=$(is_saving.get())>"プロジェクトを作る"</button>
                </div>
            </form>
        </div>
    })
}

/// プロジェクトを作る。成功したらそのプロジェクトの資料一覧の URL、失敗したら画面に出す文言を返す。
#[procedure]
pub async fn create_project(cx: &Cx, form: ProjectForm) -> Result<Result<String, String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(create(app, form).await)
}

async fn create(app: &WebApp, form: ProjectForm) -> Result<String, String> {
    app.create_project
        .execute(&form.into())
        .await
        .map(|project| format!("/{}", project.slug))
        .map_err(project_form::error_message)
}

/// 紋章の色の初期値。作った順に6色を回し、並んだプロジェクトが同じ色になりにくくする。
fn default_color(project_count: usize) -> CrestColor {
    CrestColor::ALL[project_count % CrestColor::ALL.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::test_app;

    fn form(slug: &str) -> ProjectForm {
        ProjectForm {
            name: "営業企画".to_owned(),
            slug: slug.to_owned(),
            description: String::new(),
            color: "green".to_owned(),
        }
    }

    #[tokio::test]
    async fn create_returns_project_url() -> std::result::Result<(), String> {
        let app = test_app::new().await;
        assert_eq!(create(&app, form("sales")).await?, "/sales");
        let project = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(project.name.as_str(), "営業企画");
        assert_eq!(project.color, CrestColor::Green);
        Ok(())
    }

    #[tokio::test]
    async fn create_explains_invalid_or_taken_url() -> std::result::Result<(), String> {
        let app = test_app::new().await;
        create(&app, form("sales")).await?;
        assert_eq!(
            create(&app, form("sales")).await,
            Err(
                "URL「sales」はほかのプロジェクト（アーカイブしたものを含む）が使っています"
                    .to_owned()
            )
        );
        assert_eq!(
            create(&app, form("new")).await,
            Err("「new」は Galley の画面で使うため、URL にできません".to_owned())
        );
        assert_eq!(
            create(&app, form("営業")).await,
            Err("URL に使えるのは英小文字・数字・ハイフン（-）だけです".to_owned())
        );
        Ok(())
    }

    #[test]
    fn default_color_cycles_through_all_colors() {
        assert_eq!(default_color(0), CrestColor::Red);
        assert_eq!(default_color(5), CrestColor::Silver);
        assert_eq!(default_color(6), CrestColor::Red);
    }
}
