// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::{Read, Write};

use amplify::Bytes32;
use amplify::confinement::{TinyOrdMap, U24 as U24MAX};
use bpstd::{Block, BlockHash};
use netservices::Frame;
use strict_encoding::{
    DecodeError, Ident, StreamReader, StreamWriter, StrictDecode, StrictDumb, StrictEncode,
    StrictReader, StrictWriter,
};

use crate::BP_RPC_LIB;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExporterPub {
    /// Start session
    #[display("hello({0})")]
    #[strict_type(tag = 0x01)]
    Hello(HelloMsg),

    /// Retrieve bloom filters for known block headers
    #[display("getFilters()")]
    #[strict_type(tag = 0x02, dumb)]
    GetFilters,

    /// Send new block.
    #[display("block(...)")]
    #[strict_type(tag = 0x04)]
    Block(Block),
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display(lowercase)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom, dumb = Self::Filters(strict_dumb!()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImporterReply {
    #[display("filters(...)")]
    #[strict_type(tag = 0x02)]
    Filters(FiltersMsg),
}

impl Frame for ExporterPub {
    type Error = DecodeError;

    fn unmarshall(reader: impl Read) -> Result<Option<Self>, Self::Error> {
        let mut reader = StrictReader::with(StreamReader::new::<U24MAX>(reader));
        match Self::strict_decode(&mut reader) {
            Ok(request) => Ok(Some(request)),
            Err(DecodeError::Io(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn marshall(&self, writer: impl Write) -> Result<(), Self::Error> {
        let writer = StrictWriter::with(StreamWriter::new::<U24MAX>(writer));
        self.strict_encode(writer)?;
        Ok(())
    }
}

impl Frame for ImporterReply {
    type Error = DecodeError;

    fn unmarshall(reader: impl Read) -> Result<Option<Self>, Self::Error> {
        let mut reader = StrictReader::with(StreamReader::new::<U24MAX>(reader));
        match Self::strict_decode(&mut reader) {
            Ok(request) => Ok(Some(request)),
            Err(DecodeError::Io(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn marshall(&self, writer: impl Write) -> Result<(), Self::Error> {
        let writer = StrictWriter::with(StreamWriter::new::<U24MAX>(writer));
        self.strict_encode(writer)?;
        Ok(())
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("{agent} v{version} (features: {features:08x}")]
#[derive(StrictType, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HelloMsg {
    pub agent: Ident,
    pub version: Version,
    pub features: u64,
    pub network: Ident,
    // Backend used by importer (Bitcoin Core etc)
    pub uses: Ident,
}

impl StrictDumb for HelloMsg {
    fn strict_dumb() -> Self {
        Self {
            agent: strict_dumb!(),
            version: strict_dumb!(),
            features: strict_dumb!(),
            network: strict_dumb!(),
            uses: strict_dumb!(),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display, Default)]
#[display("{major}.{minor}.{patch}")]
#[derive(StrictType, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
#[derive(StrictType, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
pub struct FiltersMsg {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub hello: HelloMsg,
    pub height: u32,
    pub timestamp: u32,
    pub block_hash: BlockHash,
    pub bloom_filters: TinyOrdMap<(u32, u32), Bytes32>,
}

impl StrictDumb for FiltersMsg {
    fn strict_dumb() -> Self {
        Self {
            hello: strict_dumb!(),
            height: 0,
            timestamp: 0,
            block_hash: strict_dumb!(),
            bloom_filters: none!(),
        }
    }
}
