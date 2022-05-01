mod blockchain;

use node_service::node_api_client::NodeApiClient;
use node_service::node_api_server::{NodeApi, NodeApiServer};
use node_service::{
    AddBlockRequest, AddBlockResponse, Block, GetBlockchainRequest, GetBlockchainResponse,
    MineBlockRequest, MineBlockResponse, RegisterNodeRequest, RegisterNodeResponse,
};
use parking_lot::FairMutex;
use structopt::StructOpt;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

use crate::blockchain::Blockchain;

pub mod node_service {
    tonic::include_proto!("node_service");
}

#[derive(Debug, Default)]
pub struct NodeServer {
    blockchain: FairMutex<Blockchain>,
    peers: FairMutex<Vec<NodeApiClient<Channel>>>,
}

#[tonic::async_trait]
impl NodeApi for NodeServer {
    async fn add_block(
        &self,
        request: Request<AddBlockRequest>,
    ) -> Result<Response<AddBlockResponse>, Status> {
        println!("Received request: {:?}", request);

        let new_block = request.into_inner().block.expect("Block is expected");
        let mut chain = self.blockchain.lock();
        let is_valid = chain.is_valid_new_block(&new_block);
        if is_valid {
            chain.blocks.push(new_block);
        }

        let reply = AddBlockResponse { added: is_valid };

        Ok(Response::new(reply))
    }

    async fn get_blockchain(
        &self,
        request: Request<GetBlockchainRequest>,
    ) -> Result<Response<GetBlockchainResponse>, Status> {
        println!("Received request: {:?}", request);

        let reply = GetBlockchainResponse {
            blocks: self.blockchain.lock().blocks.clone(),
        };

        Ok(Response::new(reply))
    }

    async fn mine_block(
        &self,
        request: Request<MineBlockRequest>,
    ) -> Result<Response<MineBlockResponse>, Status> {
        println!("Received request: {:?}", request);

        let mut chain = self.blockchain.lock();
        let block = chain.generate_new_block(request.into_inner().data);
        chain.blocks.push(block.clone());
        let peers = self.peers.lock().clone();

        let reply = MineBlockResponse {
            block: Some(block.clone()),
        };

        tokio::spawn(async move { NodeServer::broadcast_block_to_peers(block, peers).await });

        Ok(Response::new(reply))
    }

    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        println!("Received request: {:?}", request);
        let client = NodeApiClient::connect(format!(
            "http://127.0.0.1:{}",
            request.into_inner().peer_port
        ))
        .await
        .unwrap();
        let mut peers = self.peers.lock();
        peers.push(client);

        Ok(Response::new(RegisterNodeResponse {}))
    }
}

impl NodeServer {
    async fn broadcast_block_to_peers(block: Block, peers: Vec<NodeApiClient<Channel>>) {
        for mut peer in peers {
            println!("Broadcasting {:?} to {:?}", block, peer);
            let _ = peer
                .add_block(Request::new(AddBlockRequest {
                    block: Some(block.clone()),
                }))
                .await;
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(short = "p", long = "port", default_value = "5150")]
    port: String,

    #[structopt(long = "peer_port", default_value = "")]
    peer_port: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let port_no = opt.port;
    let peer_port = opt.peer_port;

    let node_server = if !peer_port.is_empty() {
        let mut client = NodeApiClient::connect(format!("http://127.0.0.1:{}", peer_port)).await?;
        let get_blockchain_response = client
            .get_blockchain(Request::new(GetBlockchainRequest {}))
            .await?
            .into_inner();
        NodeServer {
            blockchain: FairMutex::new(Blockchain::new(get_blockchain_response.blocks)),
            peers: FairMutex::new(vec![client]),
        }
    } else {
        NodeServer::default()
    };

    let addr = format!("127.0.0.1:{}", port_no).parse()?;
    Server::builder()
        .add_service(NodeApiServer::new(node_server))
        .serve(addr)
        .await?;

    Ok(())
}
