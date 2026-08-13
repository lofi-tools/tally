-- Durable background jobs + Companies House filings / balance-sheet history
-- (spec: docs/spec/ch-filings-sync-spec.md).
--
-- Follows the 0001 conventions: TEXT columns (RFC 3339 / ISO-8601 strings
-- written by the app), JSON columns for toasty::Json fields, no FK
-- constraints (ownership + cleanup are enforced in the handlers, like the
-- rest of the schema).

-- #[toasty::breakpoint]
CREATE TABLE "jobs" (
    "id" UUID NOT NULL,
    "kind" TEXT NOT NULL,
    "company_id" UUID NOT NULL,
    "status" TEXT NOT NULL,
    "attempts" INTEGER NOT NULL,
    "last_error" TEXT,
    "next_retry_at" TEXT,
    "created_at" TEXT NOT NULL,
    "updated_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_jobs_by_company_id" ON "jobs" ("company_id");

-- #[toasty::breakpoint]
CREATE INDEX "index_jobs_by_status" ON "jobs" ("status");

-- Dedupe concurrent enqueues: at most one pending/running job per
-- (kind, company). Done/failed jobs are freely re-enqueued (Refresh).
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_jobs_pending_unique" ON "jobs" ("kind", "company_id") WHERE "status" IN ('pending', 'running');

-- #[toasty::breakpoint]
CREATE TABLE "filings" (
    "id" UUID NOT NULL,
    "company_id" UUID NOT NULL,
    "ch_transaction_id" TEXT NOT NULL,
    "category" TEXT NOT NULL,
    "form_type" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "filed_on" TEXT,
    "document_metadata_url" TEXT NOT NULL,
    "raw" JSON NOT NULL,
    "fetched_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_filings_by_company_id" ON "filings" ("company_id");

-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_filings_by_company_and_transaction" ON "filings" ("company_id", "ch_transaction_id");

-- #[toasty::breakpoint]
CREATE TABLE "balance_sheets" (
    "id" UUID NOT NULL,
    "company_id" UUID NOT NULL,
    "period_end" TEXT NOT NULL,
    "filed_on" TEXT,
    "source_filing_id" UUID,
    "figures" JSON NOT NULL,
    "raw_document" BYTEA,
    "parsed_document" TEXT,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_balance_sheets_by_company_id" ON "balance_sheets" ("company_id");

-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_balance_sheets_by_company_and_period_end" ON "balance_sheets" ("company_id", "period_end");
