use std::sync::Arc;

use snafu::{ResultExt, ensure};
use text_io::read;

use crate::Result;
use crate::auth::user::{delete_user, list_users, update_user_password, update_user_status};
use crate::bucket::{NewBucket, create_bucket};
use crate::client::NewClient;
use crate::config::{BucketCommand, Config, UserCommand};
use crate::db::{create_db_mapper, create_db_pool};
use crate::error::{PasswordPromptSnafu, ValidationSnafu};
use crate::storage::StorageClient;
use crate::web::server::AppState;

use crate::auth::user::NewUser;
use crate::auth::user::{create_user, get_user};
use crate::client::create_client;

pub async fn run_setup(config: &Config) -> Result<()> {
    print!("Enter username for the admin user: ");
    let username: String = read!("{}\n");

    let password = rpassword::prompt_password("Enter password for the admin user: ").context(
        PasswordPromptSnafu {
            msg: "Failed to read password",
        },
    )?;

    let password = password.trim().to_string();
    let new_user = NewUser {
        username: username.trim().to_string(),
        password,
        roles: "SystemAdmin".to_string(),
    };

    let storage_client = StorageClient::new(config.cloud.credentials.as_str()).await?;
    let pool = create_db_pool(config.db.url.as_str());
    let db = create_db_mapper(config.db.url.as_str());

    let state = AppState {
        config: config.clone(),
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
        db_pool: pool,
    };

    let client_id: String;
    let admin_client = state.db.clients.find_admin().await?;
    if let Some(client) = admin_client {
        client_id = client.id;
    } else {
        let new_client = NewClient {
            name: "system-admin".to_string(),
            status: "active".to_string(),
            default_bucket_id: None,
        };
        let client = create_client(state, &new_client, true).await?;
        println!("{{ id = {}, name = {} }}", client.id, client.name);
        println!("Created system admin client.");
        client_id = client.id;
    }

    let users = list_users(&db_pool, &client_id).await?;
    if users.len() > 0 {
        println!("Admin user already exists.");
        return Ok(());
    }

    let user = create_user(&db_pool, &client_id, &new_user).await?;
    println!(
        "{{ id = {}, username = {} status = {} }}",
        user.id, user.username, user.status
    );
    println!("Created system admin user.");
    Ok(())
}

pub async fn run_user_command(cmd: UserCommand, config: &Config) -> Result<()> {
    match cmd {
        UserCommand::List { client_id } => run_list_users(config, client_id).await,
        UserCommand::Create {
            client_id,
            username,
            roles,
        } => run_create_user(config, client_id, username, roles).await,
        UserCommand::Password { id } => run_set_user_password(config, id).await,
        UserCommand::Disable { id } => run_disable_user(config, id).await,
        UserCommand::Enable { id } => run_enable_user(config, id).await,
        UserCommand::Delete { id } => run_delete_user(config, id).await,
    }
}

async fn run_list_users(config: &Config, client_id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let users = list_users(&db_pool, &client_id).await?;
    for user in users.iter() {
        println!(
            "{{ id = {}, username = {}, roles = {}, status = {} }}",
            user.id, user.username, user.roles, user.status
        );
    }
    Ok(())
}

async fn run_create_user(
    config: &Config,
    client_id: String,
    username: String,
    roles: String,
) -> Result<()> {
    let password = rpassword::prompt_password("Enter password for the new user: ").context(
        PasswordPromptSnafu {
            msg: "Failed to read password",
        },
    )?;

    let password = password.trim().to_string();
    let new_user = NewUser {
        username,
        password,
        roles,
    };

    let db_pool = create_db_pool(config.db.url.as_str());
    let user = create_user(&db_pool, &client_id, &new_user).await?;
    println!(
        "{{ id = {}, username = {} status = {} }}",
        user.id, user.username, user.status
    );
    println!("Created user.");
    Ok(())
}

async fn run_set_user_password(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let user = get_user(&db_pool, &id).await?;
    if let Some(node) = user {
        let prompt = format!("Enter new password for {}: ", node.username);
        let Ok(password) = rpassword::prompt_password(prompt) else {
            return Err("Failed to read password".into());
        };
        let password = password.trim().to_string();
        let pwdlen = password.len();
        ensure!(
            pwdlen >= 8 && pwdlen <= 100,
            ValidationSnafu {
                msg: "Password must be between 8 to 60 characters".to_string()
            }
        );
        let _ = update_user_password(&db_pool, &id, &password).await?;
        println!("Password updated.");
    } else {
        println!("User not found.");
    }
    Ok(())
}

async fn run_disable_user(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let user = get_user(&db_pool, &id).await?;
    if let Some(node) = user {
        if &node.status == "inactive" {
            println!("User already disabled.");
            return Ok(());
        }
        let _ = update_user_status(&db_pool, &id, "inactive").await?;
        println!("User disabled.");
    } else {
        println!("User not found.");
    }
    Ok(())
}

async fn run_enable_user(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let user = get_user(&db_pool, &id).await?;
    if let Some(node) = user {
        if &node.status == "inactive" {
            println!("User already disabled.");
            return Ok(());
        }
        let _ = update_user_status(&db_pool, &id, "inactive").await?;
        println!("User disabled.");
    } else {
        println!("User not found.");
    }
    Ok(())
}

async fn run_delete_user(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let user = get_user(&db_pool, &id).await?;
    if let Some(_) = user {
        let _ = delete_user(&db_pool, &id).await?;
        println!("User deleted.");
    } else {
        println!("User not found.");
    }
    Ok(())
}

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
    let db = create_db_mapper(config.db.url.as_str());
    let buckets = db.buckets.list(client_id.as_str()).await?;
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
    let storage_client = StorageClient::new(config.cloud.credentials.as_str()).await?;
    let pool = create_db_pool(config.db.url.as_str());
    let db = create_db_mapper(config.db.url.as_str());

    let state = AppState {
        config: config.clone(),
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
        db_pool: pool,
    };

    let res: Result<bool> = match images_only.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            return ValidationSnafu {
                msg: "Invalid boolean".to_string(),
            }
            .fail();
        }
    };

    let Ok(img_only) = res else {
        return Err("images_only must be either true or false".into());
    };

    let data = NewBucket {
        name,
        images_only: img_only,
    };
    let bucket = create_bucket(state, &client_id, &data).await?;

    println!(
        "{{ id = {}, name = {}, images_only = {} }}",
        bucket.id, bucket.name, bucket.images_only
    );
    println!("Created bucket.");
    Ok(())
}

async fn run_delete_bucket(config: &Config, id: String) -> Result<()> {
    let db = create_db_mapper(config.db.url.as_str());
    let bucket = db.buckets.get(&id).await?;
    if let Some(_) = bucket {
        let _ = db.buckets.delete(&id).await?;
        println!("Bucket deleted.");
    } else {
        println!("Bucket not found.");
    }
    Ok(())
}
