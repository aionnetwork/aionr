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

//! Gas prices histogram.

use aion_types::U256;

/// Values of RPC settings.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Histogram {
    /// Gas prices for bucket edges.
    #[serde(rename = "bucketBounds")]
    pub bucket_bounds: Vec<U256>,
    /// Transacion counts for each bucket.
    pub counts: Vec<usize>,
}

impl From<::stats::Histogram<::aion_types::U256>> for Histogram {
    fn from(h: ::stats::Histogram<::aion_types::U256>) -> Self {
        Histogram {
            bucket_bounds: h.bucket_bounds,
            counts: h.counts,
        }
    }
}
