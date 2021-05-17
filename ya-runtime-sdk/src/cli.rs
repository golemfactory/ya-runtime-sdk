use std::path::PathBuf;
use structopt::{clap, StructOpt};

pub trait CommandCli: StructOpt {
    fn workdir(&self) -> Option<PathBuf>;
    fn command(&self) -> &Command;
}

#[derive(Clone, Debug, Eq, PartialEq, StructOpt)]
#[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
pub enum Command {
    /// Deploy the runtime
    Deploy { args: Vec<String> },
    /// Start the runtime
    Start { args: Vec<String> },
    /// Run a runtime command
    Run { args: Vec<String> },
    /// Output a market offer template JSON
    OfferTemplate { args: Vec<String> },
    /// Perform a self-test
    Test { args: Vec<String> },
}

#[derive(StructOpt)]
pub struct EmptyArgs {}
