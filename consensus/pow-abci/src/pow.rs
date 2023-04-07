use anyhow::Result;
use futures::future::Select;
use std::ops::Shl;
use bigint::U256;
use crate::{utils::{serialize, hash_to_u8, hash_to_str}, Transaction};

const MAX_NONCE: usize = usize::MAX;

pub struct ProofOfWork {
    target: U256,
}

impl ProofOfWork {
    pub fn new(bits: usize) -> Self {
        let mut target = U256::from(1 as usize);
        target = target.shl(256 - bits);

        Self {
            target
        }
    }

    pub fn run(&self, trans: &Transaction) -> (usize, String){
        let mut nonce = 0;
        let mut hash: String = String::from("");
        while nonce < MAX_NONCE {
            if let pre_hash = Self::prepare_data(trans, nonce) {
                let mut hash_u: [u8; 32] = [0; 32];
                hash_to_u8(&pre_hash,&mut hash_u);
                let pre_hash_int = U256::from(hash_u);

                if pre_hash_int.lt(&(self.target)) {
                    //TODO: this should be changed
                    hash = hash_to_str(&pre_hash);
                    break;
                }else {
                    nonce += 1;
                }
            }
        }
        println!("本次工作量证明共计算了{:?}次哈希", nonce);
        (nonce, hash)
    }

    fn prepare_data(tx: &Transaction, nonce: usize) -> Vec<u8> {
        let pre_data = (tx, nonce);
        serialize(&pre_data).unwrap()
    }
}