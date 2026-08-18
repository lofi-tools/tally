-- Model `Filing.form_type` becomes a toasty embedded enum (`ChFormType`):
-- the `form_type` column becomes the Postgres `ch_form_type` enum type
-- (variant discriminant), and a nullable `form_type_code` payload column
-- holds the raw code for filings whose code has no modelled variant (the
-- `Other` variant), so unknown codes are not lost.

-- The enum type name and labels are exactly what toasty's `push_schema`
-- generates for the model (`ch_form_type` = snake_case of `ChFormType`;
-- labels are the snake_case variant names).
CREATE TYPE "ch_form_type" AS ENUM (
    'accounts',
    'change_accounting_reference_date',
    'confirmation_statement',
    'change_registered_office',
    'incorporation',
    'other'
);

-- Payload column for the `Other` variant (nullable — only rows whose
-- discriminant is `other` carry a value).
ALTER TABLE "filings" ADD COLUMN "form_type_code" TEXT;

-- Preserve the raw code of any filing without a modelled variant before
-- the column is recast (the code would otherwise be lost).
UPDATE "filings"
   SET "form_type_code" = "form_type"
 WHERE "form_type" NOT IN ('AA', 'AA01', 'CS01', 'AD01', 'NEWINC');

ALTER TABLE "filings"
    ALTER COLUMN "form_type" TYPE "ch_form_type"
    USING CASE "form_type"
        WHEN 'AA' THEN 'accounts'::"ch_form_type"
        WHEN 'AA01' THEN 'change_accounting_reference_date'::"ch_form_type"
        WHEN 'CS01' THEN 'confirmation_statement'::"ch_form_type"
        WHEN 'AD01' THEN 'change_registered_office'::"ch_form_type"
        WHEN 'NEWINC' THEN 'incorporation'::"ch_form_type"
        ELSE 'other'::"ch_form_type"
    END;
