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
use aion_types::H256;
use acore::ids::BlockId;
use byteorder::{BigEndian, ReadBytesExt};
use rlp::RlpStream;

pub struct OnDemandHandler;

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
                let (mut block_num_bytes, addr_hash_bytes) = rest.split_at(8);
                let blk_num = block_num_bytes.read_u64::<BigEndian>().unwrap_or(0);
                if blk_num > SyncStorage::get_synced_block_number() {
                    // TODO: error
                    warn!(target: "on_demand", "Invalid block number!");
                    return;
                }

                let block_id = BlockId::Number(blk_num);
                let addr_hash = H256::from(addr_hash_bytes);

                // TODO: handling start

                let client = SyncStorage::get_block_chain();
                if let Some((proof, _acc)) = client.prove_account(addr_hash, block_id) {
                    let mut rlp = RlpStream::new_list(proof.len());
                    for n in proof {
                        rlp.append(&n);
                    }
                    res.body.put_slice(rlp.as_raw());
                } else {
                    warn!(target:"on_demand","account prove failed!!");
                    return;
                }
                // TODO: handling end
            }
            Kind::UNKNOWN => {
                // TODO： ERROR
                error!(target:"on_demand", "Unknown on_demand request!!")
            }
        }

        res.head.set_length(res.body.len() as u32);
        P2pMgr::send(node.node_hash, res);
    }
}
