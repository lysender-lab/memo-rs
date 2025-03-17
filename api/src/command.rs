use text_io::read;

use crate::Result;
use crate::auth::user::{delete_user, list_users, update_user_password, update_user_status};
use crate::bucket::{NewBucket, create_bucket, delete_bucket};
use crate::bucket::{get_bucket, list_buckets};
use crate::client::{
    delete_client, find_admin_client, get_client, list_clients, set_client_default_bucket,
    unset_client_default_bucket, update_client_status,
};
use crate::config::{BucketCommand, ClientCommand, Config, UserCommand};
use crate::db::create_db_pool;
use crate::storage::create_storage_client;

use crate::auth::user::NewUser;
use crate::auth::user::{create_user, get_user};
use crate::client::{NewClient, create_client};

pub async fn run_setup(config: &Config) -> Result<()> {
    let username: String = read!("Enter username for the admin user: {}\n");

    let Ok(password) = rpassword::prompt_password("Enter password for the admin user: ") else {
        return Err("Failed to read password".into());
    };

    let password = password.trim().to_string();
    let new_user = NewUser {
        username,
        password,
        roles: "SystemAdmin".to_string(),
    };

    let db_pool = create_db_pool(config.db.url.as_str());
    let admin_client = find_admin_client(&db_pool).await?;
    if admin_client.is_some() {
        println!("Admin client already exists.");
        return Ok(());
    }

    let new_client = NewClient {
        name: "System Admin".to_string(),
    };
    let client = create_client(&db_pool, &new_client).await?;
    println!("{{ id = {}, name = {} }}", client.id, client.name);
    println!("Created system admin client.");

    let user = create_user(&db_pool, &client.id, &new_user).await?;
    println!(
        "{{ id = {}, username = {} status = {} }}",
        user.id, user.username, user.status
    );
    println!("Created system admin user.");
    Ok(())
}

pub async fn run_client_command(cmd: ClientCommand, config: &Config) -> Result<()> {
    match cmd {
        ClientCommand::List => run_list_clients(config).await,
        ClientCommand::Create { name } => run_create_client(config, name).await,
        ClientCommand::Enable { id } => run_enable_client(config, id).await,
        ClientCommand::Disable { id } => run_disable_client(config, id).await,
        ClientCommand::Delete { id } => run_delete_client(config, id).await,
        ClientCommand::SetDefaultBucket { id, bucket_id } => {
            run_set_default_bucket(config, id, bucket_id).await
        }
        ClientCommand::UnsetDefaultBucket { id } => run_unset_default_bucket(config, id).await,
    }
}

async fn run_list_clients(config: &Config) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let clients = list_clients(&db_pool).await?;
    for client in clients.iter() {
        println!(
            "{{ id = {}, name = {}, status = {}, default_bucket_id = {} }}",
            client.id,
            client.name,
            client.status,
            client
                .default_bucket_id
                .clone()
                .unwrap_or("None".to_string())
        );
    }
    Ok(())
}

async fn run_create_client(config: &Config, name: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let new_client = NewClient { name };
    let client = create_client(&db_pool, &new_client).await?;
    println!("{{ id = {}, name = {} }}", client.id, client.name);
    println!("Created client.");
    Ok(())
}

async fn run_enable_client(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let client = get_client(&db_pool, &id).await?;
    if let Some(node) = client {
        if &node.status == "active" {
            println!("Client already enabled.");
            return Ok(());
        }

        let _ = update_client_status(&db_pool, &id, "active").await?;
        println!("Client enabled.");
    } else {
        println!("Client not found.");
    }
    Ok(())
}

async fn run_disable_client(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let client = get_client(&db_pool, &id).await?;
    if let Some(node) = client {
        if &node.status == "inactive" {
            println!("Client already disabled.");
            return Ok(());
        }

        let _ = update_client_status(&db_pool, &id, "inactive").await?;
        println!("Client disabled.");
    } else {
        println!("Client not found.");
    }
    Ok(())
}

async fn run_delete_client(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let client = get_client(&db_pool, &id).await?;
    if let Some(_) = client {
        let _ = delete_client(&db_pool, &id).await?;
        println!("Client deleted.");
    } else {
        println!("Client not found.");
    }
    Ok(())
}

async fn run_set_default_bucket(config: &Config, id: String, bucket_id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let client = get_client(&db_pool, &id).await?;
    if let Some(_) = client {
        let _ = set_client_default_bucket(&db_pool, &id, &bucket_id).await?;
        println!("Client default bucket set.");
    } else {
        println!("Client not found.");
    }
    Ok(())
}

async fn run_unset_default_bucket(config: &Config, id: String) -> Result<()> {
    let db_pool = create_db_pool(config.db.url.as_str());
    let client = get_client(&db_pool, &id).await?;
    if let Some(node) = client {
        if node.default_bucket_id.is_none() {
            println!("Client do not have a default bucket.");
            return Ok(());
        }

        let _ = unset_client_default_bucket(&db_pool, &id).await?;
        println!("Client default bucket unset.");
    } else {
        println!("Client not found.");
    }
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
    let Ok(password) = rpassword::prompt_password("Enter password for the new user: ") else {
        return Err("Failed to read password".into());
    };

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
        if password.len() < 8 {
            return Err("Password must be at least 8 characters".into());
        }
        if password.len() > 100 {
            return Err("Password must be at most 100 characters".into());
        }
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
