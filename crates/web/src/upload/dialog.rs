//! 登録の画面の骨組み：ドラッグ中の重ね表示、確認のダイアログ「資料を登録」、登録できない資料のダイアログ。
//!
//! 中身（ファイル名、登録先の候補、該当箇所の一覧など）は `assets/upload.js` が入れる。

use galley_core::domain::{
    document::{DOCUMENT_TITLE_MAX_CHARS, UNTITLED_DOCUMENT},
    project::ProjectSummary,
    revision::{AUTHOR_NAME_MAX_CHARS, REVISION_MESSAGE_MAX_CHARS},
    upload::HtmlSizeLimit,
};
use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

/// 登録の画面一式。`projects` は登録先に選べるプロジェクト（アーカイブしたものを除く）、
/// `author` はブラウザに保存した更新者名、`size_limit` は1ファイルのサイズ上限。
#[component]
pub async fn upload_dialogs<'a>(
    projects: &'a [ProjectSummary],
    author: Option<&'a str>,
    size_limit: HtmlSizeLimit,
) -> Result<impl View> {
    Ok(view! {
        drop_overlay()
        register_dialog(projects: projects, author: author, size_limit: size_limit)
        rejected_dialog()
    })
}

/// ドラッグ中に画面全体に重ねる受け皿。資料の iframe（別オリジン）より上に置き、ドロップを受け止める。
#[component]
async fn drop_overlay() -> Result<impl View> {
    Ok(view! {
        <div class="upload-drop" data-upload-drop="" hidden="">
            <div class="upload-drop-card">
                <span class="upload-drop-icon">(Unescaped::new_unchecked(UPLOAD_SVG))</span>
                <p class="upload-drop-title" data-drop-title="">"離すと登録できます"</p>
                // ブラウザはドロップするまでファイル名を教えないので、ここでは種類だけ出す
                <p class="upload-drop-file" data-drop-file="">"HTML ファイル"</p>
                <p class="upload-drop-note" data-drop-note=""></p>
            </div>
        </div>
    })
}

/// 確認のダイアログ「資料を登録」。
#[component]
async fn register_dialog<'a>(
    projects: &'a [ProjectSummary],
    author: Option<&'a str>,
    size_limit: HtmlSizeLimit,
) -> Result<impl View> {
    Ok(view! {
        <dialog
            id="upload-dialog"
            class="dialog upload-dialog"
            aria-labelledby="upload-dialog-title"
            data-max-bytes=(size_limit.bytes().to_string())
        >
            <form method="dialog" class="dialog-body" novalidate="">
                <div class="upload-head">
                    <h2 id="upload-dialog-title">"資料を登録"</h2>
                    <p data-upload-where=""></p>
                </div>
                if projects.is_empty() {
                    <p class="upload-no-project">
                        "資料はプロジェクトにまとめて登録します。まだプロジェクトがないので、先に作ってください。"
                        " " <a href="/new">"プロジェクトを作る"</a>
                    </p>
                }
                <div class="upload-tabs" role="tablist" aria-label="登録のしかた">
                    <button type="button" class="upload-tab" role="tab" id="upload-tab-file" aria-controls="upload-panel-file" aria-selected="true" data-upload-tab="file">"ファイル"</button>
                    <button type="button" class="upload-tab" role="tab" id="upload-tab-paste" aria-controls="upload-panel-paste" aria-selected="false" tabindex="-1" data-upload-tab="paste">"貼り付け"</button>
                </div>
                <div class="upload-panel" role="tabpanel" id="upload-panel-file" aria-labelledby="upload-tab-file" data-upload-panel="file">
                    <div class="upload-file upload-file--empty" data-file-empty="">
                        <p>"HTML ファイルを画面にドロップするか、ファイルを選んでください。"</p>
                        <button type="button" class="button button--outline button--small" data-action="pick-file">"ファイルを選ぶ"</button>
                    </div>
                    <div class="upload-file" data-file-chosen="" hidden="">
                        <span class="upload-file-icon">(Unescaped::new_unchecked(FILE_SVG))</span>
                        <div class="upload-file-body">
                            <p class="upload-file-name" data-file-name=""></p>
                            <p class="upload-file-meta">
                                <span data-file-size=""></span>
                                " · "
                                <span class="upload-status" data-file-status="" aria-live="polite"></span>
                            </p>
                        </div>
                        <button type="button" class="button button--outline button--small" data-action="pick-file">"別のファイルを選ぶ"</button>
                    </div>
                    <input type="file" accept=".html,.htm,text/html" hidden="" tabindex="-1" data-file-input="">
                </div>
                <div class="upload-panel" role="tabpanel" id="upload-panel-paste" aria-labelledby="upload-tab-paste" data-upload-panel="paste" hidden="">
                    <label class="field">
                        "HTML"
                        <textarea
                            class="input textarea upload-paste"
                            name="html"
                            spellcheck="false"
                            autocomplete="off"
                            placeholder="AI のチャットからコピーした HTML を貼り付けます"
                        ></textarea>
                    </label>
                    <p class="upload-status field-hint" data-paste-status="" aria-live="polite"></p>
                </div>

                <label class="field" data-project-field="" hidden="">
                    "プロジェクト"
                    <select class="select" name="project">
                        #[key(summary.project.id.to_string())]
                        for summary in projects {
                            <option value=(summary.project.slug.as_str())>(summary.project.name.as_str())</option>
                        }
                    </select>
                </label>

                <fieldset class="upload-targets" data-targets="">
                    <legend>"登録先"</legend>
                    <label class="upload-target" data-target-suggested="" hidden="">
                        <input type="radio" name="target" value="suggested">
                        <span class="upload-target-body">
                            <span class="upload-target-title"><span data-suggested-title=""></span>" の新しい版"</span>
                            <span class="tag upload-match" data-suggested-match="" hidden="">"ファイル名が一致"</span>
                        </span>
                        <span class="version" data-suggested-version=""></span>
                    </label>
                    <label class="upload-target">
                        <input type="radio" name="target" value="new">
                        <span class="upload-target-body">
                            <span class="upload-target-title">"新しい資料"</span>
                        </span>
                    </label>
                    <label class="upload-target" data-target-other="" hidden="">
                        <input type="radio" name="target" value="other">
                        <span class="upload-target-body">
                            <span class="upload-target-title">"ほかの資料の新しい版"</span>
                        </span>
                    </label>
                    <div class="upload-other" data-other="" hidden="">
                        <input type="search" class="input" placeholder="資料名で探す" aria-label="資料名で探す" autocomplete="off" data-other-filter="">
                        <ul class="upload-other-list" data-other-list="" aria-label="資料"></ul>
                        <p class="field-hint" data-other-empty="" hidden="">"合う資料はありません"</p>
                    </div>
                    <p class="field-hint" data-targets-status="" hidden=""></p>
                </fieldset>

                <label class="field" data-title-field="">
                    "資料名"
                    <input class="input" name="title" maxlength=(DOCUMENT_TITLE_MAX_CHARS.to_string()) placeholder=(UNTITLED_DOCUMENT) autocomplete="off">
                </label>
                <label class="field">
                    "何を変えたか（任意）"
                    <input class="input" name="message" maxlength=(REVISION_MESSAGE_MAX_CHARS.to_string()) placeholder="例：売上の見込みを最新の数字にした" autocomplete="off">
                </label>
                <label class="field">
                    "更新者"
                    <input class="input" name="author" maxlength=(AUTHOR_NAME_MAX_CHARS.to_string()) autocomplete="name" value=(author.unwrap_or_default()) required="">
                    <span class="field-hint">"このブラウザに保存され、版の更新者として残ります。"</span>
                </label>
                <p class="form-error" data-upload-error="" role="alert" hidden=""></p>
                <div class="dialog-actions">
                    <button type="button" class="button button--outline" data-action="close-upload">"やめる" <kbd>"esc"</kbd></button>
                    <button type="submit" class="button button--primary" data-upload-submit="">
                        <span data-submit-label="">"新しい資料として登録"</span>
                        <kbd>"↵"</kbd>
                    </button>
                </div>
            </form>
        </dialog>
    })
}

/// 登録できない資料のダイアログ（外部のファイルを読み込んでいるとき）。
#[component]
async fn rejected_dialog() -> Result<impl View> {
    Ok(view! {
        <dialog id="upload-rejected" class="dialog upload-rejected" aria-labelledby="upload-rejected-title">
            <div class="dialog-body">
                <div class="upload-head">
                    <h2 id="upload-rejected-title">"外部のファイルを読み込んでいるため、登録できません"</h2>
                    <p class="upload-file-meta">
                        <span data-rejected-name=""></span>
                        " · "
                        <span data-rejected-size=""></span>
                    </p>
                </div>
                <p>
                    "Galley に登録できるのは、外部のファイル（CDN のスクリプト、Web フォント、外部の画像など）を読み込まない、1ファイルで完結した HTML だけです。次の "
                    <strong data-rejected-total=""></strong>
                    " か所で外部のファイルを読み込んでいます。"
                </p>
                <div class="upload-findings-wrap">
                    <table class="upload-findings">
                        <thead>
                            <tr>
                                <th scope="col">"行"</th>
                                <th scope="col">"種類"</th>
                                <th scope="col">"該当するコード"</th>
                            </tr>
                        </thead>
                        <tbody data-rejected-rows=""></tbody>
                    </table>
                </div>
                <p data-rejected-more="" hidden=""></p>
                <section class="upload-ai" aria-labelledby="upload-ai-title">
                    <h3 id="upload-ai-title">"AI への依頼文"</h3>
                    <p>"資料を作った AI にこの文を送ると、登録できる形に作り直してもらえます。"</p>
                    <p class="upload-ai-text" data-rejected-request=""></p>
                    <div>
                        <button type="button" class="button button--outline button--small" data-copy-text="" data-rejected-copy="">
                            <span data-copy-label="">"依頼文をコピー"</span>
                        </button>
                    </div>
                </section>
                <div class="dialog-actions">
                    <button type="button" class="button button--outline" data-action="close-rejected">"閉じる"</button>
                    <button type="button" class="button button--primary" data-action="retry-upload">"別のファイルを選ぶ"</button>
                </div>
            </div>
        </dialog>
    })
}

const UPLOAD_SVG: &str = r#"<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 16V4"/><path d="M7 9l5-5 5 5"/><path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3"/></svg>"#;
const FILE_SVG: &str = r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/><path d="M10 13l-2 2 2 2"/><path d="M14 13l2 2-2 2"/></svg>"#;
