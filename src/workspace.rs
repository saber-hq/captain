use crate::command;
use crate::config::ArtifactPaths;
use crate::config::NetworkConfig;
use crate::Config;
use crate::Network;
use anyhow::{anyhow, format_err, Result};
use cargo_toml::Manifest;
use semver::Version;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Deploys a program.
pub struct Workspace {
    pub root: PathBuf,
    pub network: Network,
    pub deployer_path: PathBuf,
    pub deploy_version: Version,
    pub program_paths: ProgramPaths,
    pub config: Config,
    pub network_config: NetworkConfig,
    pub artifact_paths: ArtifactPaths,
    pub program_key: Pubkey,
}

pub struct ProgramPaths {
    pub bin: PathBuf,
    pub idl: PathBuf,
    pub id: PathBuf,
}

pub fn load(program: &str, version: Option<Version>, network: Network) -> Result<Workspace> {
    let (config, _, root) = Config::discover()?;

    let deploy_version = get_deploy_version(program, &root, version)?;
    let program_paths = check_and_get_program_paths(&config, program, &root, &deploy_version)?;

    let network_config = config.network_config(&network)?;
    let deployer_path = network_config.deployer.as_path_buf();
    if !deployer_path.exists() {
        return Err(anyhow!(
            "Deployer path {} does not exist",
            deployer_path.display()
        ));
    }

    let artifact_paths = config.artifact_paths(&deploy_version, program);
    fs::create_dir_all(&artifact_paths.root)?;

    // TODO(igm): allow specifying pubkey without requiring the keyfile
    let program_id_path_display = program_paths.id.display();
    let program_key = solana_sdk::signer::keypair::read_keypair_file(&program_paths.id)
        .map_err(|_| format_err!("could not read kp file {}", program_id_path_display))?
        .pubkey();

    Ok(Workspace {
        config: config.clone(),
        network,
        root,
        network_config: network_config.clone(),
        deployer_path,
        deploy_version,
        program_paths,
        artifact_paths,
        program_key,
    })
}

fn check_and_get_program_paths(
    config: &Config,
    program: &str,
    root: &Path,
    deploy_version: &Version,
) -> Result<ProgramPaths> {
    let program_bin_path = root
        .join("target")
        .join("deploy")
        .join(format!("{}.so", program));
    let program_idl_path = root
        .join("target")
        .join("idl")
        .join(format!("{}.json", program));
    let program_id_path = config.program_kp_path(deploy_version, program);

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

    Ok(ProgramPaths {
        bin: program_bin_path,
        idl: program_idl_path,
        id: program_id_path,
    })
}

pub fn get_program_version(program: &str, root: &Path) -> Result<Version> {
    let mf_path = &root.join("programs").join(program).join("Cargo.toml");
    let program_manifest_path = if mf_path.exists() {
        mf_path.clone()
    } else {
        root.join("programs")
            .join(&program.replace("_", "-"))
            .join("Cargo.toml")
    };
    let program_manifest = Manifest::from_path(&program_manifest_path).map_err(|_| {
        format_err!(
            "Program Cargo.toml not found at paths {} or {}",
            &mf_path.display(),
            &program_manifest_path.display()
        )
    })?;
    Ok(Version::parse(
        program_manifest
            .package
            .ok_or_else(|| anyhow!("invalid package"))?
            .version
            .as_str(),
    )?)
}

fn get_deploy_version(program: &str, root: &Path, version: Option<Version>) -> Result<Version> {
    match version {
        Some(v) => Ok(v),
        None => get_program_version(program, root),
    }
}

impl Workspace {
    pub fn show_program(&self) -> Result<bool> {
        let exit = command::exec_unhandled(
            solana_cmd!(self)
                .arg("program")
                .arg("show")
                .arg(self.program_key.to_string()),
        )?;
        Ok(exit.status.success())
    }

    pub fn copy_artifacts(&self) -> Result<()> {
        command::exec(
            std::process::Command::new("cp")
                .arg(&self.program_paths.bin)
                .arg(&self.artifact_paths.bin),
        )?;
        command::exec(
            std::process::Command::new("cp")
                .arg(&self.program_paths.idl)
                .arg(&self.artifact_paths.idl),
        )?;
        Ok(())
    }

    /// Returns true if this is also an Anchor workspace.
    pub fn has_anchor(&self) -> bool {
        self.root.join("Anchor.toml").exists()
    }

    pub fn network_url(&self) -> String {
        self.network_config
            .url
            .clone()
            .unwrap_or_else(|| self.network.url().to_string())
    }
}
