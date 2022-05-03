#! /bin/bash
echo "Starting 20 nodes locally."
for i in {0..20}
do
    tmux new -d "cargo run --bin node-server"
    sleep .5
done;

echo "Sleeping five seconds to ensure nodes register with one another."
sleep 5

echo "Current chain looks like this:"
grpcurl -plaintext -import-path ./proto -proto node_service.proto -d '{}' 127.0.0.1:8000 node_service.NodeApi/GetBlockchain

sleep 5

echo "Mining 20 blocks on each node."
for i in {0..20}
do
    for port in {8000..8019}
    do
        grpcurl -plaintext -import-path ./proto -proto node_service.proto -d '{"data":"test_string"}' 127.0.0.1:${port} node_service.NodeApi/MineBlock
        sleep .1
    done;
done;

echo "The final chain looks like this:"
grpcurl -plaintext -import-path ./proto -proto node_service.proto -d '{}' 127.0.0.1:8000 node_service.NodeApi/GetBlockchain

echo "Tearing down all running nodes."
tmux kill-server
