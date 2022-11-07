#![cfg(feature = "macros")]
mod utils;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ya_runtime_sdk::*;

type RuntimeCli = <Runtime as RuntimeDef>::Cli;

#[derive(structopt::StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Cli {
    #[structopt(long)]
    param: usize,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct Conf {
    numeric: i64,
    string: String,
    vec: Vec<String>,
}

impl Default for Conf {
    fn default() -> Self {
        Conf {
            numeric: 42,
            string: String::from("default value"),
            vec: vec!["first entry".to_string(), "second entry".to_string()],
        }
    }
}

#[derive(Debug)]
pub struct Env {
    temp_dir: tempdir::TempDir,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            temp_dir: tempdir::TempDir::new("ya-runtime-sdk")
                .expect("Cannot create a temp directory"),
        }
    }
}

impl ya_runtime_sdk::env::Env<RuntimeCli> for Env {
    fn data_directory(&self, _: &str) -> anyhow::Result<PathBuf> {
        Ok(self.temp_dir.path().to_path_buf())
    }

    fn args(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(
            vec![
                env!("CARGO_PKG_NAME").to_string(),
                "--workdir".to_string(),
                self.temp_dir.path().display().to_string(),
                "--param".to_string(),
                42.to_string(),
                "deploy".to_string(),
                "deploy-arg".to_string(),
            ]
            .into_iter(),
        )
    }
}

#[derive(ya_runtime_sdk::RuntimeDef, Default)]
#[conf(Conf)]
#[cli(Cli)]
struct Runtime;
impl_empty_runtime!(Runtime);

#[test]
fn context_env() {
    let env = Env::default();
    let dir = env.temp_dir.path().to_path_buf();

    let context = Context::<Runtime>::try_with(env).expect("Failed to initialize runtime context");
    let deploy = Command::Deploy {
        args: vec!["deploy-arg".to_string()],
    };

    assert_eq!(context.conf, Conf::default());
    assert_eq!(context.cli.workdir, Some(dir));
    assert_eq!(context.cli.runtime.param, 42);
    assert_eq!(context.cli.command, deploy);
}

#[test]
fn conf_file() {
    let temp_dir = Env::default().temp_dir;
    let path = temp_dir.path();
    let conf = Conf::default();

    let check = |ext: &str| {
        let p = path.join(format!("config.{}", ext));

        Context::<Runtime>::write_config(&conf, &p).expect("Error writing config to file");
        let read = Context::<Runtime>::read_config(&p).expect("Error reading config from file");
        std::fs::remove_file(p).expect("Unable to remove file");

        assert_eq!(conf, read);
    };

    check("json");
    check("JSON");
    check("JsoN");
    check("yaml");
    check("yml");
    check("toml");
}

#[test]
fn conf_file_err() {
    let temp_dir = Env::default().temp_dir;
    let path = temp_dir.path();
    let conf = Conf::default();

    let write_path = path.join("config.json");
    Context::<Runtime>::write_config(&conf, &write_path).expect("Error writing config to file");

    let read = Context::<Runtime>::read_config(&write_path);
    assert_eq!(read.is_ok(), true);

    let renamed_path = path.join("config.toml");
    std::fs::rename(&write_path, &renamed_path).expect("Failed to rename file");

    let read = Context::<Runtime>::read_config(&renamed_path);
    assert_eq!(read.is_ok(), false);
}
