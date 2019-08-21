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

use std::io;
use tokio_codec::Decoder;
use tokio_codec::Encoder;
use acore_bytes::to_hex;
use bincode::config;
use bytes::BytesMut;
use node::HEADER_LENGTH;
use msg::ChannelBuffer;
use route::VERSION;

pub struct Codec;

impl Encoder for Codec {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn encode(&mut self, item: ChannelBuffer, dst: &mut BytesMut) -> io::Result<()> {
        let mut encoder = config();
        let encoder = encoder.big_endian();
        if let Ok(encoded) = encoder.serialize(&item.head) {
            dst.extend_from_slice(encoded.as_slice());
            dst.extend_from_slice(item.body.as_slice());
        }

        Ok(())
    }
}

impl Decoder for Codec {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<ChannelBuffer>> {
        let len = src.len();
        if len >= HEADER_LENGTH {
            let mut decoder = config();
            let decoder = decoder.big_endian();
            let mut invalid = false;
            let mut decoded = ChannelBuffer::new();
            {
                let (head_raw, _) = src.split_at(HEADER_LENGTH);
                if let Ok(head) = decoder.deserialize(head_raw) {
                    decoded.head = head;
                    if decoded.head.ver > VERSION::V2.value() || decoded.head.ctrl > 1
                    //TODO: FIX IT
                    {
                        invalid = true;
                    } else if decoded.head.len as usize + HEADER_LENGTH > len {
                        return Ok(None);
                    }
                }
            }

            if invalid {
                src.split_to(len);
                Ok(None)
            } else {
                let buf = src.split_to(decoded.head.len as usize + HEADER_LENGTH);
                let (_, body) = buf.split_at(HEADER_LENGTH);
                decoded.body.extend_from_slice(body);
                Ok(Some(decoded))
            }
        } else {
            if len > 0 {
                debug!(target: "p2p", "len = {}, {}", len, to_hex(src));
            }
            Ok(None)
        }
    }
}
