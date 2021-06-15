//! Fleet entrypoint

mod config;

use crate::config::Config;
use crate::config::Network;
use anyhow::{anyhow, format_err, Result};
use cargo_toml::Manifest;
use clap::{crate_authors, crate_description, crate_version, AppSettings, Clap};
use colored::*;
use rand::rngs::OsRng;
use semver::Version;
use solana_sdk::signature::Signer;
use std::env;
use std::fs;
use std::process::Stdio;
use strum::VariantNames;
use tempfile::NamedTempFile;

#[derive(Debug, Clap)]
pub enum SubCommand {
    #[clap(about = "Initializes a new Fleet workspace.")]
    Init,
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

    // Gets a value for config if supplied by user, or defaults to "default.conf"
    println!("Value for config: {:?}", opts.command);

    match opts.command {
        SubCommand::Init => {
            println!("not implemented");
        }
        SubCommand::Deploy {
            version,
            program,
            ref network,
        } => {
            let (config, _, root) = Config::discover()?;

            let deploy_version = match version {
                Some(v) => v,
                None => {
                    let program_manifest = Manifest::from_path(
                        root.join("programs")
                            .join(program.clone())
                            .join("Cargo.toml"),
                    )
                    .map_err(|_| anyhow!("Program Cargo.toml not found"))?;
                    Version::parse(
                        program_manifest
                            .package
                            .ok_or_else(|| anyhow!("invalid package"))?
                            .version
                            .as_str(),
                    )?
                }
            };

            println!(
                "Deploying program {} with version {}",
                program, deploy_version
            );

            let program_bin_path = root
                .join("target")
                .join("deploy")
                .join(format!("{}.so", program));
            let program_idl_path = root
                .join("target")
                .join("idl")
                .join(format!("{}.json", program));
            let program_id_path = config.program_kp_path(&deploy_version, program.as_str());

            if !program_bin_path.exists() {
                return Err(anyhow!(
                    "Program bin path {} does not exist",
                    program_bin_path.display()
                ));
            }
            if !program_idl_path.exists() {
                return Err(anyhow!(
                    "Program idl path {} does not exist",
                    program_idl_path.display()
                ));
            }
            if !program_id_path.exists() {
                return Err(anyhow!(
                    "Program id path {} does not exist",
                    program_id_path.display()
                ));
            }

            let network_cfg = config.network_config(network)?;
            let deployer_path = network_cfg.deployer.as_path_buf();
            if !deployer_path.exists() {
                return Err(anyhow!(
                    "Program id path {} does not exist",
                    program_id_path.display()
                ));
            }

            let artifact_paths = config.artifact_paths(&deploy_version, &program.as_str());
            fs::create_dir_all(artifact_paths.root)?;

            let program_id_path_display = program_id_path.display();
            let program_key = solana_sdk::signer::keypair::read_keypair_file(&program_id_path)
                .map_err(|_| format_err!("could not read kp file {}", program_id_path_display))?
                .pubkey();
            println!("Address: {}", program_key);

            let exit = std::process::Command::new("solana")
                .args(&["program", "show", program_key.to_string().as_str()])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if exit.status.success() {
                println!("Program already deployed. Use `fleet upgrade` if you want to upgrade the program.");
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Deploying program");

            let exit = std::process::Command::new("solana")
                .args(&["program", "deploy"])
                .arg(&program_bin_path)
                .arg("--keypair")
                .arg(&deployer_path)
                .arg("--program-id")
                .arg(&program_id_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Setting upgrade authority");

            let exit = std::process::Command::new("solana")
                .args(&["program", "set-upgrade-authority"])
                .arg(&program_id_path)
                .arg("--keypair")
                .arg(&deployer_path)
                .arg("--new-upgrade-authority")
                .arg(&network_cfg.upgrade_authority)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            let exit = std::process::Command::new("solana")
                .args(&["program", "show", program_key.to_string().as_ref()])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Initializing IDL");

            let exit = std::process::Command::new("anchor")
                .args(&[
                    "idl",
                    "init",
                    program_key.to_string().as_str(),
                    "--filepath",
                ])
                .arg(&program_idl_path)
                .arg("--provider.cluster")
                .arg(network.to_string())
                .arg("--provider.wallet")
                .arg(&deployer_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Setting IDL authority");

            let exit = std::process::Command::new("anchor")
                .args(&["idl", "set-authority", "--program-id"])
                .arg(program_key.to_string())
                .arg("--new-authority")
                .arg(&network_cfg.upgrade_authority)
                .arg("--provider.cluster")
                .arg(network.as_ref())
                .arg("--provider.wallet")
                .arg(deployer_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Copying artifacts");

            let exit = std::process::Command::new("cp")
                .arg(program_bin_path)
                .arg(artifact_paths.bin)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }
            let exit = std::process::Command::new("cp")
                .arg(program_idl_path)
                .arg(artifact_paths.idl)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

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

            let (config, _, root) = Config::discover()?;

            let deploy_version = match version {
                Some(v) => v,
                None => {
                    let program_manifest = Manifest::from_path(
                        root.join("programs")
                            .join(program.clone())
                            .join("Cargo.toml"),
                    )
                    .map_err(|_| anyhow!("Program Cargo.toml not found"))?;
                    Version::parse(
                        program_manifest
                            .package
                            .ok_or_else(|| anyhow!("invalid package"))?
                            .version
                            .as_str(),
                    )?
                }
            };

            println!(
                "Deploying program {} with version {}",
                program, deploy_version
            );

            let program_bin_path = root
                .join("target")
                .join("deploy")
                .join(format!("{}.so", program));
            let program_idl_path = root
                .join("target")
                .join("idl")
                .join(format!("{}.json", program));
            let program_id_path = config.program_kp_path(&deploy_version, program.as_str());

            if !program_bin_path.exists() {
                return Err(anyhow!(
                    "Program bin path {} does not exist",
                    program_bin_path.display()
                ));
            }
            if !program_idl_path.exists() {
                return Err(anyhow!(
                    "Program idl path {} does not exist",
                    program_idl_path.display()
                ));
            }
            if !program_id_path.exists() {
                return Err(anyhow!(
                    "Program id path {} does not exist",
                    program_id_path.display()
                ));
            }

            let network_cfg = config.network_config(network)?;
            let deployer_path = network_cfg.deployer.as_path_buf();
            if !deployer_path.exists() {
                return Err(anyhow!(
                    "Program id path {} does not exist",
                    program_id_path.display()
                ));
            }

            let artifact_paths = config.artifact_paths(&deploy_version, &program.as_str());
            fs::create_dir_all(artifact_paths.root)?;

            if artifact_paths.bin.exists() || artifact_paths.idl.exists() {
                return Err(anyhow!("Program artifacts already exist for this version. Make sure to bump your Cargo.toml."));
            }

            let program_id_path_display = program_id_path.display();
            let program_key = solana_sdk::signer::keypair::read_keypair_file(&program_id_path)
                .map_err(|_| format_err!("could not read kp file {}", program_id_path_display))?
                .pubkey();
            println!("Address: {}", program_key);

            let exit = std::process::Command::new("solana")
                .args(&["program", "show", program_key.to_string().as_str()])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                println!("Program does not exist. Use `fleet deploy` if you want to deploy the program for the first time.");
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Writing buffer");

            let buffer_kp = solana_sdk::signer::keypair::Keypair::generate(&mut OsRng);
            let buffer_key = buffer_kp.pubkey();
            println!("Buffer Pubkey: {}", buffer_key);

            let mut buffer_file = NamedTempFile::new()?;
            solana_sdk::signer::keypair::write_keypair(&buffer_kp, &mut buffer_file)
                .map_err(|_| format_err!("could not generate temp buffer keypair"))?;

            let exit = std::process::Command::new("solana")
                .arg("program")
                .arg("write-buffer")
                .arg(&program_bin_path)
                .arg("--keypair")
                .arg(&deployer_path)
                .arg("--output")
                .arg("json")
                .arg("--buffer")
                .arg(&buffer_file.path())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Setting buffer authority");

            let exit = std::process::Command::new("solana")
                .arg("program")
                .arg("set-buffer-authority")
                .arg(buffer_key.to_string())
                .arg("--keypair")
                .arg(&deployer_path)
                .arg("--new-buffer-authority")
                .arg(&network_cfg.upgrade_authority)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Switching to new buffer (please connect your wallet)");

            let exit = std::process::Command::new("solana")
                .arg("program")
                .arg("deploy")
                .arg("--buffer")
                .arg(buffer_key.to_string())
                .arg("--keypair")
                .arg(&upgrade_authority_keypair)
                .arg("--program-id")
                .arg(program_key.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            let exit = std::process::Command::new("solana")
                .args(&["program", "show", program_key.to_string().as_ref()])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Uploading new IDL");

            let exit = std::process::Command::new("anchor")
                .arg("idl")
                .arg("write-buffer")
                .arg(program_key.to_string())
                .arg("--filepath")
                .arg(&program_idl_path)
                .arg("--provider.cluster")
                .arg(network.to_string())
                .arg("--provider.wallet")
                .arg(&deployer_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            println!(
                "WARNING: please manually run `anchor idl set-buffer {} --buffer <BUFFER>`",
                program_key.to_string()
            );
            println!("TODO: need to be able to hook into anchor for this");

            output_header("Copying artifacts");

            let exit = std::process::Command::new("cp")
                .arg(program_bin_path)
                .arg(artifact_paths.bin)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }
            let exit = std::process::Command::new("cp")
                .arg(program_idl_path)
                .arg(artifact_paths.idl)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

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
