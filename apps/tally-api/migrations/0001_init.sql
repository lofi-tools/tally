-- Initial tally-api schema (matches the toasty models in src/models.rs).
--
-- Generated from toasty's own schema diff (empty schema -> current models)
-- with the postgres driver, so the DDL is byte-for-byte what `push_schema`
-- would have produced — but applied exactly once, in order, and tracked in
-- `_migrations_history` (see src/migrations.rs).

-- #[toasty::breakpoint]
CREATE TABLE "users" (
    "id" UUID NOT NULL,
    "email" TEXT NOT NULL,
    "password_hash" TEXT NOT NULL,
    "display_name" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_email" ON "users" ("email");

-- #[toasty::breakpoint]
CREATE TABLE "sessions" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "token_hash" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    "expires_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_sessions_by_user_id" ON "sessions" ("user_id");

-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_sessions_by_token_hash" ON "sessions" ("token_hash");

-- #[toasty::breakpoint]
CREATE TABLE "companies" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "tax_reference" TEXT NOT NULL,
    "company_number" TEXT NOT NULL,
    "registration_date" TEXT,
    "directors" JSON NOT NULL,
    "contact_name" TEXT,
    "address_lines" JSON NOT NULL,
    "county" TEXT,
    "location" TEXT,
    "postcode" TEXT,
    "email" TEXT,
    "phone_country" TEXT,
    "phone_area" TEXT,
    "phone_number" TEXT,
    "website_url" TEXT,
    "website_description" TEXT,
    "vat_registration" TEXT,
    "sic_codes" JSON NOT NULL,
    "activities" TEXT,
    "jurisdiction" TEXT,
    "accountant_name" TEXT,
    "accountant_business" TEXT,
    "accountant_address" TEXT,
    "auditor_name" TEXT,
    "auditor_business" TEXT,
    "auditor_address" TEXT,
    "industry_sector_dimension" TEXT,
    "legal_form_dimension" TEXT,
    "country_dimension" TEXT,
    "contact_country_dimension" TEXT,
    "phone_type_dimension" TEXT,
    "logo_b64" TEXT,
    "fy1_year" INTEGER NOT NULL,
    "fy2_year" INTEGER NOT NULL,
    "associated_companies" INTEGER,
    "report_date" TEXT,
    "authorised_date" TEXT,
    "incorporation_date" TEXT,
    "signed_by" TEXT,
    "average_employees" JSON,
    "signature_b64" TEXT,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_companies_by_user_id" ON "companies" ("user_id");

-- #[toasty::breakpoint]
CREATE TABLE "ledgers" (
    "id" UUID NOT NULL,
    "company_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "file_path" TEXT NOT NULL,
    "file_sha256" TEXT NOT NULL,
    "uploaded_at" TEXT NOT NULL,
    "accounts_count" BIGINT NOT NULL,
    "transactions_count" BIGINT NOT NULL,
    "splits_count" BIGINT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_ledgers_by_company_id" ON "ledgers" ("company_id");

-- #[toasty::breakpoint]
CREATE TABLE "accounts" (
    "id" UUID NOT NULL,
    "ledger_id" UUID NOT NULL,
    "guid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "account_type" TEXT NOT NULL,
    "parent_guid" TEXT NOT NULL,
    "balance" NUMERIC NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_accounts_by_ledger_id" ON "accounts" ("ledger_id");

-- #[toasty::breakpoint]
CREATE TABLE "transactions" (
    "id" UUID NOT NULL,
    "ledger_id" UUID NOT NULL,
    "guid" TEXT NOT NULL,
    "post_datetime" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_transactions_by_ledger_id" ON "transactions" ("ledger_id");

-- #[toasty::breakpoint]
CREATE TABLE "splits" (
    "id" UUID NOT NULL,
    "ledger_id" UUID NOT NULL,
    "tx_guid" TEXT NOT NULL,
    "account_guid" TEXT NOT NULL,
    "value" NUMERIC NOT NULL,
    PRIMARY KEY ("id")
);

-- #[toasty::breakpoint]
CREATE INDEX "index_splits_by_ledger_id" ON "splits" ("ledger_id");

-- #[toasty::breakpoint]
CREATE INDEX "index_splits_by_tx_guid" ON "splits" ("tx_guid");

-- #[toasty::breakpoint]
CREATE INDEX "index_splits_by_account_guid" ON "splits" ("account_guid");
