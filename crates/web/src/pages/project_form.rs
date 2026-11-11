//! プロジェクトの作成（`/new`）と設定（`/{project}/settings`）で共有する入力欄と、エラーの文言。

use galley_core::{
    app::{error::AppError, project::ProjectInput},
    domain::{
        project::{CrestColor, PROJECT_DESCRIPTION_MAX_CHARS, PROJECT_NAME_MAX_CHARS},
        shared::{ProjectId, SLUG_MAX_LEN, SlugError},
    },
};
use topcoat::{
    Result,
    context::Cx,
    runtime::{Event, Signal, record},
    view::{Unescaped, View, component, view},
};

use super::documents;
use crate::{WebApp, icon};

/// 画面から送るプロジェクトの入力（procedure の引数）。ブラウザから来る値なので、検査は core に任せる。
#[record]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectForm {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub color: String,
}

impl From<ProjectForm> for ProjectInput {
    fn from(form: ProjectForm) -> Self {
        let ProjectForm {
            name,
            slug,
            description,
            color,
        } = form;
        Self {
            slug: slug.trim().to_owned(),
            name,
            description,
            color,
        }
    }
}

/// 紋章の色の名前（画面の言葉）。
pub fn crest_label(color: CrestColor) -> &'static str {
    match color {
        CrestColor::Red => "赤",
        CrestColor::Blue => "青",
        CrestColor::Green => "緑",
        CrestColor::Gold => "金",
        CrestColor::Purple => "紫",
        CrestColor::Silver => "銀",
    }
}

/// URL の欄の前に出す `<ホスト>/` のホスト。`GALLEY_APP_URL` があればそのホスト、無ければリクエストの Host。
pub fn app_host(cx: &Cx, app: &WebApp) -> Option<String> {
    app.app_url.display_host(documents::host(cx))
}

/// ブラウザから来たプロジェクトの ID を読む。読めなければ「見つかりません」にする。
pub fn parse_project_id(id: &str) -> Result<ProjectId, String> {
    ProjectId::parse(id).map_err(|_| error_message(AppError::ProjectNotFound))
}

/// ユースケースのエラーを、画面に出す文言にする。入力で直せないもの（DB の障害など）はログに残し、
/// 「もう一度」を促す文言にする。
pub fn error_message(error: AppError) -> String {
    match error {
        AppError::InvalidSlug(error) => slug_message(&error),
        AppError::InvalidCrestColor(_) => "紋章の色を選んでください".to_owned(),
        AppError::ProjectSlugTaken(slug) => {
            format!("URL「{slug}」はほかのプロジェクト（アーカイブしたものを含む）が使っています")
        }
        AppError::Repository(_) | AppError::Storage(_) => {
            tracing::error!(%error, "プロジェクトの操作に失敗しました");
            "保存できませんでした。時間をおいて、もう一度試してください".to_owned()
        }
        // ほかは core の文言が利用者向けに書いてあるので、そのまま出す（新しいエラーもここに来る）
        error => error.to_string(),
    }
}

/// slug の検査エラーを、画面の言葉（「URL」）で言い直す。
fn slug_message(error: &SlugError) -> String {
    match error {
        SlugError::Empty => "URL を入力してください".to_owned(),
        SlugError::TooLong => format!("URL は {SLUG_MAX_LEN} 文字までです"),
        SlugError::InvalidCharacter => {
            "URL に使えるのは英小文字・数字・ハイフン（-）だけです".to_owned()
        }
        SlugError::InvalidHyphen => {
            "URL の先頭と末尾にはハイフンを使えず、ハイフンを続けることもできません".to_owned()
        }
        SlugError::Reserved(word) => {
            format!("「{word}」は Galley の画面で使うため、URL にできません")
        }
    }
}

/// 名前の欄。
#[component]
pub async fn name_field(name: &Signal<String>, initial: &str) -> Result<impl View> {
    Ok(view! {
        <label class="field">
            "名前"
            <input
                class="input"
                name="name"
                value=(initial)
                maxlength=(PROJECT_NAME_MAX_CHARS)
                placeholder="営業企画"
                required=""
                @input=$(|e: Event| name.set(e.target.value))
            >
        </label>
    })
}

/// URL（slug）の欄。`<ホスト>/` に続けて入力する。
#[component]
pub async fn slug_field(
    slug: &Signal<String>,
    initial: &str,
    host: Option<&str>,
    #[default] note: Option<&str>,
) -> Result<impl View> {
    Ok(view! {
        <div class="field">
            <label for="project-slug">"URL"</label>
            <div class="slug-input">
                <span class="slug-prefix">(host.unwrap_or_default()) "/"</span>
                <input
                    id="project-slug"
                    class="input"
                    name="slug"
                    value=(initial)
                    maxlength=(SLUG_MAX_LEN)
                    placeholder="sales-plan"
                    autocapitalize="off"
                    spellcheck="false"
                    required=""
                    aria-describedby="project-slug-hint"
                    @input=$(|e: Event| slug.set(e.target.value))
                >
            </div>
            <p class="field-hint" id="project-slug-hint">
                match note {
                    Some(note) => (note),
                    None => "英小文字・数字・ハイフン（-）で入力します",
                }
            </p>
        </div>
    })
}

/// 説明の欄。
#[component]
pub async fn description_field(description: &Signal<String>, initial: &str) -> Result<impl View> {
    Ok(view! {
        <label class="field">
            "説明"
            <textarea
                class="input textarea"
                name="description"
                rows="3"
                maxlength=(PROJECT_DESCRIPTION_MAX_CHARS)
                placeholder="資料一覧の見出しの下に出ます"
                @input=$(|e: Event| description.set(e.target.value))
            >(initial)</textarea>
        </label>
    })
}

/// 紋章の色（6色から1つ選ぶ）。
#[component]
pub async fn crest_picker(color: &Signal<String>, initial: CrestColor) -> Result<impl View> {
    Ok(view! {
        <fieldset class="field crest-picker">
            <legend>"紋章の色"</legend>
            <div class="crest-options">
                for option in CrestColor::ALL {
                    <label class="crest-option">
                        <input
                            type="radio"
                            name="color"
                            value=(option.as_str())
                            checked=(option == initial)
                            @change=$(|e: Event| color.set(e.target.value))
                        >
                        <span class=(format!("crest-swatch crest--{}", option.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                        <span>(crest_label(option))</span>
                    </label>
                }
            </div>
            <p class="field-hint">"一覧でプロジェクトを見分ける印になります"</p>
        </fieldset>
    })
}

/// 操作のエラー（`error` が空なら隠す）。
#[component]
pub async fn form_error(error: &Signal<String>) -> Result<impl View> {
    Ok(view! {
        <p class="form-error" role="alert" :hidden=$(error.get().is_empty())>$(error.get())</p>
    })
}

#[cfg(test)]
mod tests {
    use galley_core::domain::{error::RepositoryError, shared::TextError};

    use super::*;

    #[test]
    fn form_becomes_input_with_trimmed_slug() {
        let input = ProjectInput::from(ProjectForm {
            name: " 営業企画 ".to_owned(),
            slug: " sales ".to_owned(),
            description: "説明".to_owned(),
            color: "blue".to_owned(),
        });
        assert_eq!(
            input,
            ProjectInput {
                slug: "sales".to_owned(),
                name: " 営業企画 ".to_owned(),
                description: "説明".to_owned(),
                color: "blue".to_owned(),
            }
        );
    }

    #[test]
    fn every_color_has_a_label() {
        let labels: Vec<_> = CrestColor::ALL.into_iter().map(crest_label).collect();
        assert_eq!(labels, ["赤", "青", "緑", "金", "紫", "銀"]);
    }

    #[test]
    fn slug_errors_use_the_word_url() {
        for error in [
            SlugError::Empty,
            SlugError::TooLong,
            SlugError::InvalidCharacter,
            SlugError::InvalidHyphen,
            SlugError::Reserved("archive".to_owned()),
        ] {
            let message = error_message(AppError::InvalidSlug(error));
            assert!(message.contains("URL"), "{message}");
            assert!(!message.contains("slug"), "{message}");
        }
        assert_eq!(
            error_message(AppError::InvalidSlug(SlugError::Reserved("new".to_owned()))),
            "「new」は Galley の画面で使うため、URL にできません"
        );
    }

    #[test]
    fn other_errors_keep_core_wording_or_hide_internals() {
        assert_eq!(
            error_message(AppError::InvalidText(TextError::Empty {
                field: "プロジェクト名"
            })),
            "プロジェクト名を入力してください"
        );
        assert_eq!(
            error_message(AppError::ProjectSlugTaken("sales".to_owned())),
            "URL「sales」はほかのプロジェクト（アーカイブしたものを含む）が使っています"
        );
        let internal = error_message(AppError::Repository(RepositoryError::Corrupted(
            "disk I/O error".to_owned(),
        )));
        assert!(!internal.contains("disk"), "{internal}");
    }
}
