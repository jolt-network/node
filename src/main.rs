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

abigen!(Master, "abis/master.json");

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

static MASTER_ADDRESS: Lazy<HashMap<U256, Address>> = Lazy::new(|| {
    [(
        Chain::Rinkeby.into(),
        Address::from_str("85B931f4fEF1bCb00a550862d6c7890b4652f04E").expect("Decoding failed"),
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

    // Getting master contract
    let master = Master::new(
        MASTER_ADDRESS
            .get(&chain_id)
            .expect("Invalid chain id")
            .clone(),
        client.clone(),
    );

    while let Some(block) = websocket_provider.subscribe_blocks().await?.next().await {
        let block_number = U256::from(block.number.unwrap().as_u64());

        let jobs = fetch_jobs(&master).await?;
        let workable_jobs: Vec<(&JobInfo, Bytes)> =
            get_workable_jobs(&mut multicall, &master, &jobs).await?;

        println!(
            "Workable jobs at block {}: {}",
            block_number,
            workable_jobs.len()
        );

        for (job, bytes) in workable_jobs {
            let contract_call = master.work(job.id, bytes.clone());
            let pending_tx: PendingTransaction<Http> = contract_call.send().await?;
            dbg!(pending_tx.tx_hash());
            pending_tx.await?;
            println!("Worked successfully on job with id {}", job.id);
        }
    }
    Ok(())
}

async fn fetch_jobs(
    master: &Master<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> eyre::Result<Vec<JobInfo>> {
    let jobs_amount = master.jobs_amount().call().await?;
    Ok(master
        .jobs_slice(U256::from(0 as u32), jobs_amount)
        .call()
        .await?)
}

async fn get_workable_jobs<'a>(
    multicall: &'a mut Multicall<SignerMiddleware<Provider<Http>, LocalWallet>>,
    master: &'a Master<SignerMiddleware<Provider<Http>, LocalWallet>>,
    jobs: &'a Vec<JobInfo>,
) -> eyre::Result<Vec<(&'a JobInfo, Bytes)>> {
    multicall.clear_calls();
    for job in jobs {
        multicall.add_call(master.workable(master.client().address(), job.id));
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
