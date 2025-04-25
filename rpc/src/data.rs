// BP Node: sovereign bitcoin wallet backend.
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed & written in 2020-2025 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2025 LNP/BP Labs, InDCS, Switzerland. All rights reserved.
// Copyright (C) 2020-2025 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use amplify::confinement::{SmallVec, TinyOrdMap, TinyString};
use strict_encoding::{Ident, StrictDumb};

use crate::BP_RPC_LIB;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display(doc_comments)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = repr, into_u8, try_from_u8)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
#[repr(u8)]
pub enum FailureCode {
    /// Network mismatch
    #[strict_type(dumb)]
    NetworkMismatch = 1,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("code={code}, message={message}, details={details:?}")]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Failure {
    pub code: u8,
    pub message: TinyString,
    pub details: TinyOrdMap<TinyString, TinyString>,
}

impl Failure {
    pub fn new(code: FailureCode) -> Self {
        Self {
            code: code.into(),
            message: TinyString::from_checked(code.to_string()),
            details: Default::default(),
        }
    }

    pub fn network_mismatch() -> Self { Self::new(FailureCode::NetworkMismatch) }
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClientInfo {
    pub agent: Option<AgentInfo>,
    /// Millisecond-based timestamp
    pub connected: u64,
    /// Millisecond-based timestamp
    pub last_seen: u64,
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Status {
    pub clients: SmallVec<ClientInfo>,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display("{agent} v{version} on {network} (features {features:08x})")]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AgentInfo {
    pub agent: Ident,
    pub version: Version,
    pub network: Ident,
    pub features: u64,
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

impl Version {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self { Self { major, minor, patch } }
}
