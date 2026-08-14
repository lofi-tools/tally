-- Temporary (guest) users + company accounting-standard / activity columns
-- (spec: docs/spec/temp-user-spec.md §4.1).
--
-- A temp user is a `users` row with `is_temporary = TRUE`, a placeholder
-- email (`temp+<uuid>@local`) and the dummy argon2 hash (login can never
-- succeed). `guest_id` is the client-generated anonymous identity; it is
-- NULL for real users and cleared on register (in-place upgrade).
--
-- `companies.updated_at` (RFC 3339) is the activity clock for the guest
-- sweep (§8); `accounting_standard` is the FRS 105/102 choice the web add
-- dialog collects (previously web-mock-only).

-- #[toasty::breakpoint]
ALTER TABLE "users" ADD COLUMN "is_temporary" BOOLEAN NOT NULL DEFAULT FALSE;

-- #[toasty::breakpoint]
ALTER TABLE "users" ADD COLUMN "guest_id" TEXT;

-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_guest_id" ON "users" ("guest_id");

-- #[toasty::breakpoint]
ALTER TABLE "companies" ADD COLUMN "accounting_standard" TEXT NOT NULL DEFAULT 'FRS 105';

-- #[toasty::breakpoint]
ALTER TABLE "companies" ADD COLUMN "updated_at" TEXT;
