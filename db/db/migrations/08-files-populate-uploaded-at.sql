UPDATE files
SET uploaded_at = created_at
WHERE uploaded_at IS NULL;
