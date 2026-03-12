#!/bin/sh

# env vars required
# DB_BACKUP_PATH=/path/to/db-backups
# MEMO_DB_PATH=/path/to/db
# S3_BUCKET=your-s3-bucket-name

CURRENT_DATE=$(date +"%Y-%m-%d-%H-%M-%S")
TARGET_DIR="$DB_BACKUP_PATH/memo-rs/$CURRENT_DATE"
BACKUP_FILE="memo-db-$CURRENT_DATE.tar.gz"

echo "Creating backup for memo-rs database at $CURRENT_DATE"

# Create the backup dir
mkdir -p "$TARGET_DIR"

# Backup the database
sqlite3 "$MEMO_DB_PATH/db.sqlite3" ".backup $TARGET_DIR/db.sqlite3"
# Verify backup
sqlite3 "$TARGET_DIR/db.sqlite3" "PRAGMA integrity_check;"

# Compress directory
cd "$DB_BACKUP_PATH/memo-rs"
tar cvf "$BACKUP_FILE" "$CURRENT_DATE"

# Upload to S3
aws s3 cp "$BACKUP_FILE" "s3://$S3_BUCKET/db-backups/memo-rs/$BACKUP_FILE"

# Cleanup
rm -rf $TARGET_DIR
rm $BACKUP_FILE
