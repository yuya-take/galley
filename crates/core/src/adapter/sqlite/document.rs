use async_trait::async_trait;
use toasty::stmt::{Type, Value};

use crate::domain::{
    blob::Blob,
    document::{
        Document, DocumentListItem, DocumentOrder, DocumentQuery, DocumentRepository, DocumentSlug,
        DocumentTitle,
    },
    error::RepositoryError,
    project::{CrestColor, ProjectName, ProjectSlug},
    revision::{AuthorName, Revision, RevisionMessage, RevisionNumber},
    shared::{ArchiveFilter, DocumentId, ProjectId},
};

use super::{
    backend_error,
    convert::corrupted,
    model::{BlobRecord, DocumentRecord, RevisionRecord},
    raw::{int, string, timestamp, timestamp_param, uuid, uuid_param},
};

#[derive(Debug, Clone)]
pub struct ToastyDocumentRepository {
    db: toasty::Db,
}

impl ToastyDocumentRepository {
    pub(super) fn new(db: toasty::Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DocumentRepository for ToastyDocumentRepository {
    async fn insert_with_first_revision(
        &self,
        document: &Document,
        revision: &Revision,
        blob: &Blob,
    ) -> Result<(), RepositoryError> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await.map_err(backend_error)?;

        let existing_blob = BlobRecord::filter_by_hash(blob.hash.as_str())
            .first()
            .exec(&mut tx)
            .await
            .map_err(backend_error)?;
        if existing_blob.is_none() {
            BlobRecord::create()
                .hash(blob.hash.as_str())
                .size(blob.size)
                .created_at(blob.created_at)
                .exec(&mut tx)
                .await
                .map_err(backend_error)?;
        }

        DocumentRecord::create()
            .id(document.id.as_uuid())
            .project_id(document.project_id.as_uuid())
            .slug(document.slug.as_str())
            .title(document.title.as_str())
            .current_revision_id(document.current_revision_id.as_uuid())
            .archived_at(document.archived_at)
            .created_at(document.created_at)
            .updated_at(document.updated_at)
            .exec(&mut tx)
            .await
            .map_err(backend_error)?;

        RevisionRecord::create()
            .id(revision.id.as_uuid())
            .document_id(revision.document_id.as_uuid())
            .number(revision.number.get())
            .blob_hash(revision.blob_hash.as_str())
            .message(revision.message.as_str())
            .author_name(revision.author_name.as_str())
            .source(revision.source.as_str())
            .restored_from_number(revision.restored_from.map(RevisionNumber::get))
            .created_at(revision.created_at)
            .exec(&mut tx)
            .await
            .map_err(backend_error)?;

        tx.commit().await.map_err(backend_error)
    }

    async fn update(&self, document: &Document) -> Result<(), RepositoryError> {
        let mut db = self.db.clone();
        DocumentRecord::filter_by_id(document.id.as_uuid())
            .update()
            .title(document.title.as_str())
            .archived_at(document.archived_at)
            .exec(&mut db)
            .await
            .map_err(backend_error)?;
        Ok(())
    }

    async fn find_by_id(&self, id: DocumentId) -> Result<Option<Document>, RepositoryError> {
        let mut db = self.db.clone();
        DocumentRecord::filter_by_id(id.as_uuid())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Document::try_from)
            .transpose()
    }

    async fn find_by_slug(
        &self,
        project_id: ProjectId,
        slug: &DocumentSlug,
    ) -> Result<Option<Document>, RepositoryError> {
        let mut db = self.db.clone();
        DocumentRecord::filter_by_project_id_and_slug(project_id.as_uuid(), slug.as_str())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Document::try_from)
            .transpose()
    }

    async fn slugs_with_prefix(
        &self,
        project_id: ProjectId,
        prefix: &str,
    ) -> Result<Vec<DocumentSlug>, RepositoryError> {
        let mut db = self.db.clone();
        let fields = DocumentRecord::fields();
        let records = DocumentRecord::filter(
            fields
                .project_id()
                .eq(project_id.as_uuid())
                .and(fields.slug().starts_with(prefix)),
        )
        .exec(&mut db)
        .await
        .map_err(backend_error)?;

        records
            .into_iter()
            .map(|record| {
                DocumentSlug::parse(&record.slug).map_err(|e| corrupted("documents.slug", e))
            })
            .collect()
    }

    async fn list(&self, query: &DocumentQuery) -> Result<Vec<DocumentListItem>, RepositoryError> {
        let mut db = self.db.clone();
        let rows = list_sql(query).exec(&mut db).await.map_err(backend_error)?;
        rows.into_iter().map(list_item).collect()
    }
}

/// 一覧で取得する列（`list_item` で読む順）。UUID はバイト列、日時は文字列で受け取る（`raw.rs`）。
const LIST_COLUMNS: &str = "d.id, d.slug, d.title, p.id, p.slug, p.name, p.color, \
                            r.number, r.message, r.author_name, d.archived_at, d.updated_at";

fn list_column_types() -> [Type; 12] {
    [
        Type::Bytes,
        Type::String,
        Type::String,
        Type::Bytes,
        Type::String,
        Type::String,
        Type::String,
        Type::I64,
        Type::String,
        Type::String,
        Type::String,
        Type::String,
    ]
}

/// 一覧の SQL を組み立てる。結合と EXISTS が要るので Toasty のクエリではなく SQL で書く。
fn list_sql(query: &DocumentQuery) -> toasty::sql::Query {
    let Conditions { clauses, params } = Conditions::from_query(query);
    let order = match query.order {
        DocumentOrder::UpdatedDesc => "d.updated_at DESC, d.id DESC",
        DocumentOrder::UpdatedAsc => "d.updated_at ASC, d.id ASC",
        DocumentOrder::TitleAsc => "d.title ASC, d.id ASC",
        DocumentOrder::TitleDesc => "d.title DESC, d.id DESC",
    };
    let limit = query
        .limit
        .map(|limit| format!(" LIMIT {limit}"))
        .unwrap_or_default();

    let sql = format!(
        "SELECT {LIST_COLUMNS} \
         FROM documents d \
         JOIN projects p ON p.id = d.project_id \
         JOIN revisions r ON r.id = d.current_revision_id \
         WHERE {} \
         ORDER BY {order}{limit}",
        clauses.join(" AND ")
    );

    params
        .into_iter()
        .fold(toasty::sql::query(sql), |sql, value| sql.bind(value))
        .column_types(list_column_types())
}

/// WHERE 句の条件と、そのプレースホルダーに渡す値。値は SQL に埋め込まない。
#[derive(Default)]
struct Conditions {
    clauses: Vec<String>,
    params: Vec<Value>,
}

impl Conditions {
    fn from_query(query: &DocumentQuery) -> Self {
        let mut conditions = Self::default();
        match query.archive {
            ArchiveFilter::Active => {
                conditions.push("d.archived_at IS NULL");
                if query.project_id.is_none() {
                    conditions.push("p.archived_at IS NULL");
                }
            }
            ArchiveFilter::Archived => conditions.push("d.archived_at IS NOT NULL"),
        }
        if let Some(project_id) = query.project_id {
            let p = conditions.bind(uuid_param(project_id.as_uuid()));
            conditions.push(&format!("d.project_id = {p}"));
        }
        if let Some(text) = query.title_contains.as_deref().filter(|t| !t.is_empty()) {
            let p = conditions.bind(Value::String(like_pattern(text)));
            conditions.push(&format!("d.title LIKE {p} ESCAPE '\\'"));
        }
        if let Some(since) = query.updated_since {
            let p = conditions.bind(timestamp_param(since));
            conditions.push(&format!("d.updated_at >= {p}"));
        }
        if let Some(author_name) = &query.author_name {
            let p = conditions.bind(Value::String(author_name.as_str().to_owned()));
            conditions.push(&format!(
                "EXISTS (SELECT 1 FROM revisions a WHERE a.document_id = d.id AND a.author_name = {p})"
            ));
        }
        conditions
    }

    fn push(&mut self, clause: &str) {
        self.clauses.push(clause.to_owned());
    }

    /// 値を追加し、そのプレースホルダー（`?1` など）を返す。
    fn bind(&mut self, value: Value) -> String {
        self.params.push(value);
        format!("?{}", self.params.len())
    }
}

/// LIKE の `%` `_` `\` をエスケープして、部分一致のパターンにする。
fn like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

fn list_item(row: Value) -> Result<DocumentListItem, RepositoryError> {
    let Value::Record(row) = row else {
        return Err(corrupted("document list", format!("{row:?}")));
    };
    let mut columns = row.fields.into_iter();
    let mut next = |name: &'static str| {
        columns
            .next()
            .ok_or_else(|| corrupted(name, "列が足りません"))
    };

    let id = uuid(next("documents.id")?, "documents.id")?;
    let slug = string(next("documents.slug")?, "documents.slug")?;
    let title = string(next("documents.title")?, "documents.title")?;
    let project_id = uuid(next("projects.id")?, "projects.id")?;
    let project_slug = string(next("projects.slug")?, "projects.slug")?;
    let project_name = string(next("projects.name")?, "projects.name")?;
    let project_color = string(next("projects.color")?, "projects.color")?;
    let number = int(next("revisions.number")?, "revisions.number")?;
    let message = string(next("revisions.message")?, "revisions.message")?;
    let author_name = string(next("revisions.author_name")?, "revisions.author_name")?;
    let archived_at = match next("documents.archived_at")? {
        Value::Null => None,
        value => Some(timestamp(value, "documents.archived_at")?),
    };
    let updated_at = timestamp(next("documents.updated_at")?, "documents.updated_at")?;

    Ok(DocumentListItem {
        id: DocumentId::from_uuid(id),
        slug: DocumentSlug::parse(&slug).map_err(|e| corrupted("documents.slug", e))?,
        title: DocumentTitle::parse(&title).map_err(|e| corrupted("documents.title", e))?,
        project_id: ProjectId::from_uuid(project_id),
        project_slug: ProjectSlug::parse(&project_slug)
            .map_err(|e| corrupted("projects.slug", e))?,
        project_name: ProjectName::parse(&project_name)
            .map_err(|e| corrupted("projects.name", e))?,
        project_color: CrestColor::parse(&project_color)
            .map_err(|e| corrupted("projects.color", e))?,
        revision_number: u32::try_from(number)
            .map_err(|e| corrupted("revisions.number", e))
            .and_then(|n| RevisionNumber::new(n).map_err(|e| corrupted("revisions.number", e)))?,
        revision_message: RevisionMessage::parse(&message)
            .map_err(|e| corrupted("revisions.message", e))?,
        author_name: AuthorName::parse(&author_name)
            .map_err(|e| corrupted("revisions.author_name", e))?,
        archived_at,
        updated_at,
    })
}
