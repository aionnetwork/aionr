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

use std::collections::{VecDeque, HashMap};
use std::sync::Mutex;

use sync::wrappers::{HeaderWrapper, BlockWrapper};

pub struct SyncStorage {
    /// Downloaded headers wrappers
    pub downloaded_headers: Mutex<VecDeque<HeaderWrapper>>,

    /// Downloaded blocks wrappers
    pub downloaded_blocks: Mutex<VecDeque<BlockWrapper>>,

    /// headers wrappers map for coming bodies
    pub headers_with_bodies_request: Mutex<HashMap<u64, HeaderWrapper>>,
}

impl SyncStorage {
    pub fn new() -> Self {
        SyncStorage {
            downloaded_headers: Mutex::new(VecDeque::new()),
            downloaded_blocks: Mutex::new(VecDeque::new()),
            headers_with_bodies_request: Mutex::new(HashMap::new()),
        }
    }

    pub fn downloaded_headers(&self) -> &Mutex<VecDeque<HeaderWrapper>> { &self.downloaded_headers }

    pub fn _downloaded_blocks(&self) -> &Mutex<VecDeque<BlockWrapper>> { &self.downloaded_blocks }
}
