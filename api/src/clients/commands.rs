use crate::clients::{
    delete_client, get_client, list_clients, set_client_default_bucket,
    unset_client_default_bucket, update_client_status,
};
use crate::config::{ClientCommand, Config};

use crate::Result;
use crate::db::create_db_pool;

use super::{NewClient, create_client};

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
