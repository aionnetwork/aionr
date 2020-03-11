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

use super::TIMEOUT_MAX;
use super::codec::Codec;

use std::io;
use std::time::Duration;
use std::hash::Hash;
use std::hash::Hasher;
use std::collections::hash_map::DefaultHasher;
use tokio_codec::{Decoder,Framed};
use tokio::prelude::*;
use tokio::net::TcpStream;

pub(crate) fn calculate_hash<T: Hash>(t: T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

/// helper function for setting inbound & outbound stream
pub(crate) fn config_stream(stream: &TcpStream) -> Result<(), io::Error> {
    stream.set_recv_buffer_size(1 << 24)?;
    stream.set_keepalive(Some(Duration::from_secs(TIMEOUT_MAX)))?;

    Ok(())
}

/// helper function for tokio io frame
pub(crate) fn split_frame(
    socket: TcpStream,
) -> (
    stream::SplitSink<Framed<TcpStream, Codec>>,
    stream::SplitStream<Framed<TcpStream, Codec>>,
) {
    Codec.framed(socket).split()
}

pub(crate) fn convert_ip_string(ip_str: String) -> [u8; 8] {
    let mut ip: [u8; 8] = [0u8; 8];
    let ip_vec: Vec<&str> = ip_str.split(".").collect();
    if ip_vec.len() == 4 {
        ip[1] = ip_vec[0].parse::<u8>().unwrap_or(0);
        ip[3] = ip_vec[1].parse::<u8>().unwrap_or(0);
        ip[5] = ip_vec[2].parse::<u8>().unwrap_or(0);
        ip[7] = ip_vec[3].parse::<u8>().unwrap_or(0);
    }
    ip
}
