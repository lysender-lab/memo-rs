use snafu::ResultExt;
use text_io::read;

use crate::Result;
use crate::auth::user::create_user;
use crate::client::create_client;
use crate::config::Config;
use crate::error::{DbSnafu, PasswordPromptSnafu};
use crate::state::create_app_state;
use db::client::NewClient;
use db::user::NewUser;

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

    let state = create_app_state(config).await?;

    let client_id: String;
    let admin_client = state.db.clients.find_admin().await.context(DbSnafu)?;
    if let Some(client) = admin_client {
        client_id = client.id;
    } else {
        let new_client = NewClient {
            name: "system-admin".to_string(),
            status: "active".to_string(),
            default_bucket_id: None,
        };
        let client = create_client(&state, new_client, true).await?;
        println!("{{ id = {}, name = {} }}", client.id, client.name);
        println!("Created system admin client.");
        client_id = client.id;
    }

    let users = state.db.users.list(&client_id).await.context(DbSnafu)?;
    if !users.is_empty() {
        println!("Admin user already exists.");
        return Ok(());
    }

    let user = create_user(&state, &client_id, &new_user, true).await?;
    println!(
        "{{ id = {}, username = {} status = {} }}",
        user.id, user.username, user.status
    );
    println!("Created system admin user.");
    Ok(())
}
