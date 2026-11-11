//! プロジェクト設定（`/{project}/settings`）。名前・URL・説明・紋章の色、AI への頼み方、アーカイブ。

use std::sync::Arc;

use galley_core::{app::error::AppError, domain::project::Project};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, page, path_param, request},
    runtime::{Event, procedure, signal},
    view::{Unescaped, View, component, view},
};

use super::{
    archive::restore_project,
    project_form::{
        self, ProjectForm, crest_picker, description_field, form_error, name_field, slug_field,
    },
};
use crate::{WebApp, icon};

mod param {
    topcoat::router::path_param!(pub project);
}

/// 保存したあと、設定の URL に付けるクエリ（「保存しました」を出す）。
const SAVED_QUERY: &str = "saved=1";

#[page("/{project}/settings")]
pub async fn project_settings(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let project = match app
        .find_project
        .execute(path_param::<param::Project>(cx))
        .await
    {
        Ok(project) => project,
        Err(AppError::ProjectNotFound) => return Err(not_found().into()),
        Err(error) => return Err(error.into()),
    };
    let host = project_form::app_host(cx, app);
    let is_saved = request::uri(cx).query() == Some(SAVED_QUERY);

    Ok(view! {
        <div class="settings">
            breadcrumb(project: &project)
            <header class="page-header">
                <div class="page-title">
                    <h1>"プロジェクト設定"</h1>
                </div>
            </header>
            if project.is_archived() {
                <p class="banner">"このプロジェクトはアーカイブしています。一覧とサイドバーには出ません。"</p>
            }
            if is_saved {
                <p class="banner" role="status">"保存しました"</p>
            }
            settings_form(project: &project, host: host.as_deref())
            ai_section(project: &project)
            archive_section(project: &project)
        </div>
    })
}

/// 名前・URL・説明・紋章の色と「保存」。
#[component]
async fn settings_form(cx: &Cx, project: &Project, host: Option<&str>) -> Result<impl View> {
    let id = project.id.to_string();
    let name = signal(cx, || project.name.to_string());
    let slug = signal(cx, || project.slug.to_string());
    let description = signal(cx, || project.description.to_string());
    let color = signal(cx, || project.color.as_str().to_owned());
    let error = signal(cx, String::new);
    let is_saving = signal(cx, || false);

    Ok(view! {
            <form
                class="panel settings-form"
                autocomplete="off"
                @submit=$(async |e: Event| {
                    e.prevent_default();
                    is_saving.set(true);
                    error.set("".to_owned());
                    let result = update_project(id, ProjectForm {
                        name: name.get(),
                        slug: slug.get(),
                        description: description.get(),
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
                name_field(name: &name, initial: project.name.as_str())
                slug_field(
                    slug: &slug,
                    initial: project.slug.as_str(),
                    host: host,
                    note: Some("変えると、前のURLで共有したリンクは開けなくなります。"),
                )
                description_field(description: &description, initial: project.description.as_str())
                crest_picker(color: &color, initial: project.color)
                form_error(error: &error)
                <div class="form-actions">
                    <button type="submit" class="button button--primary" :disabled=$(is_saving.get())>"保存"</button>
                </div>
            </form>
    })
}

/// 「AIからこのプロジェクトに保存する」。頼み方の例と、接続の手引き（`/connect`）へのリンク。
#[component]
async fn ai_section(project: &Project) -> Result<impl View> {
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="ai-heading">
            <h2 id="ai-heading">"AIからこのプロジェクトに保存する"</h2>
            <p>"Galley とつないだ AI に、たとえば次のように頼むと、このプロジェクトに資料を保存します。"</p>
            <p class="ai-example">(format!("「この資料を Galley の『{}』に保存して」", project.name))</p>
            <p><a href="/connect">"AI とつなぐ手順を見る"</a></p>
        </section>
    })
}

/// パンくず（紋章とプロジェクト名 / 設定）。アーカイブしたプロジェクトの資料一覧は開けないので、
/// そのときは名前にリンクを付けない。
#[component]
async fn breadcrumb(project: &Project) -> Result<impl View> {
    Ok(view! {
        <nav class="breadcrumb" aria-label="現在地">
            if project.is_archived() {
                <span class="breadcrumb-item">
                    <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                    (project.name.as_str())
                </span>
            } else {
                <a class="breadcrumb-item" href=(format!("/{}", project.slug))>
                    <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                    (project.name.as_str())
                </a>
            }
            <span class="breadcrumb-separator" aria-hidden="true">"/"</span>
            <span aria-current="page">"設定"</span>
        </nav>
    })
}

/// 「このプロジェクトをアーカイブ」。アーカイブ済みなら「元に戻す」。
#[component]
async fn archive_section(cx: &Cx, project: &Project) -> Result<impl View> {
    let error = signal(cx, String::new);
    let is_busy = signal(cx, || false);
    let id = project.id.to_string();
    Ok(view! {
        <section class="panel settings-section" aria-labelledby="archive-heading">
            if project.is_archived() {
                <h2 id="archive-heading">"アーカイブから戻す"</h2>
                <p>"戻すと、一覧とサイドバーにまた出ます。"</p>
                <div class="form-actions form-actions--start">
                    <button
                        type="button"
                        class="button button--outline"
                        :disabled=$(is_busy.get())
                        @click=$(async |_e: Event| {
                            is_busy.set(true);
                            let failure = restore_project(id).await;
                            if failure.is_none() {
                                raw!("location.reload()");
                            } else {
                                is_busy.set(false);
                                error.set(failure.unwrap());
                            }
                        })
                    >"元に戻す"</button>
                </div>
            } else {
                <h2 id="archive-heading">"このプロジェクトをアーカイブ"</h2>
                <p>"一覧とサイドバーに出なくなります。資料と版の履歴はそのまま残り、アーカイブからいつでも戻せます。"</p>
                <div class="form-actions form-actions--start">
                    <button
                        type="button"
                        class="button button--outline"
                        :disabled=$(is_busy.get())
                        @click=$(async |_e: Event| {
                            is_busy.set(true);
                            let result = archive_project(id).await;
                            if result.is_ok() {
                                let url = result.unwrap();
                                raw!("location.assign(${url})", drop(url));
                            } else {
                                is_busy.set(false);
                                error.set(result.unwrap_err());
                            }
                        })
                    >"アーカイブする"</button>
                </div>
            }
            form_error(error: &error)
        </section>
    })
}

/// 設定を保存する。成功したら（URL を変えたなら新しい）設定の URL、失敗したら画面に出す文言を返す。
#[procedure]
pub async fn update_project(
    cx: &Cx,
    id: String,
    form: ProjectForm,
) -> Result<Result<String, String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(update(app, &id, form).await)
}

async fn update(app: &WebApp, id: &str, form: ProjectForm) -> Result<String, String> {
    let id = project_form::parse_project_id(id)?;
    app.update_project
        .execute(id, &form.into())
        .await
        .map(|project| format!("/{}/settings?{SAVED_QUERY}", project.slug))
        .map_err(project_form::error_message)
}

/// アーカイブする。成功したらアーカイブの一覧の URL、失敗したら画面に出す文言を返す。
#[procedure]
pub async fn archive_project(cx: &Cx, id: String) -> Result<Result<String, String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(archive(app, &id).await)
}

async fn archive(app: &WebApp, id: &str) -> Result<String, String> {
    let id = project_form::parse_project_id(id)?;
    app.archive_project
        .execute(id)
        .await
        .map(|_| "/archive".to_owned())
        .map_err(project_form::error_message)
}

#[cfg(test)]
mod tests {
    use galley_core::{app::project::ProjectInput, domain::shared::ProjectId};

    use super::*;
    use crate::pages::test_app;

    type TestResult = std::result::Result<(), String>;

    fn form(slug: &str) -> ProjectForm {
        ProjectForm {
            name: "営業企画".to_owned(),
            slug: slug.to_owned(),
            description: "提案書と見積もり".to_owned(),
            color: "purple".to_owned(),
        }
    }

    async fn create(app: &WebApp, slug: &str) -> std::result::Result<Project, String> {
        app.create_project
            .execute(&ProjectInput {
                slug: slug.to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .map_err(|e| e.to_string())
    }

    #[tokio::test]
    async fn update_returns_settings_url_of_new_slug() -> TestResult {
        let app = test_app::new().await;
        let project = create(&app, "sales").await?;
        let id = project.id.to_string();

        assert_eq!(
            update(&app, &id, form("sales-2026")).await?,
            "/sales-2026/settings?saved=1"
        );
        let updated = app
            .find_project
            .execute("sales-2026")
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(updated.id, project.id);
        assert_eq!(updated.description.as_str(), "提案書と見積もり");
        assert!(app.find_project.execute("sales").await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_other_projects_slug_and_unknown_id() -> TestResult {
        let app = test_app::new().await;
        let project = create(&app, "sales").await?;
        create(&app, "planning").await?;

        assert_eq!(
            update(&app, &project.id.to_string(), form("planning")).await,
            Err(
                "URL「planning」はほかのプロジェクト（アーカイブしたものを含む）が使っています"
                    .to_owned()
            )
        );
        assert_eq!(
            update(&app, "not-an-id", form("sales")).await,
            Err("プロジェクトが見つかりません".to_owned())
        );
        assert_eq!(
            update(&app, &ProjectId::generate().to_string(), form("sales")).await,
            Err("プロジェクトが見つかりません".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn archive_hides_project_from_active_list() -> TestResult {
        let app = test_app::new().await;
        let project = create(&app, "sales").await?;

        assert_eq!(archive(&app, &project.id.to_string()).await?, "/archive");
        let archived = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        assert!(archived.is_archived());
        let active = app.sidebar_projects().await.map_err(|e| e.to_string())?;
        assert!(active.is_empty());
        Ok(())
    }
}
