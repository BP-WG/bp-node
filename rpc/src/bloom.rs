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

// TODO: Move to `amplify`

use amplify::{Bytes, Wrapper, WrapperMut};

use crate::BP_RPC_LIB;

pub type BloomFilter32 = BloomFilter<32>;

#[derive(Wrapper, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Default, From)]
#[wrapper(AsSlice, Display, FromStr, Hex)]
#[derive(StrictType, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BloomFilter<const LEN: usize>(Bytes<LEN>);

impl<const LEN: usize> BloomFilter<LEN> {
    pub fn new(filter: impl Into<[u8; LEN]>) -> Self { Self(filter.into().into()) }

    pub fn insert(&mut self, value: impl Into<[u8; LEN]>) {
        for (byte, add) in self.0.as_inner_mut().iter_mut().zip(value.into().iter()) {
            *byte |= *add;
        }
    }

    pub fn with_inserted(mut self, value: impl Into<[u8; LEN]>) -> Self {
        self.insert(value);
        self
    }

    pub fn contains(&self, value: impl Into<[u8; LEN]>) -> bool {
        for (byte, add) in self.0.as_inner().iter().zip(value.into().iter()) {
            if *byte & *add == 0 {
                return false;
            }
        }
        true
    }
}

impl<T: Into<[u8; LEN]>, const LEN: usize> Extend<T> for BloomFilter<LEN> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}
