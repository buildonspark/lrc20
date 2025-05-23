use clap::Subcommand;
use color_eyre::eyre;

use self::{
    address::GenerateAddressArgs, config::GenerateConfigArgs, hash::GenerateReceiptHashArgs,
};
use crate::{actions::generate::keypair::GenerateKeypairArgs, context::Context};

mod address;
mod config;
mod hash;
mod keypair;
mod utils;

#[derive(Subcommand, Debug)]
pub enum GenerateCommands {
    /// Generate secret key, public key and address.
    Keypair(GenerateKeypairArgs),
    Address(GenerateAddressArgs),
    ReceiptHash(GenerateReceiptHashArgs),

    /// Generate a configuration file with random keys.
    Config(GenerateConfigArgs),
}

pub fn run(cmd: GenerateCommands, context: Context) -> eyre::Result<()> {
    match cmd {
        GenerateCommands::Keypair(args) => keypair::run(args, context),
        GenerateCommands::Address(args) => address::run(args),
        GenerateCommands::ReceiptHash(args) => hash::run(args),
        GenerateCommands::Config(args) => config::run(args, context),
    }
}
