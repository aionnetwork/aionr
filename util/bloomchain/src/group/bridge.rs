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

use bloom::Bloom;
use crate::config::Config;
use crate::database::BloomDatabase;
use crate::position::Position;
use crate::group::position::Manager as PositionManager;
use super::BloomGroupDatabase;

/// Bridge between `BloomDatabase` and `BloomGroupDatabase`.
pub struct GroupDatabaseBridge<'a> {
    positioner: PositionManager,
    db: &'a dyn BloomGroupDatabase,
}

impl<'a> GroupDatabaseBridge<'a> {
    pub fn new(config: Config, db: &'a dyn BloomGroupDatabase) -> Self {
        let positioner = PositionManager::new(config.elements_per_index);

        GroupDatabaseBridge {
            positioner: positioner,
            db: db,
        }
    }
}

impl<'a> BloomDatabase for GroupDatabaseBridge<'a> {
    fn bloom_at(&self, position: &Position) -> Option<Bloom> {
        let position = self.positioner.position(position);
        self.db
            .blooms_at(&position.group)
            .and_then(|group| group.blooms.into_iter().nth(position.number))
    }
}
