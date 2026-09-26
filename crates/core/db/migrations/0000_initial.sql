CREATE TABLE "documents" (
    "id" BLOB NOT NULL,
    "project_id" BLOB NOT NULL,
    "slug" TEXT NOT NULL,
    "title" TEXT NOT NULL,
    "current_revision_id" BLOB NOT NULL,
    "archived_at" TEXT,
    "created_at" TEXT NOT NULL,
    "updated_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_documents_by_project_id_and_slug" ON "documents" ("project_id", "slug");
-- #[toasty::breakpoint]
CREATE INDEX "index_documents_by_archived_at" ON "documents" ("archived_at");
-- #[toasty::breakpoint]
CREATE INDEX "index_documents_by_updated_at" ON "documents" ("updated_at");
-- #[toasty::breakpoint]
CREATE TABLE "revisions" (
    "id" BLOB NOT NULL,
    "document_id" BLOB NOT NULL,
    "number" INTEGER NOT NULL,
    "blob_hash" TEXT NOT NULL,
    "message" TEXT NOT NULL,
    "author_name" TEXT NOT NULL,
    "source" TEXT NOT NULL,
    "restored_from_number" INTEGER,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_revisions_by_document_id_and_number" ON "revisions" ("document_id", "number");
-- #[toasty::breakpoint]
CREATE INDEX "index_revisions_by_author_name" ON "revisions" ("author_name");
-- #[toasty::breakpoint]
CREATE TABLE "projects" (
    "id" BLOB NOT NULL,
    "slug" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "color" TEXT NOT NULL,
    "archived_at" TEXT,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_projects_by_slug" ON "projects" ("slug");
-- #[toasty::breakpoint]
CREATE INDEX "index_projects_by_archived_at" ON "projects" ("archived_at");
-- #[toasty::breakpoint]
CREATE TABLE "blobs" (
    "hash" TEXT NOT NULL,
    "size" INTEGER NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("hash")
);
