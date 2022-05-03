mod blockchain;

use std::net::TcpListener;
use std::time::Duration;

use node_service::node_api_client::NodeApiClient;
use node_service::node_api_server::{NodeApi, NodeApiServer};
use node_service::{
    AddBlockRequest, AddBlockResponse, Block, GetBlockchainRequest, GetBlockchainResponse,
    MineBlockRequest, MineBlockResponse, RegisterNodeRequest, RegisterNodeResponse,
};
use parking_lot::FairMutex;
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
    async fn new(mut peer_clients: Vec<NodeApiClient<Channel>>) -> Self {
        let get_blockchain_response = peer_clients[0]
            .get_blockchain(Request::new(GetBlockchainRequest {}))
            .await
            .expect("Couldn't retrieve blockchain!")
            .into_inner();

        Self {
            blockchain: FairMutex::new(Blockchain::new(get_blockchain_response.blocks)),
            peers: FairMutex::new(peer_clients),
        }
    }

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

    async fn register_node_with_peers(
        available_port: u16,
        peer_clients: Vec<NodeApiClient<Channel>>,
    ) {
        for mut client in peer_clients {
            let _ = client
                .register_node(Request::new(RegisterNodeRequest {
                    peer_port: available_port.to_string(),
                }))
                .await
                .expect("Couldn't register node with peer.")
                .into_inner();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut peer_ports = Vec::new();
    let available_port = (8000..9000)
        .find(|port| match TcpListener::bind(("127.0.0.1", *port)) {
            Ok(_) => true,
            Err(_) => {
                peer_ports.push(*port);
                false
            }
        })
        .expect("Unable to find available port!");

    let mut peer_clients = Vec::new();
    for peer_port in peer_ports {
        let client = NodeApiClient::connect(format!("http://127.0.0.1:{}", peer_port))
            .await
            .expect("Couldn't connect to client!");
        peer_clients.push(client);
    }

    let node_server = if peer_clients.len() > 0 {
        NodeServer::new(peer_clients.clone()).await
    } else {
        NodeServer::default()
    };

    let addr = format!("127.0.0.1:{}", available_port).parse()?;
    let future = Server::builder()
        .add_service(NodeApiServer::new(node_server))
        .serve(addr);

    println!(
        "Node running on 127.0.0.1:{} with {} peers.",
        available_port,
        peer_clients.len()
    );

    if peer_clients.len() > 0 {
        tokio::spawn(async move {
            println!("Delaying registration of node with peers for 5 seconds while servers starts.");
            tokio::time::sleep(Duration::from_secs(5)).await;
            NodeServer::register_node_with_peers(available_port, peer_clients.clone()).await;
        });
    }

    future.await?;

    Ok(())
}
