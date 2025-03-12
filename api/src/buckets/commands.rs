use crate::Result;
use crate::buckets::{NewBucket, create_bucket, delete_bucket};
use crate::config::{BucketCommand, Config};
use crate::db::create_db_pool;
use crate::storage::create_storage_client;

use super::{get_bucket, list_buckets};

pub async fn run_bucket_command(cmd: BucketCommand, config: &Config) -> Result<()> {
    match cmd {
        BucketCommand::List { client_id } => run_list_buckets(config, client_id).await,
        BucketCommand::Create {
            client_id,
            name,
            images_only,
        } => run_create_bucket(config, client_id, name, images_only).await,
        BucketCommand::Delete { id } => run_delete_bucket(config, id).await,
    }
}

async fn run_list_buckets(config: &Config, client_id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let buckets = list_buckets(&db_pool, &client_id).await?;
    for bucket in buckets.iter() {
        println!(
            "{{ id = {}, name = {}, images_only = {} }}",
            bucket.id, bucket.name, bucket.images_only
        );
    }
    Ok(())
}

async fn run_create_bucket(
    config: &Config,
    client_id: String,
    name: String,
    images_only: String,
) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let storage_client = create_storage_client(config.cloud.credentials.as_str()).await?;

    let res: Result<bool> = match images_only.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Invalid boolean".into()),
    };

    let Ok(img_only) = res else {
        return Err("images_only must be either true or false".into());
    };

    let data = NewBucket {
        name,
        images_only: img_only,
    };
    let bucket = create_bucket(&db_pool, &storage_client, &client_id, &data).await?;
    println!(
        "{{ id = {}, name = {}, images_only = {} }}",
        bucket.id, bucket.name, bucket.images_only
    );
    println!("Created bucket.");
    Ok(())
}

async fn run_delete_bucket(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let bucket = get_bucket(&db_pool, &id).await?;
    if let Some(_) = bucket {
        let _ = delete_bucket(&db_pool, &id).await?;
        println!("Bucket deleted.");
    } else {
        println!("Bucket not found.");
    }
    Ok(())
}
