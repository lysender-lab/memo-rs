use snafu::ResultExt;
use text_io::read;

use crate::Result;
use crate::client::NewClient;
use crate::config::Config;
use crate::error::PasswordPromptSnafu;
use crate::state::create_app_state;

use crate::auth::user::NewUser;
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

    let state = create_app_state(config).await?;

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
        let client = create_client(&state, &new_client, true).await?;
        println!("{{ id = {}, name = {} }}", client.id, client.name);
        println!("Created system admin client.");
        client_id = client.id;
    }

    let users = state.db.users.list(&client_id).await?;
    if users.len() > 0 {
        println!("Admin user already exists.");
        return Ok(());
    }

    let user = state.db.users.create(&client_id, &new_user, true).await?;
    println!(
        "{{ id = {}, username = {} status = {} }}",
        user.id, user.username, user.status
    );
    println!("Created system admin user.");
    Ok(())
}
