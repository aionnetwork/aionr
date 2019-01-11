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

use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::collections::BTreeMap;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Error, Visitor, MapAccess};
use serde::de::value::MapAccessDeserializer;
use serde_json::Value;
use serde_json::de::from_reader;
use serde_json::ser::to_string;
use journaldb::Algorithm;

pub struct UserDefaults {
    pub is_first_launch: bool,
    pub pruning: Algorithm,
    pub fat_db: bool,
}

impl Serialize for UserDefaults {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut map: BTreeMap<String, Value> = BTreeMap::new();
        map.insert("is_first_launch".into(), Value::Bool(self.is_first_launch));
        map.insert(
            "pruning".into(),
            Value::String(self.pruning.as_str().into()),
        );
        map.insert("fat_db".into(), Value::Bool(self.fat_db));

        map.serialize(serializer)
    }
}

struct UserDefaultsVisitor;

impl<'a> Deserialize<'a> for UserDefaults {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'a> {
        deserializer.deserialize_any(UserDefaultsVisitor)
    }
}

impl<'a> Visitor<'a> for UserDefaultsVisitor {
    type Value = UserDefaults;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a valid UserDefaults object")
    }

    fn visit_map<V>(self, visitor: V) -> Result<Self::Value, V::Error>
    where V: MapAccess<'a> {
        let mut map: BTreeMap<String, Value> =
            Deserialize::deserialize(MapAccessDeserializer::new(visitor))?;
        let pruning: Value = map
            .remove("pruning")
            .ok_or_else(|| Error::custom("missing pruning"))?;
        let pruning = pruning
            .as_str()
            .ok_or_else(|| Error::custom("invalid pruning value"))?;
        let pruning = pruning
            .parse()
            .map_err(|_| Error::custom("invalid pruning method"))?;
        let fat_db: Value = map.remove("fat_db").unwrap_or_else(|| Value::Bool(false));
        let fat_db = fat_db
            .as_bool()
            .ok_or_else(|| Error::custom("invalid fat_db value"))?;

        let user_defaults = UserDefaults {
            is_first_launch: false,
            pruning: pruning,
            fat_db: fat_db,
        };

        Ok(user_defaults)
    }
}

impl Default for UserDefaults {
    fn default() -> Self {
        UserDefaults {
            is_first_launch: true,
            pruning: Algorithm::default(),
            fat_db: false,
        }
    }
}

impl UserDefaults {
    pub fn load<P>(path: P) -> Result<Self, String>
    where P: AsRef<Path> {
        match File::open(path) {
            Ok(file) => {
                match from_reader(file) {
                    Ok(defaults) => Ok(defaults),
                    Err(e) => {
                        warn!(target:"run","Error loading user defaults file: {:?}", e);
                        Ok(UserDefaults::default())
                    }
                }
            }
            _ => Ok(UserDefaults::default()),
        }
    }

    pub fn save<P>(&self, path: P) -> Result<(), String>
    where P: AsRef<Path> {
        let mut file: File =
            File::create(path).map_err(|_| "Cannot create user defaults file".to_owned())?;
        file.write_all(
            to_string(&self)
                .map_err(|_| format!("User default can't parse into string"))?
                .as_bytes(),
        )
        .map_err(|_| "Failed to save user defaults".to_owned())
    }
}
