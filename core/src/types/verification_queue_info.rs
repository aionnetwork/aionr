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

//! Verification queue info types

/// Verification queue status
#[derive(Debug, Clone)]
pub struct VerificationQueueInfo {
    /// Number of queued items pending verification
    pub unverified_queue_size: usize,
    /// Number of verified queued items pending import
    pub verified_queue_size: usize,
    /// Number of items being verified
    pub verifying_queue_size: usize,
    /// Configured maximum number of items in the queue
    pub max_queue_size: usize,
    /// Configured maximum number of bytes to use
    pub max_mem_use: usize,
    /// Heap memory used in bytes
    pub mem_used: usize,
}

impl VerificationQueueInfo {
    /// The total size of the queues.
    pub fn total_queue_size(&self) -> usize {
        self.unverified_queue_size + self.verified_queue_size + self.verifying_queue_size
    }

    /// The size of the unverified and verifying queues.
    pub fn incomplete_queue_size(&self) -> usize {
        self.unverified_queue_size + self.verifying_queue_size
    }

    /// Indicates that queue is full
    pub fn is_full(&self) -> bool {
        self.unverified_queue_size + self.verified_queue_size + self.verifying_queue_size
            > self.max_queue_size
            || self.mem_used > self.max_mem_use
    }

    /// Indicates that queue is empty
    pub fn is_empty(&self) -> bool {
        self.unverified_queue_size + self.verified_queue_size + self.verifying_queue_size == 0
    }
}
