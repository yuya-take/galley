//! 資料と版の物理削除（`galley purge`）を、DB とストレージを使って確かめる。

use std::{error::Error, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::{
        document::{
            ArchiveDocument, CreateDocument, CreateDocumentInput, FindDocument, PurgeDocument,
            RevisionLocation, SlugSource,
        },
        error::AppError,
        project::{ArchiveProject, CreateProject, ProjectInput},
        revision::{
            AddRevision, FindRevision, ReadRevisionContent, RegisteredRevision, RevertToRevision,
            RevisionInput,
        },
    },
    domain::{
        blob::{Blob, BlobHash, BlobStore},
        document::{
            Document, DocumentError, DocumentListItem, DocumentQuery, DocumentRepository,
            DocumentSlug, PurgedRevisions,
        },
        error::RepositoryError,
        project::{Project, ProjectSlug},
        revision::{
            AuthorName, NewRevision, Revision, RevisionMessage, RevisionNumber, RevisionRepository,
            RevisionSource,
        },
        shared::{DocumentId, ProjectId, RevisionId},
        upload::HtmlSizeLimit,
    },
};
use jiff::Timestamp;

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

impl TempDir {
    fn new() -> Result<Self, Box<dyn Error>> {
        let dir =
            TempDir(std::env::temp_dir().join(format!("galley-test-{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(&dir.0)?;
        Ok(dir)
    }
}

impl Fixture {
    async fn new() -> Result<Self, Box<dyn Error>> {
        Self::with_database(SqliteDatabase::in_memory().await?).await
    }

    async fn with_database(database: SqliteDatabase) -> Result<Self, Box<dyn Error>> {
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
            _dir: None,
        })
    }

    fn purge(&self) -> PurgeDocument {
        self.purge_with(Arc::new(self.database.documents()))
    }

    fn purge_with(&self, documents: Arc<dyn DocumentRepository>) -> PurgeDocument {
        PurgeDocument::new(
            Arc::new(self.database.projects()),
            documents,
            Arc::new(self.database.revisions()),
            self.blobs.clone(),
        )
    }

    fn create_document(&self) -> CreateDocument {
        CreateDocument::new(
            Arc::new(self.database.projects()),
            Arc::new(self.database.documents()),
            self.blobs.clone(),
            HtmlSizeLimit::DEFAULT,
        )
    }

    async fn create(&self, file_name: &str, html: &str) -> Result<RegisteredRevision, AppError> {
        self.create_document()
            .execute(CreateDocumentInput {
                project_id: self.project.id,
                title: None,
                slug: SlugSource::FileName(file_name.to_owned()),
                revision: input(html),
            })
            .await
    }

    async fn add(&self, id: DocumentId, html: &str) -> Result<RegisteredRevision, AppError> {
        AddRevision::new(
            Arc::new(self.database.documents()),
            Arc::new(self.database.revisions()),
            self.blobs.clone(),
            HtmlSizeLimit::DEFAULT,
        )
        .execute(id, input(html))
        .await
    }

    async fn revert(&self, id: DocumentId, number: u32) -> Result<RegisteredRevision, AppError> {
        RevertToRevision::new(
            Arc::new(self.database.documents()),
            Arc::new(self.database.revisions()),
        )
        .execute(id, number, "鈴木", RevisionSource::Web)
        .await
    }

    async fn numbers(&self, id: DocumentId) -> Result<Vec<u32>, Box<dyn Error>> {
        Ok(self
            .database
            .revisions()
            .list_by_document(id, None)
            .await?
            .iter()
            .map(|revision| revision.number.get())
            .collect())
    }

    async fn has_blob(&self, html: &str) -> Result<bool, Box<dyn Error>> {
        Ok(self
            .blobs
            .get(&BlobHash::of(html.as_bytes()))
            .await?
            .is_some())
    }
}

fn input(html: &str) -> RevisionInput {
    RevisionInput {
        html: html.as_bytes().to_vec(),
        message: "更新".to_owned(),
        author_name: "佐藤".to_owned(),
        source: RevisionSource::Web,
    }
}

fn hash(html: &str) -> BlobHash {
    BlobHash::of(html.as_bytes())
}

fn location(document: &str, number: u32) -> Result<RevisionLocation, Box<dyn Error>> {
    Ok(RevisionLocation {
        project: ProjectSlug::parse("sales")?,
        document: DocumentSlug::parse(document)?,
        number: RevisionNumber::new(number)?,
    })
}

const V1: &str = "<!doctype html><title>事業計画</title><p>第1版";
const V2: &str = "<!doctype html><title>事業計画</title><p>機密を含む第2版";
const V3: &str = "<!doctype html><title>事業計画</title><p>第3版";

// ---- 資料ごと ----

#[tokio::test]
async fn purge_document_removes_revisions_and_blobs() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let id = created.document.id;
    fx.add(id, V2).await?;
    fx.revert(id, 1).await?;

    let plan = fx.purge().plan("sales", "plan", None).await?;
    assert_eq!(plan.document.id, id);
    assert_eq!(plan.revision, None);
    let numbers: Vec<u32> = plan.revisions.iter().map(|r| r.number.get()).collect();
    assert_eq!(numbers, vec![3, 2, 1]);
    // 第3版は第1版に戻した版なので、ブロブは2つ
    assert_eq!(plan.blobs, vec![hash(V1), hash(V2)]);
    assert!(plan.kept_blobs.is_empty());

    let outcome = fx.purge().execute(&plan).await?;
    let purged: Vec<u32> = outcome.purged.numbers.iter().map(|n| n.get()).collect();
    assert_eq!(purged, vec![3, 2, 1]);
    assert_eq!(outcome.purged.unreferenced_blobs, vec![hash(V1), hash(V2)]);
    assert_eq!(outcome.deleted_blobs, vec![hash(V1), hash(V2)]);
    assert!(outcome.reused_blobs.is_empty());
    assert!(outcome.failed_blobs.is_empty());

    assert_eq!(fx.database.documents().find_by_id(id).await?, None);
    assert!(fx.numbers(id).await?.is_empty());
    assert!(!fx.has_blob(V1).await?);
    assert!(!fx.has_blob(V2).await?);
    assert!(matches!(
        FindDocument::new(Arc::new(fx.database.documents()))
            .execute(fx.project.id, "plan")
            .await,
        Err(AppError::DocumentNotFound)
    ));

    // slug は空くので、同じファイル名で登録し直せる（実体も書き直される）
    let again = fx.create("plan.html", V1).await?;
    assert_eq!(again.document.slug.as_str(), "plan");
    assert!(fx.has_blob(V1).await?);
    Ok(())
}

#[tokio::test]
async fn purge_document_keeps_blob_shared_with_other_document() -> TestResult {
    let fx = Fixture::new().await?;
    let target = fx.create("plan.html", V1).await?;
    fx.add(target.document.id, V2).await?;
    let other = fx.create("copy.html", V1).await?;

    let plan = fx.purge().plan("sales", "plan", None).await?;
    assert_eq!(plan.blobs, vec![hash(V2)]);
    assert_eq!(plan.kept_blobs.len(), 1);
    assert_eq!(plan.kept_blobs[0].hash, hash(V1));
    assert_eq!(plan.kept_blobs[0].used_by, vec![location("copy", 1)?]);

    let outcome = fx.purge().execute(&plan).await?;
    assert_eq!(outcome.deleted_blobs, vec![hash(V2)]);
    assert!(fx.has_blob(V1).await?);
    assert!(!fx.has_blob(V2).await?);
    let content = ReadRevisionContent::new(Arc::new(fx.database.revisions()), fx.blobs.clone())
        .execute(other.revision.id)
        .await?;
    assert_eq!(content.html, V1.as_bytes());
    Ok(())
}

#[tokio::test]
async fn purge_works_on_archived_document_and_project() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    ArchiveDocument::new(Arc::new(fx.database.documents()))
        .execute(created.document.id)
        .await?;
    ArchiveProject::new(Arc::new(fx.database.projects()))
        .execute(fx.project.id)
        .await?;

    let plan = fx.purge().plan("sales", "plan", None).await?;
    fx.purge().execute(&plan).await?;
    assert_eq!(
        fx.database
            .documents()
            .find_by_id(created.document.id)
            .await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn plan_rejects_unknown_targets() -> TestResult {
    let fx = Fixture::new().await?;
    fx.create("plan.html", V1).await?;
    let purge = fx.purge();
    assert!(matches!(
        purge.plan("marketing", "plan", None).await,
        Err(AppError::ProjectNotFound)
    ));
    assert!(matches!(
        purge.plan("sales", "missing", None).await,
        Err(AppError::DocumentNotFound)
    ));
    assert!(matches!(
        purge.plan("Sales!", "plan", None).await,
        Err(AppError::ProjectNotFound)
    ));
    assert!(matches!(
        purge.plan("sales", "plan", Some(0)).await,
        Err(AppError::RevisionNotFound)
    ));
    assert!(matches!(
        purge.plan("sales", "plan", Some(9)).await,
        Err(AppError::RevisionNotFound)
    ));
    Ok(())
}

// ---- 版だけ ----

#[tokio::test]
async fn purge_revision_leaves_a_gap_in_numbers() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let id = created.document.id;
    fx.add(id, V2).await?;
    fx.add(id, V3).await?;

    let plan = fx.purge().plan("sales", "plan", Some(2)).await?;
    assert_eq!(plan.revision.as_ref().map(|r| r.number.get()), Some(2));
    assert_eq!(plan.blobs, vec![hash(V2)]);
    let outcome = fx.purge().execute(&plan).await?;
    let purged: Vec<u32> = outcome.purged.numbers.iter().map(|n| n.get()).collect();
    assert_eq!(purged, vec![2]);
    assert_eq!(outcome.deleted_blobs, vec![hash(V2)]);
    assert!(!fx.has_blob(V2).await?);
    assert!(fx.has_blob(V1).await?);

    // 番号は詰めない。消した版は見つからず、次の版は最新の版の次の番号になる
    assert_eq!(fx.numbers(id).await?, vec![3, 1]);
    let document = fx
        .database
        .documents()
        .find_by_id(id)
        .await?
        .ok_or("資料がありません")?;
    let find = FindRevision::new(Arc::new(fx.database.revisions()));
    assert!(matches!(
        find.execute(&document, Some(2)).await,
        Err(AppError::RevisionNotFound)
    ));
    assert_eq!(find.execute(&document, None).await?.number.get(), 3);
    assert_eq!(fx.add(id, V2).await?.revision.number.get(), 4);
    assert_eq!(fx.revert(id, 1).await?.revision.number.get(), 5);
    assert!(matches!(
        fx.revert(id, 2).await,
        Err(AppError::RevisionNotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn purge_revision_rejects_latest() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    // 版が1つだけなら、その版が最新の版
    assert!(matches!(
        fx.purge().plan("sales", "plan", Some(1)).await,
        Err(AppError::Document(DocumentError::PurgeLatest(n))) if n == RevisionNumber::FIRST
    ));

    fx.add(created.document.id, V2).await?;
    assert!(matches!(
        fx.purge().plan("sales", "plan", Some(2)).await,
        Err(AppError::Document(DocumentError::PurgeLatest(n))) if n.get() == 2
    ));
    assert!(fx.purge().plan("sales", "plan", Some(1)).await.is_ok());
    Ok(())
}

#[tokio::test]
async fn purge_revision_keeps_blob_used_by_restored_revision() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V2).await?;
    let id = created.document.id;
    fx.add(id, V1).await?;
    // 第3版は第1版（機密を含む）に戻した版
    let restored = fx.revert(id, 1).await?;
    fx.add(id, V3).await?;

    let plan = fx.purge().plan("sales", "plan", Some(1)).await?;
    assert!(plan.blobs.is_empty());
    assert_eq!(plan.kept_blobs.len(), 1);
    assert_eq!(plan.kept_blobs[0].used_by, vec![location("plan", 3)?]);

    let outcome = fx.purge().execute(&plan).await?;
    assert!(outcome.purged.unreferenced_blobs.is_empty());
    assert!(outcome.deleted_blobs.is_empty());
    assert!(fx.has_blob(V2).await?);
    let content = ReadRevisionContent::new(Arc::new(fx.database.revisions()), fx.blobs.clone())
        .execute(restored.revision.id)
        .await?;
    assert_eq!(content.html, V2.as_bytes());

    // 第3版も消せば、ブロブも消える
    let plan = fx.purge().plan("sales", "plan", Some(3)).await?;
    assert_eq!(plan.blobs, vec![hash(V2)]);
    fx.purge().execute(&plan).await?;
    assert!(!fx.has_blob(V2).await?);
    assert_eq!(fx.numbers(id).await?, vec![4, 2]);
    Ok(())
}

// ---- サーバーが動いている間 ----

#[tokio::test]
async fn revert_to_revision_purged_meanwhile_is_rejected() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V2).await?;
    let id = created.document.id;
    let second = fx.add(id, V1).await?;

    // 「この版に戻す」が第1版を読んだあと、保存する前に第1版が消された
    let mut document = second.document.clone();
    let reverted = document.revert_to(
        &second.revision,
        &created.revision,
        AuthorName::parse("鈴木")?,
        RevisionSource::Web,
        Timestamp::now(),
    )?;
    let plan = fx.purge().plan("sales", "plan", Some(1)).await?;
    fx.purge().execute(&plan).await?;
    assert!(matches!(
        fx.database
            .documents()
            .insert_revision(&document, &reverted, None)
            .await,
        Err(RepositoryError::Conflict(_))
    ));
    // 読み直すと、戻す元の版は見つからない
    assert!(matches!(
        fx.revert(id, 1).await,
        Err(AppError::RevisionNotFound)
    ));
    assert_eq!(fx.numbers(id).await?, vec![2]);
    Ok(())
}

#[tokio::test]
async fn adding_to_document_purged_meanwhile_is_rejected() -> TestResult {
    let fx = Fixture::new().await?;
    let created = fx.create("plan.html", V1).await?;
    let mut document = created.document.clone();
    let added = document.add_revision(
        &created.revision,
        NewRevision {
            blob_hash: hash(V2),
            message: RevisionMessage::parse("更新")?,
            author_name: AuthorName::parse("佐藤")?,
            source: RevisionSource::Web,
        },
        Timestamp::now(),
    )?;
    let plan = fx.purge().plan("sales", "plan", None).await?;
    fx.purge().execute(&plan).await?;
    let blob = Blob {
        hash: hash(V2),
        size: V2.len() as u64,
        created_at: Timestamp::now(),
    };
    assert!(matches!(
        fx.database
            .documents()
            .insert_revision(&document, &added, Some(&blob))
            .await,
        Err(RepositoryError::Conflict(_))
    ));
    assert!(fx.numbers(created.document.id).await?.is_empty());
    assert!(matches!(
        fx.add(created.document.id, V2).await,
        Err(AppError::DocumentNotFound)
    ));
    Ok(())
}

/// DB から消してコミットした直後に、同じ内容の資料が別に登録されたことにする。
struct RegisterAfterPurge {
    inner: Arc<dyn DocumentRepository>,
    create: CreateDocument,
    project_id: ProjectId,
}

#[async_trait]
impl DocumentRepository for RegisterAfterPurge {
    async fn insert_with_first_revision(
        &self,
        document: &Document,
        revision: &Revision,
        blob: &Blob,
    ) -> Result<(), RepositoryError> {
        self.inner
            .insert_with_first_revision(document, revision, blob)
            .await
    }

    async fn insert_revision(
        &self,
        document: &Document,
        revision: &Revision,
        blob: Option<&Blob>,
    ) -> Result<(), RepositoryError> {
        self.inner.insert_revision(document, revision, blob).await
    }

    async fn update(&self, document: &Document) -> Result<(), RepositoryError> {
        self.inner.update(document).await
    }

    async fn find_by_id(&self, id: DocumentId) -> Result<Option<Document>, RepositoryError> {
        self.inner.find_by_id(id).await
    }

    async fn find_with_project(
        &self,
        id: DocumentId,
    ) -> Result<Option<(Document, Project)>, RepositoryError> {
        self.inner.find_with_project(id).await
    }

    async fn find_by_slug(
        &self,
        project_id: ProjectId,
        slug: &DocumentSlug,
    ) -> Result<Option<Document>, RepositoryError> {
        self.inner.find_by_slug(project_id, slug).await
    }

    async fn slugs_with_prefix(
        &self,
        project_id: ProjectId,
        prefix: &str,
    ) -> Result<Vec<DocumentSlug>, RepositoryError> {
        self.inner.slugs_with_prefix(project_id, prefix).await
    }

    async fn list(&self, query: &DocumentQuery) -> Result<Vec<DocumentListItem>, RepositoryError> {
        self.inner.list(query).await
    }

    async fn purge(&self, id: DocumentId) -> Result<PurgedRevisions, RepositoryError> {
        let purged = self.inner.purge(id).await?;
        self.create
            .execute(CreateDocumentInput {
                project_id: self.project_id,
                title: None,
                slug: SlugSource::FileName("again.html".to_owned()),
                revision: input(V2),
            })
            .await
            .map_err(|err| RepositoryError::Backend(Box::new(err)))?;
        Ok(purged)
    }

    async fn purge_revision(&self, id: RevisionId) -> Result<PurgedRevisions, RepositoryError> {
        self.inner.purge_revision(id).await
    }
}

#[tokio::test]
async fn blob_registered_again_after_commit_is_kept() -> TestResult {
    let fx = Fixture::new().await?;
    fx.create("plan.html", V1).await?;
    fx.create("secret.html", V2).await?;

    let plan = fx.purge().plan("sales", "secret", None).await?;
    assert_eq!(plan.blobs, vec![hash(V2)]);
    let racing = Arc::new(RegisterAfterPurge {
        inner: Arc::new(fx.database.documents()),
        create: fx.create_document(),
        project_id: fx.project.id,
    });
    let outcome = fx.purge_with(racing).execute(&plan).await?;
    assert_eq!(outcome.purged.unreferenced_blobs, vec![hash(V2)]);
    assert!(outcome.deleted_blobs.is_empty());
    assert_eq!(outcome.reused_blobs, vec![hash(V2)]);

    // 後から登録した資料は読める
    let again = fx.purge().plan("sales", "again", None).await?;
    let content = ReadRevisionContent::new(Arc::new(fx.database.revisions()), fx.blobs.clone())
        .execute(again.revisions[0].id)
        .await?;
    assert_eq!(content.html, V2.as_bytes());
    Ok(())
}

/// サーバーと `galley purge` のように、別々に開いた DB（書き込みの順番待ちを共有しない）から
/// 同時に書き込んでも、SQLite のロックを待って両方とも終わる。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn purge_from_another_connection_while_adding() -> TestResult {
    let dir = TempDir::new()?;
    let path = dir.0.join("galley.db");
    let server = Fixture {
        _dir: Some(dir),
        ..Fixture::with_database(SqliteDatabase::open(&path).await?).await?
    };
    let server = Arc::new(server);
    let busy = server.create("busy.html", V1).await?;
    let target = server.create("plan.html", V2).await?;
    for i in 0..5 {
        server
            .add(target.document.id, &format!("<title>計画</title><p>{i}"))
            .await?;
    }

    // 別のプロセスの代わりに、同じファイルを別に開く（ストレージは同じものを使う）
    let cli_database = SqliteDatabase::open(&path).await?;
    let purge = PurgeDocument::new(
        Arc::new(cli_database.projects()),
        Arc::new(cli_database.documents()),
        Arc::new(cli_database.revisions()),
        server.blobs.clone(),
    );

    let adding = {
        let server = server.clone();
        let id = busy.document.id;
        tokio::spawn(async move {
            for i in 0..10 {
                server
                    .add(id, &format!("<title>同時</title><p>{i}"))
                    .await?;
            }
            Ok::<_, AppError>(())
        })
    };
    let plan = purge.plan("sales", "plan", None).await?;
    let outcome = purge.execute(&plan).await?;
    adding.await??;

    assert_eq!(outcome.purged.numbers.len(), 6);
    assert!(outcome.failed_blobs.is_empty());
    assert_eq!(
        server
            .database
            .documents()
            .find_by_id(target.document.id)
            .await?,
        None
    );
    assert_eq!(server.numbers(busy.document.id).await?.len(), 11);
    Ok(())
}
