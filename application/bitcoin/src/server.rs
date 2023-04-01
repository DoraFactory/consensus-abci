use std::{env::{current_dir, self}, sync::Arc};

use anyhow::Result;
use bitcoin::{Node, SledDb};
// use clap::{crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut path = String::from("data");
    if let Some(args) = env::args().nth(2) {
        path = args;
    }

    let path = current_dir().unwrap().join(path);
    let db = Arc::new(SledDb::new(path));
    let mut node = Node::new(db).await?;
/*     let matches = App::new(crate_name!())
        .version(crate_version!())
        .about("a bitcoin")
        .arg_from_usage("-v... 'Sets the level of verbosity'")
        .subcommand(
            SubCommand::with_name("genesis")
        ) */



    node.start().await?;
    Ok(())
}