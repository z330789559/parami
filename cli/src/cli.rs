// This file is part of Substrate.

// Copyright (C) 2018-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use sc_cli::RunCmd;
use structopt::StructOpt;
use std::path::PathBuf;

/// An overarching CLI command definition.
#[derive(Debug, StructOpt)]
pub struct Cli {
    /// Possible subcommand with parameters.
    #[structopt(subcommand)]
    pub subcommand: Option<Subcommand>,
    #[allow(missing_docs)]
    #[structopt(flatten)]
    pub run: RunCmd,
}

/// Possible subcommands of the main binary.
#[derive(Debug, StructOpt)]
pub enum Subcommand {
    /// Export the genesis state of the parachain.
    #[structopt(name = "export-genesis-state")]
    ExportGenesisState(ExportGenesisStateCommand),
    /// A set of base subcommands handled by `sc_cli`.
    #[structopt(flatten)]
    Key(sc_cli::KeySubcommand),
    /// Export the genesis wasm of the parachain.
    #[structopt(name = "export-genesis-wasm")]
    ExportGenesisWasm(ExportGenesisWasmCommand),

    /// The custom inspect subcommmand for decoding blocks and extrinsics.
    #[structopt(
        name = "inspect",
        about = "Decode given block or extrinsic using current native runtime."
    )]
    Inspect(node_inspect::cli::InspectCmd),

    /// The custom benchmark subcommmand benchmarking runtime pallets.
    #[structopt(name = "benchmark", about = "Benchmark runtime pallets.")]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),
}
#[derive(Debug, StructOpt)]
pub struct ExportGenesisStateCommand {
    /// Output file name or stdout if unspecified.
    #[structopt(parse(from_os_str))]
    pub output: Option<PathBuf>,

    /// Id of the parachain this state is for.
    #[structopt(long, default_value = "100")]
    pub parachain_id: u32,

    /// Write output in binary. Default is to write in hex.
    #[structopt(short, long)]
    pub raw: bool,

    /// The name of the chain for that the genesis state should be exported.
    #[structopt(long)]
    pub chain: Option<String>,
}
/// Command for exporting the genesis wasm file.
#[derive(Debug, StructOpt)]
pub struct ExportGenesisWasmCommand {
    /// Output file name or stdout if unspecified.
    #[structopt(parse(from_os_str))]
    pub output: Option<PathBuf>,

    /// Write output in binary. Default is to write in hex.
    #[structopt(short, long)]
    pub raw: bool,

    /// The name of the chain for that the genesis wasm file should be exported.
    #[structopt(long)]
    pub chain: Option<String>,
}

#[derive(Debug)]
pub struct RelayChainCli {
    /// The actual relay chain cli object.
    pub base: polkadot_cli::RunCmd,

    /// Optional chain id that should be passed to the relay chain.
    pub chain_id: Option<String>,

    /// The base path that should be used by the relay chain.
    pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
    /// Create a new instance of `Self`.
    pub fn new<'a>(
        base_path: Option<PathBuf>,
        chain_id: Option<String>,
        relay_chain_args: impl Iterator<Item = &'a String>,
    ) -> Self {
        Self {
            base_path,
            chain_id,
            base: polkadot_cli::RunCmd::from_iter(relay_chain_args),
        }
    }
}