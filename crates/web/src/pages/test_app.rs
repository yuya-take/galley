//! 画面の操作のテストで使う、メモリ上の SQLite につないだ `WebApp`。

use std::{ops::Deref, sync::Arc};

use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::{
        document::{
            ArchiveDocument, CreateDocument, CreateDocumentInput, FindDocument, ListDocuments,
            RestoreDocument, SlugSource,
        },
        project::{
            ArchiveProject, CreateProject, FindProject, ListProjects, RestoreProject, UpdateProject,
        },
        revision::{ListRevisions, RevisionInput},
    },
    domain::{document::Document, revision::RevisionSource, shared::ProjectId},
};

use crate::{AppUrl, ViewerUrl, WebApp};

pub struct TestApp {
    web: WebApp,
    create_document: CreateDocument,
    archive_document: ArchiveDocument,
}

impl Deref for TestApp {
    type Target = WebApp;

    fn deref(&self) -> &WebApp {
        &self.web
    }
}

impl TestApp {
    /// 資料を1つ登録する。
    pub async fn document(&self, project_id: ProjectId, title: &str) -> Document {
        self.create_document
            .execute(CreateDocumentInput {
                project_id,
                title: Some(title.to_owned()),
                slug: SlugSource::None,
                revision: RevisionInput {
                    html: b"<!doctype html><title>t</title><p>hi</p>".to_vec(),
                    message: "初版".to_owned(),
                    author_name: "佐藤".to_owned(),
                    source: RevisionSource::Web,
                },
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"))
            .document
    }

    /// 資料を1つ登録して、すぐアーカイブする。
    pub async fn archived_document(&self, project_id: ProjectId, title: &str) -> Document {
        let document = self.document(project_id, title).await;
        self.archive_document
            .execute(document.id)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }
}

pub async fn new() -> TestApp {
    let database = SqliteDatabase::in_memory()
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let projects = Arc::new(database.projects());
    let documents = Arc::new(database.documents());
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
            restore_document: RestoreDocument::new(documents.clone()),
            list_revisions: ListRevisions::new(Arc::new(database.revisions())),
            viewer_url: ViewerUrl::SameHost { port: 8081 },
            app_url: AppUrl::FromRequest,
        },
        create_document: CreateDocument::new(
            projects,
            documents.clone(),
            Arc::new(ObjectStoreBlobStore::in_memory()),
        ),
        archive_document: ArchiveDocument::new(documents),
    }
}
