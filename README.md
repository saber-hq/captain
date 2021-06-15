# captain 🧑‍✈️

[![Crates.io](https://img.shields.io/crates/v/captain?style=flat-square)](https://crates.io/crates/captain)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/saber-hq/captain/blob/master/LICENSE-APACHE)
[![Build Status](https://img.shields.io/github/workflow/status/saber-hq/captain/CI/master?style=flat-square)](https://github.com/saber-hq/captain/actions/workflows/ci.yml?query=branch%3Amaster)
[![Contributors](https://img.shields.io/github/contributors/saber-hq/captain?style=flat-square)](https://github.com/saber-hq/captain/graphs/contributors)

Version control and key management for [Solana](https://solana.com/) programs.

- Automatic versioning of program binaries based on [Cargo](https://doc.rust-lang.org/cargo)
- Separation of deployer and authority keys
- Per-cluster configuration
- Reusable and custom program addresses
- _(optional)_ Integration with [Anchor](https://project-serum.github.io/anchor/) IDLs

## Setup

Install via Cargo like so:

```
cargo install --git https://github.com/saber-hq/captain --force
```

Then, in your directory containing your root `Cargo.toml`, run the following command:

```
captain init
```

## Usage

A Captain workflow works like so:

1. Build your latest programs via `captain build`
2. Deploy any new programs with `captain deploy`
3. Upgrade any new programs with `captain upgrade`

### Build

First, build your programs using the command:

```bash
captain build
```

This runs `anchor build -v` if you have Anchor installed, and `cargo build-bpf` if you don't have Anchor installed.

### Deploy

Once your new program is ready to be deployed to the blockchain, run:

```
captain deploy --program <program> --network <network>
```

where:

- `<network>` is one of `mainnet`, `devnet`, `testnet`, `localnet`, or `debug`
- `<program>` is the name of your program in the `target/deploy/` directory. For example, if your program is at `target/deploy/my_flash_loan_program.so`, your `<program>` is `my_flash_loan_program`.

This does several things for you:

- Deploys the program
- Sets the deployer of the program to the `upgrade_authority` of `<network>` in your `Captain.toml` file
- If Anchor is installed, uploads the IDLs and sets the authority to the `upgrade_authority`

### Upgrading

If you need to make changes to your program, you can run:

```
captain upgrade --program <program> --network <network>
```

This performs the following actions:

- Writes a new program buffer
- Replaces the existing program buffer with this new buffer using the upgrade authority
- (WIP) If Anchor is installed, uploads the new IDL

## License

Apache-2.0
