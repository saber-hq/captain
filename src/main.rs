//! Captain entrypoint
#[macro_use]
mod macros;

mod command;
mod config;
mod workspace;

use crate::config::CaptainPath;
use crate::config::Config;
use crate::config::Network;
use crate::config::NetworkConfig;
use anyhow::{anyhow, format_err, Result};
use clap::{crate_authors, crate_description, crate_version, AppSettings, Clap};
use colored::*;
use semver::Version;
use solana_sdk::signature::Signer;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use strum::VariantNames;
use tempfile::NamedTempFile;

#[derive(Debug, Clap)]
pub enum SubCommand {
    #[clap(about = "Initializes a new Captain workspace.")]
    Init,
    #[clap(about = "Builds all programs. (Uses Anchor)")]
    Build,
    #[clap(about = "Lists all available programs.")]
    Programs,
    #[clap(about = "Deploys a program.")]
    Deploy {
        #[clap(short, long)]
        version: Option<Version>,
        #[clap(short, long)]
        #[clap(about = "Name of the program in target/deploy/<id>.so")]
        program: String,
        #[clap(short, long)]
        #[clap(about = "Network to deploy to")]
        #[clap(
            default_value = Network::Devnet.into(),
            possible_values = Network::VARIANTS
        )]
        network: Network,
    },
    #[clap(about = "Upgrades a program.")]
    Upgrade {
        #[clap(short, long)]
        version: Option<Version>,
        #[clap(short, long)]
        #[clap(about = "Name of the program in target/deploy/<id>.so")]
        program: String,
        #[clap(short, long)]
        #[clap(about = "Network to deploy to")]
        #[clap(
            default_value = Network::Devnet.into(),
            possible_values = Network::VARIANTS
        )]
        network: Network,
    },
}

#[derive(Debug, Clap)]
#[clap(about = crate_description!())]
#[clap(version = crate_version!())]
#[clap(author = crate_authors!())]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    #[clap(subcommand)]
    command: SubCommand,
}

fn main_with_result() -> Result<()> {
    let opts: Opts = Opts::parse();

    match opts.command {
        SubCommand::Init => {
            if std::env::current_dir()?.join("Captain.toml").exists() {
                println!(
                    "{}",
                    "Captain.toml has already been initialized in this directory.".red()
                );
                std::process::exit(1);
            }
            if !std::env::current_dir()?.join("Cargo.toml").exists() {
                println!(
                    "{}",
                    "Cargo.toml does not exist in the current working directory. Ensure that you are at the Cargo workspace root.".red()
                );
                std::process::exit(1);
            }
            let mut cfg = Config::default();

            let deployers_root = PathBuf::from("./.captain/deployers/");
            std::fs::create_dir_all(&deployers_root)?;

            for network in &[
                Network::Mainnet,
                Network::Devnet,
                Network::Testnet,
                Network::Localnet,
            ] {
                let deployer_kp = solana_sdk::signer::keypair::Keypair::new();
                let deployer_path =
                    deployers_root.join(format!("{}/deployer.json", network.to_string()));
                solana_sdk::signer::keypair::write_keypair_file(&deployer_kp, &deployer_path)
                    .map_err(|_| format_err!("could not generate temp buffer keypair"))?;

                let networks = &mut cfg.networks;
                networks.insert(
                    network.clone(),
                    NetworkConfig {
                        deployer: CaptainPath(deployer_path),
                        url: network.url().to_string().into(),
                        ws_url: network.ws_url().to_string().into(),
                        upgrade_authority: "~/.config/solana/id.json".to_string(),
                    },
                );
            }

            let toml = toml::to_string(&cfg)?;
            let mut file = File::create("Captain.toml")?;
            file.write_all(toml.as_bytes())?;
        }
        SubCommand::Build => {
            let (_, _, root) = Config::discover()?;
            if root.join("Anchor.toml").exists() {
                println!("{}", "Anchor found! Running `anchor build -v`.".green());
                command::exec(Command::new("anchor").arg("build").arg("-v"))?;
            } else {
                println!(
                    "{}",
                    "Anchor.toml not found in workspace root. Running `cargo build-bpf`.".yellow()
                );
                command::exec(Command::new("cargo").arg("build-bpf"))?;
            }
        }
        SubCommand::Programs => {
            let paths = std::fs::read_dir("./target/deploy/").unwrap();
            for path in paths {
                let the_path = path?.path();
                if the_path.extension().and_then(|ex| ex.to_str()) != Some("so") {
                    continue;
                }
                println!(
                    "Program: {}",
                    the_path
                        .file_stem()
                        .ok_or_else(|| format_err!("no file stem"))?
                        .to_str()
                        .ok_or_else(|| format_err!("no str"))?
                )
            }
        }
        SubCommand::Deploy {
            version,
            program,
            ref network,
        } => {
            let workspace = &workspace::load(program.as_str(), version, network.clone())?;
            println!(
                "Deploying program {} with version {}",
                program, workspace.deploy_version
            );

            println!("Address: {}", workspace.program_key);

            if workspace.show_program()? {
                println!("Program already deployed. Use `captain upgrade` if you want to upgrade the program.");
                std::process::exit(0);
            }

            output_header("Deploying program");

            command::exec(
                solana_cmd!(workspace)
                    .arg("program")
                    .arg("deploy")
                    .arg(&workspace.program_paths.bin)
                    .arg("--program-id")
                    .arg(&workspace.program_paths.id),
            )?;

            output_header("Setting upgrade authority");

            command::exec(
                solana_cmd!(workspace)
                    .arg("program")
                    .arg("set-upgrade-authority")
                    .arg(&workspace.program_paths.id)
                    .arg("--new-upgrade-authority")
                    .arg(&workspace.network_config.upgrade_authority),
            )?;

            workspace.show_program()?;

            if workspace.has_anchor() {
                output_header("Initializing IDL");
                command::exec(
                    anchor_cmd!(workspace, "idl")
                        .arg("init")
                        .arg(&workspace.program_key.to_string())
                        .arg("--filepath")
                        .arg(&workspace.program_paths.idl),
                )?;

                output_header("Setting IDL authority");
                command::exec(
                    anchor_cmd!(workspace, "idl")
                        .arg("set-authority")
                        .arg("--program-id")
                        .arg(workspace.program_key.to_string())
                        .arg("--new-authority")
                        .arg(&workspace.network_config.upgrade_authority),
                )?;
            }

            output_header("Copying artifacts");
            workspace.copy_artifacts()?;

            println!("Deployment success!");
        }
        SubCommand::Upgrade {
            version,
            program,
            ref network,
        } => {
            let upgrade_authority_keypair =
                env::var("UPGRADE_AUTHORITY_KEYPAIR").map_err(|_| {
                    format_err!("Must set UPGRADE_AUTHORITY_KEYPAIR environment variable.")
                })?;

            let workspace = workspace::load(program.as_str(), version, network.clone())?;
            println!(
                "Upgrading program {} with version {}",
                program, workspace.deploy_version
            );

            if workspace.artifact_paths.exist() {
                return Err(anyhow!("Program artifacts already exist for this version. Make sure to bump your Cargo.toml."));
            }

            if !workspace.show_program()? {
                println!("Program does not exist. Use `captain deploy` if you want to deploy the program for the first time.");
                std::process::exit(1);
            }

            output_header("Writing buffer");

            let buffer_kp = solana_sdk::signer::keypair::Keypair::new();
            let buffer_key = buffer_kp.pubkey();
            println!("Buffer Pubkey: {}", buffer_key);

            let mut buffer_file = NamedTempFile::new()?;
            solana_sdk::signer::keypair::write_keypair(&buffer_kp, &mut buffer_file)
                .map_err(|_| format_err!("could not generate temp buffer keypair"))?;

            command::exec(
                solana_cmd!(workspace)
                    .arg("program")
                    .arg("write-buffer")
                    .arg(&workspace.program_paths.bin)
                    .arg("--output")
                    .arg("json")
                    .arg("--buffer")
                    .arg(&buffer_file.path()),
            )?;

            output_header("Setting buffer authority");

            command::exec(
                solana_cmd!(workspace)
                    .arg("program")
                    .arg("set-buffer-authority")
                    .arg(buffer_key.to_string())
                    .arg("--new-buffer-authority")
                    .arg(&workspace.network_config.upgrade_authority),
            )?;

            output_header("Switching to new buffer (please connect your wallet)");

            command::exec(
                Command::new("solana")
                    .arg("--url")
                    .arg(&workspace.network_url())
                    .arg("--keypair")
                    .arg(&upgrade_authority_keypair)
                    .arg("program")
                    .arg("deploy")
                    .arg("--buffer")
                    .arg(buffer_key.to_string())
                    .arg("--program-id")
                    .arg(workspace.program_key.to_string()),
            )?;

            workspace.show_program()?;

            if workspace.has_anchor() {
                output_header("Uploading new IDL");
                command::exec(
                    anchor_cmd!(workspace, "idl")
                        .arg("write-buffer")
                        .arg(workspace.program_key.to_string())
                        .arg("--filepath")
                        .arg(&workspace.program_paths.idl),
                )?;

                println!(
                    "WARNING: please manually run `anchor idl set-buffer {} --buffer <BUFFER>`",
                    workspace.program_key.to_string()
                );
                println!("TODO: need to be able to hook into anchor for this");
            }

            output_header("Copying artifacts");
            workspace.copy_artifacts()?;

            println!("Deployment success!");
        }
    }

    Ok(())
}

fn output_header(header: &'static str) {
    println!();
    println!("{}", "===================================".bold());
    println!();
    println!("    {}", header.bold());
    println!();
    println!("{}", "===================================".bold());
    println!();
}

fn main() {
    if let Err(err) = main_with_result() {
        println!("Error: {}", err);
        std::process::exit(1);
    }
}
