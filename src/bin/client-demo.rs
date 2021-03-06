extern crate getopts;
extern crate rayon;
extern crate serde_json;
extern crate solana;

use getopts::Options;
use rayon::prelude::*;
use solana::accountant_stub::AccountantStub;
use solana::mint::Mint;
use solana::signature::{KeyPair, KeyPairUtil};
use solana::transaction::Transaction;
use std::env;
use std::io::stdin;
use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn main() {
    let mut threads = 4usize;
    let mut addr: String = "127.0.0.1:8000".to_string();
    let mut send_addr: String = "127.0.0.1:8001".to_string();

    let mut opts = Options::new();
    opts.optopt("s", "", "server address", "host:port");
    opts.optopt("c", "", "client address", "host:port");
    opts.optopt("t", "", "number of threads", "4");
    let args: Vec<String> = env::args().collect();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    if matches.opt_present("s") {
        addr = matches.opt_str("s").unwrap();
    }
    if matches.opt_present("c") {
        send_addr = matches.opt_str("c").unwrap();
    }
    if matches.opt_present("t") {
        threads = matches.opt_str("t").unwrap().parse().expect("integer");
    }
    let mint: Mint = serde_json::from_reader(stdin()).unwrap();
    let mint_keypair = mint.keypair();
    let mint_pubkey = mint.pubkey();

    let socket = UdpSocket::bind(&send_addr).unwrap();
    println!("Stub new");
    let acc = AccountantStub::new(&addr, socket);
    println!("Get last id");
    let last_id = acc.get_last_id().unwrap();

    println!("Get Balance");
    let mint_balance = acc.get_balance(&mint_pubkey).unwrap().unwrap();
    println!("Mint's Initial Balance {}", mint_balance);

    println!("Signing transactions...");
    let txs = 1_000_000;
    let now = Instant::now();
    let transactions: Vec<_> = (0..txs)
        .into_par_iter()
        .map(|_| {
            let rando_pubkey = KeyPair::new().pubkey();
            Transaction::new(&mint_keypair, rando_pubkey, 1, last_id)
        })
        .collect();
    let duration = now.elapsed();
    let ns = duration.as_secs() * 2_000_000_000 + u64::from(duration.subsec_nanos());
    let bsps = f64::from(txs) / ns as f64;
    let nsps = ns as f64 / f64::from(txs);
    println!(
        "Done. {} thousand signatures per second, {}us per signature",
        bsps * 1_000_000_f64,
        nsps / 1_000_f64
    );

    println!("Transfering {} transactions in {} batches", txs, threads);
    let now = Instant::now();
    let sz = transactions.len() / threads;
    let chunks: Vec<_> = transactions.chunks(sz).collect();
    let _: Vec<_> = chunks
        .into_par_iter()
        .map(|trs| {
            println!("Transferring 1 unit {} times...", trs.len());
            let send_addr = "0.0.0.0:0";
            let socket = UdpSocket::bind(send_addr).unwrap();
            let acc = AccountantStub::new(&addr, socket);
            for tr in trs {
                acc.transfer_signed(tr.clone()).unwrap();
            }
            ()
        })
        .collect();
    println!("Waiting for last transaction to be confirmed...",);
    let mut val = mint_balance;
    let mut prev = 0;
    while val != prev {
        sleep(Duration::from_millis(20));
        prev = val;
        val = acc.get_balance(&mint_pubkey).unwrap().unwrap();
    }
    println!("Mint's Final Balance {}", val);
    let txs = mint_balance - val;
    println!("Successful transactions {}", txs);

    let duration = now.elapsed();
    let ns = duration.as_secs() * 1_000_000_000 + u64::from(duration.subsec_nanos());
    let tps = (txs * 1_000_000_000) as f64 / ns as f64;
    println!("Done. {} tps!", tps);
}
