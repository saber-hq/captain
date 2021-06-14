//! Fleet entrypoint

mod config;

use crate::config::Config;
use crate::config::Network;
use anyhow::{anyhow, format_err, Result};
use cargo_toml::Manifest;
use clap::{crate_description, crate_version, AppSettings, Clap};
use colored::*;
use semver::Version;
use solana_sdk::signature::Signer;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use strum::VariantNames;

#[derive(Debug, Clap)]
pub enum SubCommand {
    Init,
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
}

#[derive(Debug, Clap)]
#[clap(author = "Saber Team <team@saber.so>")]
#[clap(version = crate_version!())]
#[clap(about = crate_description!())]
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
                .args(&[
                    "program",
                    "deploy",
                    path_to_str(&program_bin_path, "bin path not valid")?.as_str(),
                    "--keypair",
                    path_to_str(&deployer_path, "deployer kp path not valid")?.as_str(),
                    "--program-id",
                    path_to_str(&program_id_path, "program id path not valid")?.as_str(),
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Setting upgrade authority");

            let exit = std::process::Command::new("solana")
                .args(&[
                    "program",
                    "set-upgrade-authority",
                    path_to_str(&program_id_path, "program id path not valid")?.as_str(),
                    "--keypair",
                    path_to_str(&deployer_path, "deployer kp path not valid")?.as_str(),
                    "--new-upgrade-authority",
                    network_cfg.upgrade_authority.as_str(),
                ])
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
                    path_to_str(&program_idl_path, "program idl path invalid")?.as_str(),
                    "--provider.cluster",
                    network.into(),
                    "--provider.wallet",
                    path_to_str(&deployer_path, "deployer")?.as_str(),
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Setting IDL authority");

            let exit = std::process::Command::new("anchor")
                .args(&[
                    "idl",
                    "set-authority",
                    "--program-id",
                    program_key.to_string().as_ref(),
                    "--new-authority",
                    network_cfg.upgrade_authority.as_str(),
                    "--provider.cluster",
                    network.as_ref(),
                    "--provider.wallet",
                    path_to_str(&deployer_path, "deployer")?.as_str(),
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            output_header("Copying artifacts");

            let exit = std::process::Command::new("cp")
                .args(&[
                    path_to_str(&program_bin_path, "program bin")?,
                    path_to_str(&artifact_paths.program, "program")?,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| anyhow::format_err!("Error deploying: {}", e.to_string()))?;
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap_or(1));
            }
            let exit = std::process::Command::new("cp")
                .args(&[
                    path_to_str(&program_idl_path, "program idl")?,
                    path_to_str(&artifact_paths.idl, "program")?,
                ])
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
    println!("{}", "===================================".bold());
    println!("");
    println!("    {}", header.bold());
    println!("");
    println!("{}", "===================================".bold());
}

fn path_to_str(program_bin_path: &PathBuf, err_msg: &'static str) -> Result<String> {
    Ok(fs::canonicalize(program_bin_path)?
        .to_str()
        .ok_or(anyhow!(err_msg))?
        .to_string())
}

fn main() {
    if let Err(err) = main_with_result() {
        println!("Error: {}", err)
    }
}
