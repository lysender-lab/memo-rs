#!/bin/sh

# env vars required
# DB_BACKUP_PATH=/path/to/db-backups
# MEMO_DB_PATH=/path/to/db
# BACKUP_S3_BUCKET=your-s3-bucket-name

CURRENT_DATE=$(date +"%Y-%m-%d-%H-%M-%S")
TARGET_DIR="$DB_BACKUP_PATH/memo-rs/$CURRENT_DATE"
BACKUP_FILE="memo-db-$CURRENT_DATE.tar.gz"

echo "Creating backup for memo-rs database at $CURRENT_DATE"

# Create the backup dir
mkdir -p "$TARGET_DIR"

# Backup the database
tursodb --readonly "$MEMO_DB_PATH/memo.db" ".dump" >"$TARGET_DIR/memo.sql"

# Compress directory
cd "$DB_BACKUP_PATH/memo-rs"
tar czf "$BACKUP_FILE" "$CURRENT_DATE"

# Upload to S3
aws s3 cp "$BACKUP_FILE" "s3://$BACKUP_S3_BUCKET/db-backups/memo-rs/$BACKUP_FILE"

# Cleanup
rm -rf $TARGET_DIR
rm $BACKUP_FILE
