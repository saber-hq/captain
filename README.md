# fleet 🚢

[![Crates.io](https://img.shields.io/crates/v/fleet-cli?style=flat-square)](https://crates.io/crates/fleet-cli)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/saber-hq/fleet/blob/master/LICENSE-APACHE)
[![Build Status](https://img.shields.io/github/workflow/status/saber-hq/fleet/CI/master?style=flat-square)](https://github.com/saber-hq/fleet/actions/workflows/ci.yml?query=branch%3Amaster)
[![Contributors](https://img.shields.io/github/contributors/saber-hq/fleet?style=flat-square)](https://github.com/saber-hq/fleet/graphs/contributors)

Version control and key management for [Solana](https://solana.com/) programs.

- Automatic versioning of program binaries based on [Cargo](https://doc.rust-lang.org/cargo)
- Separation of deployer and authority keys
- Per-cluster configuration
- Reusable and custom program addresses
- _(optional)_ Integration with [Anchor](https://project-serum.github.io/anchor/) IDLs

## Installation

Install via Cargo like so:

```
cargo install --git https://github.com/saber-hq/fleet --force
```

## Usage

A Fleet workflow works like so:

1. Build your latest programs via `fleet build`
2. Deploy any new programs with `fleet deploy`
3. Upgrade any new programs with `fleet upgrade`

### Build

First, build your programs using the command:

```bash
fleet build
```

This runs `anchor build -v` if you have Anchor installed, and `cargo build-bpf` if you don't have Anchor installed.

## License

Apache-2.0
