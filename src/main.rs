use clap::Parser;
use ethers::abi::Tokenizable;
use ethers::contract::Multicall;
use ethers::prelude::*;
use ethers::providers::{Http, Provider, Ws};
use ethers::signers::LocalWallet;
use ethers::types::{Address, U256};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

abigen!(Jolt, "abis/jolt.json");

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    http_rpc_url: String,

    #[clap(short, long)]
    wss_rpc_url: String,

    #[clap(short, long)]
    private_key: String,
}

static JOLT_ADDRESS: Lazy<HashMap<U256, Address>> = Lazy::new(|| {
    [(
        Chain::Goerli.into(),
        Address::from_str("d5192f7DB2c20764aa66336F61f711e3Fe9CC43C").expect("Decoding failed"),
    )]
    .into()
});

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    // Getting JSON RPC provider from HTTP RPC url
    let json_rpc_provider = Provider::<Http>::try_from(args.http_rpc_url)?;

    // Getting websocket provider from WS RPC url
    let ws = Ws::connect(args.wss_rpc_url).await?;
    let websocket_provider = Provider::new(ws);

    // Getting wallet from private key
    let chain_id = json_rpc_provider.get_chainid().await?;
    let wallet = args.private_key.parse::<LocalWallet>()?;
    let wallet = wallet.with_chain_id(chain_id.as_u64());

    // Getting client with provider + wallet
    let client = SignerMiddleware::new(json_rpc_provider.clone(), wallet);
    let client = Arc::new(client);

    // Getting multicall contracts
    let mut multicall = Multicall::new(client.clone(), None).await?;

    // Getting Jolt contract
    let jolt = Jolt::new(
        JOLT_ADDRESS
            .get(&chain_id)
            .expect("Invalid chain id")
            .clone(),
        client.clone(),
    );

    while let Some(block) = websocket_provider.subscribe_blocks().await?.next().await {
        let block_number = U256::from(block.number.unwrap().as_u64());

        let jobs = fetch_jobs(&jolt).await?;
        let workable_jobs: Vec<(&JobInfo, Bytes)> =
            get_workable_jobs(&mut multicall, &jolt, &jobs).await?;

        println!(
            "Workable jobs at block {}: {}/{}",
            block_number,
            workable_jobs.len(),
            jobs.len()
        );

        for (job, bytes) in workable_jobs {
            let contract_call = jolt.work(job.id, bytes.clone());
            let pending_tx: PendingTransaction<Http> = contract_call.send().await?;
            dbg!(pending_tx.tx_hash());
            pending_tx.await?;
            println!("Worked successfully on job with id {}", job.id);
        }
    }
    Ok(())
}

async fn fetch_jobs(
    jolt: &Jolt<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> eyre::Result<Vec<JobInfo>> {
    let jobs_amount = jolt.jobs_amount().call().await?;
    Ok(jolt
        .jobs_slice(U256::from(0 as u32), jobs_amount)
        .call()
        .await?)
}

async fn get_workable_jobs<'a>(
    multicall: &'a mut Multicall<SignerMiddleware<Provider<Http>, LocalWallet>>,
    jolt: &'a Jolt<SignerMiddleware<Provider<Http>, LocalWallet>>,
    jobs: &'a Vec<JobInfo>,
) -> eyre::Result<Vec<(&'a JobInfo, Bytes)>> {
    multicall.clear_calls();
    for job in jobs {
        multicall.add_call(jolt.workable(jolt.client().address(), job.id));
    }
    Ok(multicall
        .call_raw()
        .await?
        .into_iter()
        .enumerate()
        .filter_map(|(i, token)| {
            let (workable, bytes) = <(bool, Bytes)>::from_token(token).unwrap();
            if workable {
                Some((&jobs[i], bytes.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<(&JobInfo, Bytes)>>())
}
