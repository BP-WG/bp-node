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

use amplify::confinement::{MediumBlob, TinyBlob, U24 as U24MAX};
use amplify::IoError;
use netservices::Frame;
use strict_encoding::{DecodeError, DeserializeError, StrictDeserialize, StrictSerialize};

use crate::{Failure, Status, BP_RPC_LIB};

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom, dumb = Self::Pong(strict_dumb!()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Response {
    #[strict_type(tag = 0x00)]
    Failure(Failure),

    #[strict_type(tag = 0x01)]
    Pong(TinyBlob),

    #[strict_type(tag = 0x02)]
    Status(Status),
}
impl StrictSerialize for Response {}
impl StrictDeserialize for Response {}

impl TryFrom<Vec<u8>> for Response {
    type Error = DeserializeError;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        let data = MediumBlob::try_from(data).map_err(DecodeError::from)?;
        Response::from_strict_serialized(data)
    }
}

impl From<Response> for Vec<u8> {
    fn from(req: Response) -> Self {
        req.to_strict_serialized::<U24MAX>().expect("request does not fit frame size").into_vec()
    }
}

impl Frame for Response {
    type Error = IoError;

    fn unmarshall(reader: impl Read) -> Result<Option<Self>, Self::Error> { todo!() }

    fn marshall(&self, writer: impl Write) -> Result<usize, Self::Error> { todo!() }
}
