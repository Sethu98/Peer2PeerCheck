use std::str::FromStr;

use bitcoin::address::Address;
use bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use clap::Parser;
use futures::{AsyncReadExt, executor::block_on, stream::StreamExt};
use tokio::{io, select};
use tokio::io::AsyncBufReadExt;
use tokio::time::{Duration, Instant, sleep};

macro_rules! conditional_print {
    ($condition:expr, $($arg:tt)*) => {
        if $condition {
            println!($($arg)*);
        }
    };
}

#[derive(Debug, Parser)]
#[clap(name = "btc client")]
struct Opts {
    #[clap(long)]
    #[arg(required = true)]
    wallet_name: String,
}

fn check_block_count(rpc_client: &Client) {
    let block_count = rpc_client.get_block_count().expect("Failed to get block count");
    println!("Current block count: {}", block_count);
}

fn check_balance(rpc_client: &Client) {
    let balance = rpc_client.get_balance(None, None).expect("Failed to get balance");
    println!("Current balance: {}", balance);
}

fn send_to_address(rpc_client: &Client, address_string: &str, amount: Amount) {
    let recipient_address = match Address::from_str(address_string) {
        Ok(addr) => addr.assume_checked(),
        Err(e) => {
            eprintln!("Error parsing address {:?}", e);
            return;
        }
    };

    match rpc_client.send_to_address(&recipient_address, amount, None, None, None, None, None, None) {
        Ok(tx_id) => println!("TxID: {}", tx_id),
        Err(e) => println!("Failed to send amount to address {}. Error {:?}", address_string, e)
    }
}

fn generate_blocks_if_required(rpc_client: &Client, do_print: bool) {
    conditional_print!(do_print, "Checking for new transactions");

    match rpc_client.get_raw_mempool() {
        Ok(pending_transactions) => {
            if pending_transactions.len() > 0 {
                conditional_print!(do_print, "Found new transactions, generating block");
                // If there are pending transactions, generate 1 block (bitcoin core should automatically mine the transactions in the mempool)
                let new_address = rpc_client.get_new_address(None, None).unwrap().assume_checked();
                match rpc_client.generate_to_address(1, &new_address) {
                    Ok(_) => conditional_print!(do_print, "Generated and sent new block. Transaction count: {}", pending_transactions.len()),
                    Err(e) => conditional_print!(do_print, "Error generating block {e}")
                }
            } else {
                conditional_print!(do_print, "No new transactions found");
            }
        }
        Err(e) => {
            if !do_print {
                conditional_print!(do_print, "Error getting transactions from mempool {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up RPC authentication
    let rpc_url = "http://127.0.0.1:18443"; // Local node running on port 8332
    let rpc_user = "user";
    let rpc_password = "password";
    let opts = Opts::parse();

    // Initialize the bitcoind RPC client
    let rpc_client = Client::new(rpc_url, Auth::UserPass(rpc_user.to_string(), rpc_password.to_string()))
        .expect("Error creating RPC client");

    match rpc_client.load_wallet(opts.wallet_name.as_str()) {
        Ok(v) => println!("Loaded wallet {}", v.name),
        Err(e) => {
            println!("Failed to load wallet {:?}", e);
        }
    }

    check_block_count(&rpc_client);
    check_balance(&rpc_client);
    println!("-- TYPE COMMANDS --");

    let mut stdin = io::BufReader::new(io::stdin()).lines();
    let sleep = sleep(Duration::from_secs(15));
    tokio::pin!(sleep);

    block_on(async {
        loop {
            select! {
                Ok(Some(line)) = stdin.next_line() => {
                    handle_input_line(&rpc_client, line);
                }

                () = &mut sleep => {
                    // generate_blocks_if_required(&rpc_client, false);
                    sleep.as_mut().reset(Instant::now() + Duration::from_secs(15));
                }
            }
        }
    });
}

// For convenience. All these can be done from the CLI
fn handle_input_line(rpc_client: &Client, line: String) {
    let mut args = line.split(' ');

    match args.next() {
        Some("sendtoaddress") => {
            let address = match args.next() {
                Some(address) => address,
                None => {
                    eprintln!("Bitcoin address required");
                    return;
                }
            };

            let amount = match args.next() {
                Some(amount) => amount,
                None => {
                    eprintln!("Bitcoin address required");
                    return;
                }
            };

            match f64::from_str(amount) {
                Ok(amount_f64) => {
                    let amt = Amount::from_btc(amount_f64).unwrap();
                    send_to_address(rpc_client, address, amt);
                }
                Err(e) => {
                    eprintln!("Error parsing amount {:?}", e);
                }
            }
        }
        Some("mine") => {
            generate_blocks_if_required(rpc_client, true);
        }
        Some("balance") => {
            check_balance(rpc_client);
        }
        Some("blockcount") => {
            check_block_count(rpc_client);
        }
        _ => {
            eprintln!("Invalid command");
        }
    }
}