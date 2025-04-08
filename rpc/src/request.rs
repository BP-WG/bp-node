// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
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

use amplify::confinement::{TinyBlob, U24 as U24MAX};
use netservices::Frame;
use strict_encoding::{
    DecodeError, StreamReader, StreamWriter, StrictDecode, StrictEncode, StrictReader, StrictWriter,
};

use crate::BP_RPC_LIB;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display(lowercase)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Request {
    #[display("ping(...)")]
    #[strict_type(tag = 0x00)]
    Ping(TinyBlob),

    #[strict_type(tag = 0x01, dumb)]
    Noop,

    #[strict_type(tag = 0x02)]
    Status,
}

impl Frame for Request {
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
