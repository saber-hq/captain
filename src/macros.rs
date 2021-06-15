macro_rules! solana_cmd {
    ($workspace:expr) => {
        std::process::Command::new("solana")
            .arg("--url")
            .arg(&$workspace.network_url())
            .arg("--keypair")
            .arg(&$workspace.deployer_path)
    };
}

macro_rules! anchor_cmd {
    ($workspace:expr, $cmd:expr) => {
        std::process::Command::new("anchor")
            .arg($cmd)
            .arg("--provider.cluster")
            .arg(&$workspace.network.to_string())
            .arg("--provider.wallet")
            .arg(&$workspace.deployer_path)
    };
}
