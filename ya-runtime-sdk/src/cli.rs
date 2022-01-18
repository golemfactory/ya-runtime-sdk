use std::path::PathBuf;
use structopt::{clap, StructOpt};

pub fn parse_cli<C: CommandCli>(
    name: &str,
    version: &str,
    args: Box<dyn Iterator<Item = String>>,
) -> anyhow::Result<C> {
    let app = C::clap().name(name).version(version);
    let iter = &app.get_matches_from(args);
    Ok(C::from_clap(iter))
}

pub trait CommandCli: StructOpt + Send {
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

impl Command {
    pub fn args(&self) -> &Vec<String> {
        match self {
            Self::Deploy { args }
            | Self::Start { args }
            | Self::Run { args }
            | Self::OfferTemplate { args }
            | Self::Test { args } => args,
        }
    }
}

#[derive(StructOpt)]
pub struct EmptyArgs {}
