//! 資料の登録・新しい版の追加・過去の版に戻す処理を、DB とストレージを使って確かめる。

use std::{error::Error, path::PathBuf, sync::Arc};

use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::{
        document::{ArchiveDocument, CreateDocument, CreateDocumentInput, SlugSource},
        error::AppError,
        project::{ArchiveProject, CreateProject, ProjectInput},
        revision::{
            AddRevision, FindRevision, RegisteredRevision, RevertToRevision, RevisionInput,
        },
    },
    domain::{
        blob::{BlobHash, BlobStore},
        document::{DocumentError, DocumentRepository},
        project::{Project, ProjectError},
        revision::{RevisionNumber, RevisionRepository, RevisionSource},
        shared::DocumentId,
        upload::UploadError,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    database: SqliteDatabase,
    blobs: Arc<ObjectStoreBlobStore>,
    project: Project,
    _dir: Option<TempDir>,
}

/// テストごとの一時ディレクトリ。終わったら消す。
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Fixture {
    async fn new() -> Result<Self, Box<dyn Error>> {
        let database = SqliteDatabase::in_memory().await?;
        Self::with_database(database, None).await
    }

    /// ファイルの DB（同時の書き込みを確かめる）。
    async fn with_file() -> Result<Self, Box<dyn Error>> {
        let dir =
            TempDir(std::env::temp_dir().join(format!("galley-test-{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(&dir.0)?;
        let database = SqliteDatabase::open(&dir.0.join("galley.db")).await?;
        Self::with_database(database, Some(dir)).await
    }

    async fn with_database(
        database: SqliteDatabase,
        dir: Option<TempDir>,
    ) -> Result<Self, Box<dyn Error>> {
        let project = CreateProject::new(Arc::new(database.projects()))
            .execute(&ProjectInput {
                slug: "sales".to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "blue".to_owned(),
            })
            .await?;
        Ok(Self {
            database,
            blobs: Arc::new(ObjectStoreBlobStore::in_memory()),
            project,
            _dir: dir,
        })
    }

    fn create_document(&self) -> CreateDocument {
        CreateDocument::new(
            Arc::new(self.database.projects()),
            Arc::new(self.database.documents()),
            self.blobs.clone(),
        )
    }

    fn add_revision(&self) -> AddRevision {
        AddRevision::new(
            Arc::new(self.database.documents()),
            Arc::new(self.database.revisions()),
            self.blobs.clone(),
        )
    }

    fn revert(&self) -> RevertToRevision {
        RevertToRevision::new(
            Arc::new(self.database.documents()),
            Arc::new(self.database.revisions()),
        )
    }

    async fn create(&self, file_name: &str, html: &str) -> Result<RegisteredRevision, AppError> {
        self.create_document()
            .execute(CreateDocumentInput {
                project_id: self.project.id,
                title: None,
                slug: SlugSource::FileName(file_name.to_owned()),
                revision: input(html, "初版", "佐藤"),
            })
            .await
    }

    async fn add(&self, id: DocumentId, html: &str) -> Result<RegisteredRevision, AppError> {
        self.add_revision()
            .execute(id, input(html, "数字を更新", "田中"))
            .await
    }
}

fn input(html: &str, message: &str, author: &str) -> RevisionInput {
    RevisionInput {
        html: html.as_bytes().to_vec(),
        message: message.to_owned(),
        author_name: author.to_owned(),
        source: RevisionSource::Web,
    }
}

const V1: &str = "<!doctype html><title>事業計画 2026</title><p>第1版";
const V2: &str = "<!doctype html><title>事業計画 2026</title><p>第2版";

// ---- 新しい資料 ----

#[tokio::test]
async fn create_stores_first_revision_and_blob() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("jigyo-keikaku.html", V1).await?;

    assert_eq!(created.document.slug.as_str(), "jigyo-keikaku");
    assert_eq!(created.document.title.as_str(), "事業計画 2026");
    assert_eq!(created.document.current_revision_id, created.revision.id);
    assert_eq!(created.revision.number, RevisionNumber::FIRST);
    assert_eq!(created.revision.message.as_str(), "初版");
    assert_eq!(created.revision.author_name.as_str(), "佐藤");
    assert_eq!(created.revision.blob_hash, BlobHash::of(V1.as_bytes()));

    let stored = fx.blobs.get(&created.revision.blob_hash).await?;
    assert_eq!(stored.map(|c| c.into_bytes()), Some(V1.as_bytes().to_vec()));
    let found = fx
        .database
        .documents()
        .find_by_id(created.document.id)
        .await?;
    assert_eq!(found, Some(created.document));
    Ok(())
}

#[tokio::test]
async fn create_uses_input_title_and_avoids_slug_collision() -> TestResult {
    let fx = Fixture::new().await?;
    fx.create("plan.html", V1).await?;
    let second = fx
        .create_document()
        .execute(CreateDocumentInput {
            project_id: fx.project.id,
            title: Some("別の計画".to_owned()),
            slug: SlugSource::FileName("plan.html".to_owned()),
            revision: input(V2, "", "佐藤"),
        })
        .await?;
    assert_eq!(second.document.slug.as_str(), "plan-2");
    assert_eq!(second.document.title.as_str(), "別の計画");
    Ok(())
}

#[tokio::test]
async fn create_falls_back_to_untitled() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("memo.html", "<p>題のない資料").await?;
    assert_eq!(created.document.title.as_str(), "無題の資料");
    Ok(())
}

#[tokio::test]
async fn create_rejects_archived_or_missing_project() -> TestResult {
    let fx = Fixture::new().await?;
    ArchiveProject::new(Arc::new(fx.database.projects()))
        .execute(fx.project.id)
        .await?;
    assert!(matches!(
        fx.create("plan.html", V1).await,
        Err(AppError::Project(ProjectError::Archived))
    ));

    let missing = fx
        .create_document()
        .execute(CreateDocumentInput {
            project_id: galley_core::domain::shared::ProjectId::generate(),
            title: None,
            slug: SlugSource::None,
            revision: input(V1, "", "佐藤"),
        })
        .await;
    assert!(matches!(missing, Err(AppError::ProjectNotFound)));
    Ok(())
}

#[tokio::test]
async fn rejected_html_is_not_stored() -> TestResult {
    let fx = Fixture::new().await?;
    let html = r#"<script src="https://cdn.example.com/chart.js"></script>"#;
    let result = fx.create("chart.html", html).await;
    assert!(matches!(
        result,
        Err(AppError::InvalidUpload(UploadError::ExternalResources(_)))
    ));
    assert_eq!(fx.blobs.get(&BlobHash::of(html.as_bytes())).await?, None);
    assert!(
        fx.database
            .documents()
            .slugs_with_prefix(fx.project.id, "chart")
            .await?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn author_name_is_checked_before_html() -> TestResult {
    let fx = Fixture::new().await?;
    let result = fx
        .create_document()
        .execute(CreateDocumentInput {
            project_id: fx.project.id,
            title: None,
            slug: SlugSource::None,
            revision: input("<script src=https://e.com/a.js></script>", "", " "),
        })
        .await;
    assert!(matches!(result, Err(AppError::InvalidText(_))));
    Ok(())
}

// ---- 新しい版 ----

#[tokio::test]
async fn add_revision_increments_number_and_updates_current() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let added = fx.add(created.document.id, V2).await?;

    assert_eq!(added.revision.number.get(), 2);
    assert_eq!(added.revision.author_name.as_str(), "田中");
    assert_eq!(added.revision.message.as_str(), "数字を更新");
    assert_eq!(added.revision.restored_from, None);
    assert!(added.document.updated_at >= created.document.updated_at);

    let found = fx
        .database
        .documents()
        .find_by_id(created.document.id)
        .await?;
    assert_eq!(
        found.map(|d| d.current_revision_id),
        Some(added.revision.id)
    );
    // 資料名は版を追加しても変わらない
    assert_eq!(added.document.title, created.document.title);
    Ok(())
}

#[tokio::test]
async fn same_content_shares_one_blob() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let again = fx.add(created.document.id, V1).await?;
    assert_eq!(again.revision.number.get(), 2);
    assert_eq!(again.revision.blob_hash, created.revision.blob_hash);
    Ok(())
}

#[tokio::test]
async fn add_revision_rejects_archived_or_missing_document() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    ArchiveDocument::new(Arc::new(fx.database.documents()))
        .execute(created.document.id)
        .await?;
    assert!(matches!(
        fx.add(created.document.id, V2).await,
        Err(AppError::Document(DocumentError::Archived))
    ));
    assert!(matches!(
        fx.add(DocumentId::generate(), V2).await,
        Err(AppError::DocumentNotFound)
    ));
    // 断ったときは実体も書かない
    assert_eq!(fx.blobs.get(&BlobHash::of(V2.as_bytes())).await?, None);
    Ok(())
}

// ---- 過去の版に戻す ----

#[tokio::test]
async fn revert_adds_new_revision_with_past_content() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    fx.add(created.document.id, V2).await?;

    let reverted = fx
        .revert()
        .execute(created.document.id, 1, "鈴木", RevisionSource::Mcp)
        .await?;
    assert_eq!(reverted.revision.number.get(), 3);
    assert_eq!(reverted.revision.blob_hash, created.revision.blob_hash);
    assert_eq!(reverted.revision.restored_from, Some(RevisionNumber::FIRST));
    assert_eq!(reverted.revision.message.as_str(), "第1版の内容に戻す");
    assert_eq!(reverted.revision.author_name.as_str(), "鈴木");
    assert_eq!(reverted.revision.source, RevisionSource::Mcp);

    // 過去の版はそのまま残る
    let revisions = fx.database.revisions();
    let first = revisions
        .find_by_number(created.document.id, RevisionNumber::FIRST)
        .await?;
    assert_eq!(first, Some(created.revision));
    let saved = revisions.find_by_id(reverted.revision.id).await?;
    assert_eq!(saved, Some(reverted.revision));
    Ok(())
}

#[tokio::test]
async fn revert_rejects_latest_and_unknown_numbers() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    fx.add(created.document.id, V2).await?;
    let revert = fx.revert();
    let id = created.document.id;

    let latest = RevisionNumber::new(2)?;
    assert!(matches!(
        revert.execute(id, 2, "鈴木", RevisionSource::Web).await,
        Err(AppError::Document(DocumentError::AlreadyLatest(n))) if n == latest
    ));
    assert!(matches!(
        revert.execute(id, 3, "鈴木", RevisionSource::Web).await,
        Err(AppError::RevisionNotFound)
    ));
    assert!(matches!(
        revert.execute(id, 0, "鈴木", RevisionSource::Web).await,
        Err(AppError::RevisionNotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn revert_rejects_archived_document() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    fx.add(created.document.id, V2).await?;
    ArchiveDocument::new(Arc::new(fx.database.documents()))
        .execute(created.document.id)
        .await?;
    assert!(matches!(
        fx.revert()
            .execute(created.document.id, 1, "鈴木", RevisionSource::Web)
            .await,
        Err(AppError::Document(DocumentError::Archived))
    ));
    Ok(())
}

#[tokio::test]
async fn find_revision_defaults_to_latest() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let added = fx.add(created.document.id, V2).await?;
    let find = FindRevision::new(Arc::new(fx.database.revisions()));

    assert_eq!(find.execute(&added.document, None).await?, added.revision);
    assert_eq!(
        find.execute(&added.document, Some(1)).await?,
        created.revision
    );
    assert!(matches!(
        find.execute(&added.document, Some(9)).await,
        Err(AppError::RevisionNotFound)
    ));
    Ok(())
}

// ---- 同時の更新 ----

/// やり直しの回数（5回）までの同時の更新は、すべて別の番号で登録できる。
/// やり直すのはほかの更新が先に登録できたときだけなので、同時に N 件なら N 回で足りる。
const MAX_SIMULTANEOUS: usize = 5;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_additions_never_share_a_number() -> TestResult {
    let fx = Arc::new(Fixture::with_file().await?);
    let created = fx.create("plan.html", V1).await?;
    let id = created.document.id;

    let tasks: Vec<_> = (0..MAX_SIMULTANEOUS)
        .map(|i| {
            let fx = fx.clone();
            tokio::spawn(async move {
                fx.add(id, &format!("<title>計画</title><p>同時の更新 {i}"))
                    .await
            })
        })
        .collect();
    let mut numbers = Vec::new();
    for task in tasks {
        numbers.push(task.await??.revision.number.get());
    }
    numbers.sort_unstable();
    let expected: Vec<u32> = (2..2 + MAX_SIMULTANEOUS as u32).collect();
    assert_eq!(numbers, expected);

    let current = fx.database.documents().find_by_id(id).await?;
    let latest = fx
        .database
        .revisions()
        .find_by_number(id, RevisionNumber::new(*numbers.last().unwrap_or(&1))?)
        .await?;
    assert_eq!(current.map(|d| d.current_revision_id), latest.map(|r| r.id));
    Ok(())
}
