//! 画面の操作のテストで使う、メモリ上の SQLite につないだ `WebApp`。

use std::{ops::Deref, sync::Arc};

use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::{
        document::{
            ArchiveDocument, CreateDocument, CreateDocumentInput, FindDocument,
            FindDocumentByFileName, ListDocuments, RestoreDocument, SlugSource,
        },
        project::{
            ArchiveProject, CreateProject, FindProject, ListProjects, RestoreProject, UpdateProject,
        },
        revision::{AddRevision, ListRevisions, RevertToRevision, RevisionInput},
    },
    domain::{
        document::Document, revision::RevisionSource, shared::ProjectId, upload::HtmlSizeLimit,
    },
};

use crate::{AppUrl, ViewerUrl, WebApp};

pub struct TestApp {
    web: WebApp,
}

impl Deref for TestApp {
    type Target = WebApp;

    fn deref(&self) -> &WebApp {
        &self.web
    }
}

impl TestApp {
    /// `<slug>.html` のファイルから資料を登録し、第`revisions`版まで版を追加する。
    /// 第N版の変更メモは「第N版」、HTML は版ごとに違う。
    pub async fn document(
        &self,
        project_id: ProjectId,
        slug: &str,
        title: &str,
        revisions: u32,
    ) -> Document {
        let registered = self
            .create_document
            .execute(CreateDocumentInput {
                project_id,
                title: Some(title.to_owned()),
                slug: SlugSource::FileName(format!("{slug}.html")),
                revision: revision_input(1),
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let mut document = registered.document;
        for number in 2..=revisions {
            document = self
                .add_revision
                .execute(document.id, revision_input(number))
                .await
                .unwrap_or_else(|e| panic!("{e}"))
                .document;
        }
        document
    }

    /// 資料を1つ登録して、すぐアーカイブする。
    pub async fn archived_document(&self, project_id: ProjectId, title: &str) -> Document {
        let document = self.document(project_id, "archived", title, 1).await;
        self.archive(&document).await
    }

    pub async fn archive(&self, document: &Document) -> Document {
        self.archive_document
            .execute(document.id)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }
}

fn revision_input(number: u32) -> RevisionInput {
    RevisionInput {
        html: format!("<!doctype html><title>t</title><p>第{number}版</p>").into_bytes(),
        message: format!("第{number}版"),
        author_name: "佐藤".to_owned(),
        source: RevisionSource::Web,
    }
}

pub async fn new() -> TestApp {
    let database = SqliteDatabase::in_memory()
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let projects = Arc::new(database.projects());
    let documents = Arc::new(database.documents());
    let revisions = Arc::new(database.revisions());
    let blobs = Arc::new(ObjectStoreBlobStore::in_memory());
    TestApp {
        web: WebApp {
            list_projects: ListProjects::new(projects.clone()),
            find_project: FindProject::new(projects.clone()),
            create_project: CreateProject::new(projects.clone()),
            update_project: UpdateProject::new(projects.clone()),
            archive_project: ArchiveProject::new(projects.clone()),
            restore_project: RestoreProject::new(projects.clone()),
            list_documents: ListDocuments::new(documents.clone()),
            find_document: FindDocument::new(documents.clone()),
            archive_document: ArchiveDocument::new(documents.clone()),
            restore_document: RestoreDocument::new(documents.clone()),
            list_revisions: ListRevisions::new(revisions.clone()),
            revert_to_revision: RevertToRevision::new(documents.clone(), revisions.clone()),
            create_document: CreateDocument::new(
                projects,
                documents.clone(),
                blobs.clone(),
                HtmlSizeLimit::DEFAULT,
            ),
            add_revision: AddRevision::new(
                documents.clone(),
                revisions,
                blobs,
                HtmlSizeLimit::DEFAULT,
            ),
            find_document_by_file_name: FindDocumentByFileName::new(documents),
            viewer_url: ViewerUrl::SameHost { port: 8081 },
            app_url: AppUrl::FromRequest,
            html_size_limit: HtmlSizeLimit::DEFAULT,
        },
    }
}
