/*******************************************************************************
 * Copyright (c) 2018-2019 Aion foundation.
 *
 *     This file is part of the aion network project.
 *
 *     The aion network project is free software: you can redistribute it
 *     and/or modify it under the terms of the GNU General Public License
 *     as published by the Free Software Foundation, either version 3 of
 *     the License, or any later version.
 *
 *     The aion network project is distributed in the hope that it will
 *     be useful, but WITHOUT ANY WARRANTY; without even the implied
 *     warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 *     See the GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with the aion network project source files.
 *     If not, see <https://www.gnu.org/licenses/>.
 *
 ******************************************************************************/

use bytes::BufMut;

use super::super::action::LightAction;
//use super::super::event::LightEvent;
use super::super::kind::Kind;
use p2p::*;
use sync::storage::SyncStorage;
use aion_types::{H256,H128};
use acore::ids::BlockId;
use byteorder::{BigEndian, ReadBytesExt};
use rlp::RlpStream;

pub struct OnDemandHandler;

const MAX_HEADERS_PER_REQUEST: u64 = 512;
const LENGTH_OF_U64: usize = 8;
const LENGTH_OF_H256: usize = 32;

impl OnDemandHandler {
    /// req handle for normal mod
    pub fn handle_on_demand_req(node: &mut Node, req: ChannelBuffer) {
        trace!(target: "net", "PING received.");

        if req.body.len() < 1 {
            warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand req length!!", node.get_node_id(), node.get_ip_addr());
            P2pMgr::remove_peer(node.node_hash);
            return;
        }
        let mut res = ChannelBuffer::new();
        res.head.ver = Version::V0.value();
        res.head.ctrl = Control::LIGHT.value();
        res.head.action = LightAction::ONDEMANDRES.value();

        let (kind, rest) = req.body.split_at(1);
        res.body.push(kind[0]);
        match kind[0].into() {
            Kind::Account => {
                if req.head.len != 41 {
                    // u64 + H256 + 1
                    warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand account req length!!", node.get_node_id(), node.get_ip_addr());
                    P2pMgr::remove_peer(node.node_hash);
                    return;
                }

                let client = SyncStorage::get_block_chain();

                let (mut block_num_bytes, addr_hash_bytes) = rest.split_at(LENGTH_OF_U64);
                let blk_num = block_num_bytes.read_u64::<BigEndian>().unwrap_or(0);

                if blk_num <= client.chain_info().best_block_number {
                    let block_id = BlockId::Number(blk_num);
                    let addr_hash = H256::from(addr_hash_bytes);

                    if let Some((proof, _acc)) = client.prove_account(addr_hash, block_id) {
                        let mut rlp = RlpStream::new_list(proof.len());
                        for n in proof {
                            rlp.append(&n);
                        }
                        res.body.put_slice(rlp.as_raw());
                    } else {
                        warn!(target: "on_demand", "account prove failed!!");
                    }
                } else {
                    warn!(target: "on_demand", "Invalid block number!");
                }
            }
            Kind::Header => {
                if req.head.len != 26 {
                    // u64 * 3 + bool + 1
                    warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand header req length!!", node.get_node_id(), node.get_ip_addr());
                    P2pMgr::remove_peer(node.node_hash);
                    return;
                }

                let client = SyncStorage::get_block_chain();

                let (mut start_num_bytes, rest) = rest.split_at(LENGTH_OF_U64);
                let start_num = start_num_bytes.read_u64::<BigEndian>().unwrap_or(0);
                let (mut skip_bytes, rest) = rest.split_at(LENGTH_OF_U64);
                let skip = skip_bytes.read_u64::<BigEndian>().unwrap_or(0);
                let (mut max_bytes, reverse) = rest.split_at(LENGTH_OF_U64);
                let max = max_bytes.read_u64::<BigEndian>().unwrap_or(0);
                let max = ::std::cmp::min(MAX_HEADERS_PER_REQUEST, max);
                let reverse = reverse[0];

                if max != 0 {
                    let best_num = client.chain_info().best_block_number;

                    let headers: Vec<_> = (0u64..max)
                        .map(|x: u64| x.saturating_mul(skip.saturating_add(1)))
                        .take_while(|&x| {
                            if reverse != 0 {
                                x < start_num
                            } else {
                                best_num.saturating_sub(start_num) >= x
                            }
                        })
                        .map(|x| {
                            if reverse != 0 {
                                start_num.saturating_sub(x)
                            } else {
                                start_num.saturating_add(x)
                            }
                        })
                        .map(|x| client.block_header(BlockId::Number(x)))
                        .take_while(|x| x.is_some())
                        .flat_map(|x| x)
                        .map(|x| x.into_inner())
                        .collect();
                    let data: Vec<_> = headers.iter().flat_map(|x| x.iter()).cloned().collect();

                    if !data.is_empty() {
                        let mut rlp = RlpStream::new_list(data.len());

                        rlp.append_raw(data.as_slice(), data.len());
                        res.body.put_slice(rlp.as_raw());
                    }
                    {
                        warn!(target: "on_demand", "get body failed!!");
                    }
                }
            }
            Kind::Body => {
                if req.head.len != 33 {
                    // H256 + 1
                    warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand body req length!!", node.get_node_id(), node.get_ip_addr());
                    P2pMgr::remove_peer(node.node_hash);
                    return;
                }
                let block_hash = H256::from(rest);

                let client = SyncStorage::get_block_chain();

                if let Some(body) = client.block_body(BlockId::Hash(block_hash)) {
                    res.body.put_slice(&body.into_inner());
                } else {
                    warn!(target: "on_demand", "get body failed!!");
                }
            }
            Kind::Storage => {
                if req.head.len != 81 {
                    // H256 * 2 + H128 + 1
                    warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand storage req length!!", node.get_node_id(), node.get_ip_addr());
                    P2pMgr::remove_peer(node.node_hash);
                    return;
                }

                let (block_hash_bytes, rest) = rest.split_at(LENGTH_OF_H256);
                let (addr_hash_bytes, key_hash_bytes) = rest.split_at(LENGTH_OF_H256);
                let block_hash = H256::from(block_hash_bytes);
                let addr_hash = H256::from(addr_hash_bytes);
                let key_hash = H128::from(key_hash_bytes);

                let client = SyncStorage::get_block_chain();

                if let Some((proof, _value)) =
                    client.prove_storage(addr_hash, key_hash, BlockId::Hash(block_hash))
                {
                    let mut rlp = RlpStream::new_list(proof.len());
                    for n in proof {
                        rlp.append(&n);
                    }
                    res.body.put_slice(rlp.as_raw());
                } else {
                    warn!(target:"on_demand","storage prove failed!!");
                }
            }
            Kind::Code => {
                if req.head.len != 33 {
                    // H256 + 1
                    warn!(target: "on_demand", "Node {}@{} removed: Invalid on demand code req length!!", node.get_node_id(), node.get_ip_addr());
                    P2pMgr::remove_peer(node.node_hash);
                    return;
                }

                let code_hash = H256::from(rest);

                let client = SyncStorage::get_block_chain();

                if let Some(code) = client.state_data(&code_hash) {
                    res.body.put_slice(code.as_slice());
                } else {
                    warn!(target: "on_demand", "get code failed!!");
                }
            }
            Kind::Unknown => error!(target:"on_demand", "Unknown on_demand request!!"),
        }

        res.head.set_length(res.body.len() as u32);
        P2pMgr::send(node.node_hash, res);
    }
}
