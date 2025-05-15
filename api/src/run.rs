use crate::Result;
use crate::command::run_setup;
use crate::config::CliArgs;
use crate::config::Commands;
use crate::config::Config;
use crate::web::server::run_web_server;

pub async fn run_command(args: CliArgs) -> Result<()> {
    let config = Config::build(&args.config)?;
    match args.command {
        Commands::Server => run_web_server(&config).await,
        Commands::Setup => run_setup(&config).await,
    }
}
