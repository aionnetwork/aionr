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

extern crate p2p;
extern crate tokio;

use std::time::Duration;
use std::thread;
use p2p::Mgr;
use p2p::Config;

#[test]
fn test_multi_id_same_ip() {

    let runtime_sync = tokio::runtime::Builder::new()
        .name_prefix("p2p-loop #")
        .build()
        .expect("p2p runtime loop init failed");
    let executor_p2p = runtime_sync.executor();

    let c_0 = Config::new();       // p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303
    let mut p2p_0: Mgr = Mgr::new(c_0, vec![]);

    let mut c_1 = Config::new();
    c_1.boot_nodes.push(String::from("p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303"));
    c_1.local_node = String::from("p2p://11111111-1111-1111-1111-111111111111@0.0.0.0:30304");
    let mut p2p_1: Mgr = Mgr::new(c_1, vec![]);

    let mut c_2 = Config::new();
    c_2.boot_nodes.push(String::from("p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303"));
    c_2.local_node = String::from("p2p://22222222-2222-2222-2222-222222222222@0.0.0.0:30305");
    let mut p2p_2: Mgr = Mgr::new(c_2, vec![]);

    p2p_0.run(executor_p2p.clone());
    p2p_1.run(executor_p2p.clone());
    p2p_2.run(executor_p2p.clone());

    thread::sleep(Duration::from_secs(1));
    assert_eq(2, p2p_0.get_active_nodes().len());

    p2p_0.shutdown();
    p2p_1.shutdown();
    p2p_2.shutdown();
}
