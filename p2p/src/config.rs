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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub boot_nodes: Vec<String>,
    pub max_peers: u32,
    pub net_id: u32,
    pub local_node: String,
    pub sync_from_boot_nodes_only: bool,
    pub ip_black_list: Vec<String>,
}

impl Config {
    pub fn new() -> Self {



        Config {
            boot_nodes: Vec::new(),
            max_peers: 64,
            net_id: 0,
            local_node: String::from("p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303"),
            sync_from_boot_nodes_only: false,
            ip_black_list: Vec::new(),
        }
    }

    /// get id & binding
    pub fn get_id_and_binding(&self) -> (String, String) {
        let local = &self.local_node.clone().replace("\"", "");
        let (_, node_str) = local.split_at(6);
        let (id_str, binding_str) = node_str.split_at(36);
        (
            String::from(id_str),
            String::from(binding_str.replace("@", "")),
        )
    }

    /// get ip and port
    pub fn get_ip_and_port(&self) -> (String, u32) {
        let (id, binding) = &self.get_id_and_binding();
        let frags: Vec<&str> = binding.split(":").collect::<Vec<&str>>();
        (
            String::from(frags[0]),
            String::from(frags[1]).parse::<u32>().unwrap(),
        )
    }
}
