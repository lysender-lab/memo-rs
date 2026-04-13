use crate::Result;
use crate::config::Config;
use crate::web::server::run_web_server;

pub async fn run_command() -> Result<()> {
    let config = Config::build()?;
    run_web_server(&config).await?;

    Ok(())
}
