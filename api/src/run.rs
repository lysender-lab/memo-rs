use crate::Result;
use crate::buckets::run_bucket_command;
use crate::clients::run_client_command;
use crate::config::CliArgs;
use crate::config::Commands;
use crate::config::Config;
use crate::db::create_db_pool;
use crate::health::check_readiness;
use crate::users::run_user_command;
use crate::web::server::run_web_server;

pub async fn run_command(args: CliArgs) -> Result<()> {
    let config = Config::build(&args.config)?;
    match args.command {
        Commands::Server => run_web_server(&config).await,
        Commands::Clients(cmd) => run_client_command(cmd, &config).await,
        Commands::Buckets(cmd) => run_bucket_command(cmd, &config).await,
        Commands::Users(cmd) => run_user_command(cmd, &config).await,
        Commands::CheckHealth => check_health(&config).await,
    }
}

pub async fn check_health(config: &Config) -> Result<()> {
    let pool = create_db_pool(config.db.url.as_str());
    let health = check_readiness(config, &pool).await?;

    // Print the health status
    println!("Status: {}", health.status);
    println!("Message: {}", health.message);
    println!("Checks:");
    println!("  Cloud Storage: {}", health.checks.cloud_storage);
    println!("  Database: {}", health.checks.database);

    Ok(())
}
