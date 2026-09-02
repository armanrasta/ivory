//! Ivory keygen CLI.

use clap::Parser;
use ivory_keygen::generate;

#[derive(Parser)]
#[command(name = "ivory-keygen", about = "Generate an Ivory Ed25519 keypair")]
struct Cli {}

fn main() {
    let _ = Cli::parse();
    let (sk, pk, addr) = generate();
    println!("address:    {}", addr.to_hex());
    println!("public_key: {}", pk.to_hex());
    println!("secret_key: {}", sk.to_hex());
}
