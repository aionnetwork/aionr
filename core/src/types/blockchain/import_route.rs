/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

//! Import route.

use aion_types::H256;
use types::block::info::{BlockInfo, BlockLocation};

/// Import route for newly inserted block.
#[derive(Debug, PartialEq)]
pub struct ImportRoute {
    /// Blocks that were invalidated by new block.
    pub retracted: Vec<H256>,
    /// Blocks that were validated by new block.
    pub enacted: Vec<H256>,
    /// Blocks which are neither retracted nor enacted.
    pub omitted: Vec<H256>,
}

impl ImportRoute {
    pub fn none() -> Self {
        ImportRoute {
            retracted: vec![],
            enacted: vec![],
            omitted: vec![],
        }
    }
}

impl From<BlockInfo> for ImportRoute {
    fn from(info: BlockInfo) -> ImportRoute {
        match info.location {
            BlockLocation::CanonChain => {
                ImportRoute {
                    retracted: vec![],
                    enacted: vec![info.hash],
                    omitted: vec![],
                }
            }
            BlockLocation::Branch => {
                ImportRoute {
                    retracted: vec![],
                    enacted: vec![],
                    omitted: vec![info.hash],
                }
            }
            BlockLocation::BranchBecomingCanonChain(mut data) => {
                data.enacted.push(info.hash);
                ImportRoute {
                    retracted: data.retracted,
                    enacted: data.enacted,
                    omitted: vec![],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use aion_types::{H256, U256};
    use types::block::info::{BlockInfo, BlockLocation, BranchBecomingCanonChainData};
    use types::blockchain::import_route::ImportRoute;

    #[test]
    fn import_route_none() {
        assert_eq!(
            ImportRoute::none(),
            ImportRoute {
                enacted: vec![],
                retracted: vec![],
                omitted: vec![],
            }
        );
    }

    #[test]
    fn import_route_branch() {
        let info = BlockInfo {
            hash: H256::from(U256::from(1)),
            number: 0,
            total_difficulty: U256::from(0),
            location: BlockLocation::Branch,
        };

        assert_eq!(
            ImportRoute::from(info),
            ImportRoute {
                retracted: vec![],
                enacted: vec![],
                omitted: vec![H256::from(U256::from(1))],
            }
        );
    }

    #[test]
    fn import_route_canon_chain() {
        let info = BlockInfo {
            hash: H256::from(U256::from(1)),
            number: 0,
            total_difficulty: U256::from(0),
            location: BlockLocation::CanonChain,
        };

        assert_eq!(
            ImportRoute::from(info),
            ImportRoute {
                retracted: vec![],
                enacted: vec![H256::from(U256::from(1))],
                omitted: vec![],
            }
        );
    }

    #[test]
    fn import_route_branch_becoming_canon_chain() {
        let info = BlockInfo {
            hash: H256::from(U256::from(2)),
            number: 0,
            total_difficulty: U256::from(0),
            location: BlockLocation::BranchBecomingCanonChain(BranchBecomingCanonChainData {
                ancestor: H256::from(U256::from(0)),
                enacted: vec![H256::from(U256::from(1))],
                retracted: vec![H256::from(U256::from(3)), H256::from(U256::from(4))],
            }),
        };

        assert_eq!(
            ImportRoute::from(info),
            ImportRoute {
                retracted: vec![H256::from(U256::from(3)), H256::from(U256::from(4))],
                enacted: vec![H256::from(U256::from(1)), H256::from(U256::from(2))],
                omitted: vec![],
            }
        );
    }
}
