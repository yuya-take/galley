//! SQLite（Toasty）の実装とユースケースを、マイグレーションを適用した DB で確かめる。

use std::{error::Error, sync::Arc};

use galley_core::{
    adapter::sqlite::SqliteDatabase,
    app::{
        document::{
            ArchiveDocument, ChooseDocumentSlug, FindDocumentByFileName, ListDocuments,
            RenameDocument, RestoreDocument, SlugSource,
        },
        error::AppError,
        project::{
            ArchiveProject, CreateProject, FindProject, ListProjects, ProjectInput, RestoreProject,
            UpdateProject,
        },
    },
    domain::{
        blob::{Blob, BlobHash},
        document::{
            Document, DocumentOrder, DocumentQuery, DocumentRepository, DocumentSlug, DocumentTitle,
        },
        error::RepositoryError,
        project::Project,
        revision::{AuthorName, NewRevision, RevisionMessage, RevisionNumber, RevisionSource},
        shared::{ArchiveFilter, ProjectId},
    },
};
use jiff::{Timestamp, ToSpan};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    database: SqliteDatabase,
    documents: Arc<dyn DocumentRepository>,
}

impl Fixture {
    async fn new() -> Result<Self, Box<dyn Error>> {
        let database = SqliteDatabase::in_memory().await?;
        let documents = Arc::new(database.documents());
        Ok(Self {
            database,
            documents,
        })
    }

    async fn create_project(&self, slug: &str, name: &str) -> Result<Project, AppError> {
        CreateProject::new(Arc::new(self.database.projects()))
            .execute(&project_input(slug, name))
            .await
    }

    /// 資料と第1版を保存する。`updated_at` を変えて並び順を確かめられるようにする。
    async fn create_document(
        &self,
        project: &Project,
        slug: &str,
        title: &str,
        author: &str,
        updated_at: Timestamp,
    ) -> Result<Document, Box<dyn Error>> {
        let hash = BlobHash::parse(&"a".repeat(64))?;
        let (document, revision) = Document::create(
            project.id,
            DocumentSlug::parse(slug)?,
            DocumentTitle::parse(title)?,
            NewRevision {
                blob_hash: hash.clone(),
                message: RevisionMessage::parse("初版")?,
                author_name: AuthorName::parse(author)?,
                source: RevisionSource::Web,
            },
            updated_at,
        );
        let blob = Blob {
            hash,
            size: 1024,
            created_at: updated_at,
        };
        self.documents
            .insert_with_first_revision(&document, &revision, &blob)
            .await?;
        Ok(document)
    }

    async fn titles(&self, query: DocumentQuery) -> Result<Vec<String>, Box<dyn Error>> {
        let items = ListDocuments::new(self.documents.clone())
            .execute(&query)
            .await?;
        Ok(items.into_iter().map(|i| i.title.to_string()).collect())
    }
}

fn project_input(slug: &str, name: &str) -> ProjectInput {
    ProjectInput {
        slug: slug.to_owned(),
        name: name.to_owned(),
        description: String::new(),
        color: "blue".to_owned(),
    }
}

fn at(second: i64) -> Result<Timestamp, jiff::Error> {
    Timestamp::from_second(1_790_000_000 + second)
}

#[tokio::test]
async fn create_and_find_project() -> TestResult {
    let fixture = Fixture::new().await?;
    let created = fixture.create_project("sales", "営業部").await?;

    let found = FindProject::new(Arc::new(fixture.database.projects()))
        .execute("sales")
        .await?;
    assert_eq!(found, created);
    Ok(())
}

#[tokio::test]
async fn project_slug_must_be_unique() -> TestResult {
    let fixture = Fixture::new().await?;
    fixture.create_project("sales", "営業部").await?;

    let result = fixture.create_project("sales", "営業部 2").await;
    assert!(matches!(result, Err(AppError::ProjectSlugTaken(slug)) if slug == "sales"));
    Ok(())
}

#[tokio::test]
async fn update_project_rejects_slug_of_another_project() -> TestResult {
    let fixture = Fixture::new().await?;
    fixture.create_project("sales", "営業部").await?;
    let dev = fixture.create_project("dev", "開発部").await?;
    let update = UpdateProject::new(Arc::new(fixture.database.projects()));

    let result = update
        .execute(dev.id, &project_input("sales", "開発部"))
        .await;
    assert!(matches!(result, Err(AppError::ProjectSlugTaken(_))));

    // 自分の slug のままなら変更できる
    let updated = update
        .execute(dev.id, &project_input("dev", "開発本部"))
        .await?;
    assert_eq!(updated.name.as_str(), "開発本部");
    Ok(())
}

#[tokio::test]
async fn project_input_is_validated() -> TestResult {
    let fixture = Fixture::new().await?;
    assert!(matches!(
        fixture.create_project("archive", "アーカイブ").await,
        Err(AppError::InvalidSlug(_))
    ));
    assert!(matches!(
        fixture.create_project("sales", " ").await,
        Err(AppError::InvalidText(_))
    ));
    Ok(())
}

#[tokio::test]
async fn list_projects_counts_active_documents() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let dev = fixture.create_project("dev", "開発部").await?;
    fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;
    let archived = fixture
        .create_document(&sales, "old", "古い資料", "佐藤", at(1)?)
        .await?;
    ArchiveDocument::new(fixture.documents.clone())
        .execute(archived.id)
        .await?;

    let projects = Arc::new(fixture.database.projects());
    let summaries = ListProjects::new(projects.clone())
        .execute(ArchiveFilter::Active)
        .await?;
    let counts: Vec<_> = summaries
        .iter()
        .map(|s| (s.project.slug.as_str(), s.document_count))
        .collect();
    // 作成順
    assert_eq!(counts, [("sales", 1), ("dev", 0)]);

    ArchiveProject::new(projects.clone())
        .execute(dev.id)
        .await?;
    let active = ListProjects::new(projects.clone())
        .execute(ArchiveFilter::Active)
        .await?;
    let archived = ListProjects::new(projects.clone())
        .execute(ArchiveFilter::Archived)
        .await?;
    assert_eq!(active.len(), 1);
    assert_eq!(archived.len(), 1);
    assert!(archived[0].project.is_archived());

    RestoreProject::new(projects.clone())
        .execute(dev.id)
        .await?;
    let active = ListProjects::new(projects)
        .execute(ArchiveFilter::Active)
        .await?;
    assert_eq!(active.len(), 2);
    Ok(())
}

#[tokio::test]
async fn document_slug_is_unique_within_project() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let dev = fixture.create_project("dev", "開発部").await?;
    fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;
    // 別のプロジェクトなら同じ slug を使える
    fixture
        .create_document(&dev, "plan", "開発計画", "佐藤", at(0)?)
        .await?;

    let result = fixture
        .create_document(&sales, "plan", "事業計画 2", "佐藤", at(1)?)
        .await;
    let is_conflict = result.err().is_some_and(|err| {
        matches!(
            err.downcast_ref::<RepositoryError>(),
            Some(RepositoryError::Conflict(_))
        )
    });
    assert!(is_conflict);
    Ok(())
}

#[tokio::test]
async fn list_documents_orders_by_updated_at() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    // 小数秒の有無で文字列の並びが崩れないことも確かめる
    fixture
        .create_document(&sales, "a", "A", "佐藤", at(10)?)
        .await?;
    fixture
        .create_document(
            &sales,
            "b",
            "B",
            "佐藤",
            at(10)?.checked_add(500.milliseconds())?,
        )
        .await?;
    fixture
        .create_document(&sales, "c", "C", "佐藤", at(5)?)
        .await?;

    let newest_first = fixture.titles(DocumentQuery::default()).await?;
    assert_eq!(newest_first, ["B", "A", "C"]);

    let by_title = fixture
        .titles(DocumentQuery {
            order: DocumentOrder::TitleDesc,
            limit: Some(2),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(by_title, ["C", "B"]);
    Ok(())
}

#[tokio::test]
async fn list_documents_returns_current_revision_and_project() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let document = fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;

    let items = ListDocuments::new(fixture.documents.clone())
        .execute(&DocumentQuery::default())
        .await?;
    let [item] = items.as_slice() else {
        panic!("1件のはず: {items:?}");
    };
    assert_eq!(item.id, document.id);
    assert_eq!(item.slug.as_str(), "plan");
    assert_eq!(item.project_slug.as_str(), "sales");
    assert_eq!(item.project_name.as_str(), "営業部");
    assert_eq!(item.project_color.as_str(), "blue");
    assert_eq!(item.revision_number, RevisionNumber::FIRST);
    assert_eq!(item.revision_message.as_str(), "初版");
    assert_eq!(item.author_name.as_str(), "佐藤");
    assert_eq!(item.updated_at, at(0)?);
    assert_eq!(item.archived_at, None);
    Ok(())
}

#[tokio::test]
async fn list_documents_filters() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let dev = fixture.create_project("dev", "開発部").await?;
    fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;
    fixture
        .create_document(&sales, "rate", "達成率100%の振り返り", "田中", at(10)?)
        .await?;
    fixture
        .create_document(&dev, "arch", "設計_v2", "佐藤", at(20)?)
        .await?;

    let project = fixture
        .titles(DocumentQuery {
            project_id: Some(dev.id),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(project, ["設計_v2"]);

    let title = fixture
        .titles(DocumentQuery {
            title_contains: Some("計画".to_owned()),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(title, ["事業計画"]);

    // LIKE の特殊文字は文字として扱う
    let percent = fixture
        .titles(DocumentQuery {
            title_contains: Some("0%".to_owned()),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(percent, ["達成率100%の振り返り"]);
    let underscore = fixture
        .titles(DocumentQuery {
            title_contains: Some("_".to_owned()),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(underscore, ["設計_v2"]);

    let recent = fixture
        .titles(DocumentQuery {
            updated_since: Some(at(10)?),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(recent, ["設計_v2", "達成率100%の振り返り"]);

    let author = fixture
        .titles(DocumentQuery {
            author_name: Some(AuthorName::parse("佐藤")?),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(author, ["設計_v2", "事業計画"]);
    Ok(())
}

#[tokio::test]
async fn archived_documents_and_projects_are_hidden() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let dev = fixture.create_project("dev", "開発部").await?;
    let plan = fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;
    fixture
        .create_document(&sales, "memo", "メモ", "佐藤", at(1)?)
        .await?;
    fixture
        .create_document(&dev, "arch", "設計", "佐藤", at(2)?)
        .await?;

    ArchiveDocument::new(fixture.documents.clone())
        .execute(plan.id)
        .await?;
    ArchiveProject::new(Arc::new(fixture.database.projects()))
        .execute(dev.id)
        .await?;

    assert_eq!(fixture.titles(DocumentQuery::default()).await?, ["メモ"]);
    let archived = fixture
        .titles(DocumentQuery {
            archive: ArchiveFilter::Archived,
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(archived, ["事業計画"]);
    // プロジェクトを指定すれば、アーカイブしたプロジェクトの資料も見られる
    let in_archived_project = fixture
        .titles(DocumentQuery {
            project_id: Some(dev.id),
            ..DocumentQuery::default()
        })
        .await?;
    assert_eq!(in_archived_project, ["設計"]);

    RestoreDocument::new(fixture.documents.clone())
        .execute(plan.id)
        .await?;
    assert_eq!(
        fixture.titles(DocumentQuery::default()).await?,
        ["メモ", "事業計画"]
    );
    Ok(())
}

#[tokio::test]
async fn rename_document_keeps_updated_at() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let plan = fixture
        .create_document(&sales, "plan", "事業計画", "佐藤", at(0)?)
        .await?;

    RenameDocument::new(fixture.documents.clone())
        .execute(plan.id, "事業計画 2026")
        .await?;
    let found = fixture
        .documents
        .find_by_id(plan.id)
        .await?
        .ok_or("資料がない")?;
    assert_eq!(found.title.as_str(), "事業計画 2026");
    assert_eq!(found.updated_at, at(0)?);
    Ok(())
}

#[tokio::test]
async fn choose_document_slug_avoids_collisions() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let choose = ChooseDocumentSlug::new(fixture.documents.clone());
    let from_file = SlugSource::FileName("Jigyo Keikaku.html".to_owned());

    let slug = choose.execute(sales.id, &from_file).await?;
    assert_eq!(slug.as_str(), "jigyo-keikaku");

    fixture
        .create_document(&sales, "jigyo-keikaku", "事業計画", "佐藤", at(0)?)
        .await?;
    // 前方一致するが別の slug は衝突にしない
    fixture
        .create_document(&sales, "jigyo-keikaku-old", "旧版", "佐藤", at(0)?)
        .await?;
    let slug = choose.execute(sales.id, &from_file).await?;
    assert_eq!(slug.as_str(), "jigyo-keikaku-2");

    fixture
        .create_document(&sales, "jigyo-keikaku-2", "事業計画 2", "佐藤", at(0)?)
        .await?;
    let slug = choose.execute(sales.id, &from_file).await?;
    assert_eq!(slug.as_str(), "jigyo-keikaku-3");

    // 別のプロジェクトの slug は関係ない
    let other = choose.execute(ProjectId::generate(), &from_file).await?;
    assert_eq!(other.as_str(), "jigyo-keikaku");

    let generated = choose
        .execute(sales.id, &SlugSource::FileName("事業計画.html".to_owned()))
        .await?;
    assert!(generated.as_str().starts_with("doc-"));
    let pasted = choose.execute(sales.id, &SlugSource::None).await?;
    assert!(pasted.as_str().starts_with("doc-"));

    let requested = choose
        .execute(sales.id, &SlugSource::Requested("Bad Slug".to_owned()))
        .await;
    assert!(matches!(requested, Err(AppError::InvalidSlug(_))));
    Ok(())
}

#[tokio::test]
async fn find_document_by_file_name_skips_archived() -> TestResult {
    let fixture = Fixture::new().await?;
    let sales = fixture.create_project("sales", "営業部").await?;
    let plan = fixture
        .create_document(&sales, "jigyo-keikaku", "事業計画", "佐藤", at(0)?)
        .await?;
    let find = FindDocumentByFileName::new(fixture.documents.clone());

    let found = find.execute(sales.id, "jigyo-keikaku.html").await?;
    assert_eq!(found.map(|d| d.id), Some(plan.id));
    assert_eq!(find.execute(sales.id, "other.html").await?, None);
    assert_eq!(find.execute(sales.id, "事業計画.html").await?, None);

    ArchiveDocument::new(fixture.documents.clone())
        .execute(plan.id)
        .await?;
    assert_eq!(find.execute(sales.id, "jigyo-keikaku.html").await?, None);
    Ok(())
}

#[tokio::test]
async fn file_database_uses_wal_and_reapplies_migrations_safely() -> TestResult {
    let dir = std::env::temp_dir().join(format!("galley-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("galley.db");

    let database = SqliteDatabase::open(&path).await?;
    let sales = CreateProject::new(Arc::new(database.projects()))
        .execute(&project_input("sales", "営業部"))
        .await?;
    drop(database);

    // 2回目に開いてもマイグレーションは重ねて適用されず、データも残る
    let database = SqliteDatabase::open(&path).await?;
    let found = FindProject::new(Arc::new(database.projects()))
        .execute("sales")
        .await?;
    assert_eq!(found.id, sales.id);
    assert_eq!(database.journal_mode().await?, "wal");

    drop(database);
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
