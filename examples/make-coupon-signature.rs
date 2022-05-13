use schnorrkel::{Keypair, MiniSecretKey, signing_context};
use sp_core::crypto::{Ss58Codec, AccountId32};
use hex::{self, FromHex};
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about = "Simple program for generate signature for activate coupon in smartcontract", long_about = None)]
struct Args {
    #[clap(long, help = "Contract address SS58")]
    contract: String,

    #[clap(long, help = "Coupon secret key 0x...")]
    coupon: String,

    #[clap(long, help = "Receiver address SS58")]
    receiver: String,

    #[clap(long, help = "Output only hex signature")]
    short: bool
}

#[derive(Debug)]
enum Error {
    ParseSS58(String),
    InvalidSecret,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Contract address
    let contract_address =
        AccountId32::from_ss58check(&*args.contract).or(Err(Error::ParseSS58("contract".to_string())))?;
    let contract_address_context_bytes: &[u8; 32] = contract_address.as_ref();

    // Receiver address
    let receiver_address =
        AccountId32::from_ss58check(&*args.receiver).or(Err(Error::ParseSS58("receiver".to_string())))?;
    let receiver_address_bytes: &[u8; 32] = receiver_address.as_ref();

    // Coupon private key (is Charlie account)
    let coupon_hex = args.coupon.strip_prefix("0x").ok_or(Error::InvalidSecret)?;
    let coupon_secret_bytes = <[u8; 32]>::from_hex(&*coupon_hex).or(Err(Error::InvalidSecret))?;
    let coupon = MiniSecretKey::from_bytes(&coupon_secret_bytes).or(Err(Error::InvalidSecret))?;

    // Make signature
    let keypair = Keypair::from(coupon.expand(MiniSecretKey::ED25519_MODE));
    let context = signing_context(contract_address_context_bytes);
    let signature = keypair.sign(context.bytes(receiver_address_bytes));
    let hex_signature = hex::encode(signature.to_bytes());

    if args.short {
        println!("0x{:}", hex_signature);
    } else {
        println!("---------------------------------------");
        println!("Contract Address: {:}", args.contract);
        println!("Payout Receiver: {:}", args.receiver);
        println!("Coupon Secret Key: {:}", args.coupon);
        println!("Signature: 0x{:}", hex_signature);
    }

    Ok(())
}
