use anyhow::{anyhow, format_err, Error, Result};
use cargo_toml::Manifest;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use strum_macros::{AsRefStr, Display, EnumString, EnumVariantNames, IntoStaticStr};

#[derive(
    AsRefStr,
    Clone,
    Debug,
    Display,
    EnumString,
    EnumVariantNames,
    Eq,
    IntoStaticStr,
    Ord,
    PartialEq,
    PartialOrd,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum Network {
    Testnet,
    Mainnet,
    Devnet,
    Localnet,
    Debug,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub paths: Paths,
    /// Network configuration
    pub networks: BTreeMap<Network, NetworkConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Paths {
    /// Where binaries are stored
    pub artifacts: FleetPath,
    /// Where deployment info is stored
    pub deployments: FleetPath,
    /// Where program address keypairs are stored
    pub program_keypairs: FleetPath,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub deployer: FleetPath,
    /// The upgrade authority keypair. Can be a ledger via usb://ledger?key=n
    pub upgrade_authority: String,
}

pub struct ArtifactPaths {
    pub root: PathBuf,
    pub program: PathBuf,
    pub idl: PathBuf,
}

impl Config {
    /// Path to the keypair of the deployer.
    pub fn network_config(&self, network: &Network) -> Result<&NetworkConfig> {
        self.networks
            .get(network)
            .ok_or(format_err!("network {} not found", network))
    }

    /// Path to the keypair of a program.
    pub fn program_kp_path(&self, version: &Version, program: &str) -> PathBuf {
        self.paths
            .program_keypairs
            .0
            .join(format!("{}-{}.x.json", program, version.major))
    }

    /// Path to where program binaries should be saved.
    pub fn artifact_paths(&self, version: &Version, program: &str) -> ArtifactPaths {
        let root = self
            .paths
            .artifacts
            .0
            .join(program)
            .join(version.to_string());
        ArtifactPaths {
            root: root.clone(),
            program: root.join("program.so"),
            idl: root.join("idl.json"),
        }
    }

    // Searches all parent directories for a Fleet.toml and Cargo.toml file.
    pub fn discover() -> Result<(Self, Manifest, PathBuf)> {
        // Set to true if we ever see a Cargo.toml file when traversing the
        // parent directories.

        let _cwd = std::env::current_dir()?;
        let mut cwd_opt = Some(_cwd.as_path());

        while let Some(cwd) = cwd_opt {
            let files = fs::read_dir(cwd)?;
            // Cargo.toml file for this directory level.
            for f in files {
                let p = f?.path();
                if let Some(filename) = p.file_name() {
                    if filename.to_str() == Some("Fleet.toml") {
                        let mut cfg_file = File::open(&p)?;
                        let mut cfg_contents = String::new();
                        cfg_file.read_to_string(&mut cfg_contents)?;
                        let cfg = cfg_contents.parse()?;
                        let cwd_buf = cwd.to_path_buf();
                        return Ok((
                            cfg,
                            Manifest::from_path(cwd_buf.join("Cargo.toml"))?,
                            cwd_buf,
                        ));
                    }
                }
            }

            cwd_opt = cwd.parent();
        }

        return Err(anyhow!("Cargo.toml and Fleet.toml not found"));
    }
}

#[derive(Debug, Default, Serialize, DeserializeFromStr)]
pub struct FleetPath(PathBuf);

impl FleetPath {
    pub fn as_path_buf(&self) -> PathBuf {
        self.0.clone()
    }
}

impl FromStr for FleetPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(FleetPath(PathBuf::from_str(
            shellexpand::tilde(s).to_string().as_str(),
        )?))
    }
}

impl FromStr for Config {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str(s)
            .map_err(|e| anyhow::format_err!("Unable to deserialize config: {}", e.to_string()))
    }
}
