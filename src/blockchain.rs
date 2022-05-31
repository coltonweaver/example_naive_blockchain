use crate::node_service::Block;
use sha256::digest;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
}

impl Default for Blockchain {
    /// Used if this is the first node in the network
    fn default() -> Self {
        Self {
            blocks: vec![Block {
                index: 0,
                timestamp: 0,
                data: "The genesis block.".into(),
                hash: Blockchain::calculate_new_hash(0, "", 0, "The genesis block.".into()),
                prev_hash: "".into(),
            }],
        }
    }
}

impl Blockchain {
    /// Used if this is not the first node in the network
    pub fn new_with_existing_chain(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }

    /// Called when a client begins mining a new block
    pub fn generate_new_block(&mut self, data: String) -> Block {
        let prev_idx = self.blocks.last().expect("Unable to read prev index").index;
        let prev_hash = self
            .blocks
            .last()
            .expect("Unable to read prev hash")
            .hash
            .clone();
        let next_idx = prev_idx + 1;
        let next_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Unable to get current time.")
            .as_millis();
        let next_hash = Blockchain::calculate_new_hash(next_idx, &prev_hash, next_timestamp, &data);

        Block {
            index: next_idx,
            timestamp: next_timestamp as u64,
            data,
            hash: next_hash,
            prev_hash,
        }
    }

    /// Replaces chain in the event that we have a current chain that
    fn replace_chain_if_invalid(&mut self, new_chain: &mut Blockchain) {
        // If the current chain is shorter, it's invalid. This is a naive assumption.
        if self.blocks.len() < new_chain.blocks.len() {
            println!("current chain is invalid, replacing with new chain");
            self.blocks.clear();
            self.blocks.append(&mut new_chain.blocks)
        }
    }

    /// Used to calculate an expected new hash
    fn calculate_new_hash(index: i32, prev_hash: &str, timestamp: u128, data: &str) -> String {
        digest(format!("{}{}{}{}", index, prev_hash, timestamp, data))
    }

    /// Used to validate if a newly received block is valid
    pub fn is_valid_new_block(&self, new_block: &Block) -> bool {
        let old_block = self.blocks.last().expect("Chain cannot be empty");
        if old_block.index + 1 != new_block.index {
            return false;
        }

        if old_block.hash != new_block.prev_hash {
            return false;
        }

        if Blockchain::calculate_new_hash(
            new_block.index,
            &new_block.prev_hash,
            new_block.timestamp.into(),
            &new_block.data,
        ) != new_block.hash
        {
            return false;
        }

        true
    }
}
