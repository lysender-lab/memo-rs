BEGIN TRANSACTION;

UPDATE files
SET status = 'active'
WHERE status IS NULL;

COMMIT;
