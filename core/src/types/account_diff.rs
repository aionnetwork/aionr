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

//! Diff between two accounts.

use std::cmp::*;
use std::fmt;
use std::collections::BTreeMap;
use aion_types::{U256, H128, U128};
use acore_bytes::Bytes;

#[derive(Debug, PartialEq, Eq, Clone)]
/// Diff type for specifying a change (or not).
pub enum Diff<T>
where T: Eq
{
    /// Both sides are the same.
    Same,
    /// Left (pre, source) side doesn't include value, right side (post, destination) does.
    Born(T),
    /// Both sides include data; it chaged value between them.
    Changed(T, T),
    /// Left (pre, source) side does include value, right side (post, destination) does not.
    Died(T),
}

impl<T> Diff<T>
where T: Eq
{
    /// Construct new object with given `pre` and `post`.
    pub fn new(pre: T, post: T) -> Self {
        if pre == post {
            Diff::Same
        } else {
            Diff::Changed(pre, post)
        }
    }

    /// Get the before value, if there is one.
    pub fn pre(&self) -> Option<&T> {
        match *self {
            Diff::Died(ref x) | Diff::Changed(ref x, _) => Some(x),
            _ => None,
        }
    }

    /// Get the after value, if there is one.
    pub fn post(&self) -> Option<&T> {
        match *self {
            Diff::Born(ref x) | Diff::Changed(_, ref x) => Some(x),
            _ => None,
        }
    }

    /// Determine whether there was a change or not.
    pub fn is_same(&self) -> bool {
        match *self {
            Diff::Same => true,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// Account diff.
pub struct AccountDiff {
    /// Change in balance, allowed to be `Diff::Same`.
    pub balance: Diff<U256>,
    /// Change in nonce, allowed to be `Diff::Same`.
    pub nonce: Diff<U256>, // Allowed to be Same
    /// Change in code, allowed to be `Diff::Same`.
    pub code: Diff<Bytes>, // Allowed to be Same
    /// Change in storage, values are not allowed to be `Diff::Same`.
    pub storage: BTreeMap<Bytes, Diff<Bytes>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// Change in existance type.
// TODO: include other types of change.
pub enum Existance {
    /// Item came into existance.
    Born,
    /// Item stayed in existance.
    Alive,
    /// Item went out of existance.
    Died,
}

impl fmt::Display for Existance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Existance::Born => write!(f, "+++")?,
            Existance::Alive => write!(f, "***")?,
            Existance::Died => write!(f, "XXX")?,
        }
        Ok(())
    }
}

impl AccountDiff {
    /// Get `Existance` projection.
    pub fn existance(&self) -> Existance {
        match self.balance {
            Diff::Born(_) => Existance::Born,
            Diff::Died(_) => Existance::Died,
            _ => Existance::Alive,
        }
    }
}

// TODO: refactor into something nicer.
fn interpreted_hash(u: &Vec<u8>) -> String {
    if &H128::from(u.as_slice()) <= &H128::from(U128::from(0xffffffffu32)) {
        format!(
            "{} = 0x{:x}",
            U256::from(&**u).low_u32(),
            U256::from(&**u).low_u32()
        )
    } else if &H128::from(u.as_slice()) <= &H128::from(U128::from(u64::max_value())) {
        format!(
            "{} = 0x{:x}",
            U256::from(&**u).low_u64(),
            U256::from(&**u).low_u64()
        )
    } else {
        format!("#{:?}", u)
    }
}

impl fmt::Display for AccountDiff {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use acore_bytes::ToPretty;

        match self.nonce {
            Diff::Born(ref x) => write!(f, "  non {}", x)?,
            Diff::Changed(ref pre, ref post) => {
                write!(
                    f,
                    "#{} ({} {} {})",
                    post,
                    pre,
                    if pre > post { "-" } else { "+" },
                    *max(pre, post) - *min(pre, post)
                )?
            }
            _ => {}
        }
        match self.balance {
            Diff::Born(ref x) => write!(f, "  bal {}", x)?,
            Diff::Changed(ref pre, ref post) => {
                write!(
                    f,
                    "${} ({} {} {})",
                    post,
                    pre,
                    if pre > post { "-" } else { "+" },
                    *max(pre, post) - *min(pre, post)
                )?
            }
            _ => {}
        }
        if let Diff::Born(ref x) = self.code {
            write!(f, "  code {}", x.pretty())?;
        }
        write!(f, "\n")?;
        for (k, dv) in &self.storage {
            match *dv {
                Diff::Born(ref v) => {
                    write!(
                        f,
                        "    +  {} => {}\n",
                        interpreted_hash(k),
                        interpreted_hash(v)
                    )?
                }
                Diff::Changed(ref pre, ref post) => {
                    write!(
                        f,
                        "    *  {} => {} (was {})\n",
                        interpreted_hash(k),
                        interpreted_hash(post),
                        interpreted_hash(pre)
                    )?
                }
                Diff::Died(_) => write!(f, "    X  {}\n", interpreted_hash(k))?,
                _ => {}
            }
        }
        write!(f, "\n")?;
        Ok(())
    }
}
