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

use amplify::confinement::{SmallVec, TinyString};
use bpstd::{Address, BlockHash, BlockHeader, NormalIndex, Outpoint, Sats, StdDescr, Txid};
use bpwallet::BlockHeight;
use strict_encoding::Ident;

// TODO: Do a dedicated type in BP Std Lib
type MilliSats = Sats;

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
//#[derive(StrictApi)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum ApiRequest {
    //#[api(sub)]
    #[strict_type(tag = 0x02)]
    Actor(ActorRequest),

    //#[api(sub)]
    #[strict_type(tag = 0x04)]
    Signer(SignerRequest),

    //#[api(sub)]
    #[strict_type(tag = 0x06)]
    Wallet(WalletRequest),

    //#[api(sub)]
    #[strict_type(dumb, tag = 0x10)]
    Explorer(ExplorerRequest),
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum ActorRequest {}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum SignerRequest {}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
//#[derive(StrictApi)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum WalletRequest {
    //#[api(action = list)]
    #[strict_type(tag = 0x00, dumb)]
    List,

    //#[api(action = add)]
    #[strict_type(tag = 0x02)]
    Add(Ident, StdDescr),

    //#[api(action = remove)]
    #[strict_type(tag = 0x04)]
    Remove(Ident),

    //#[api(action = get)]
    #[strict_type(tag = 0x10)]
    Info(Ident),

    //#[api(action = get)]
    #[strict_type(tag = 0x12)]
    Utxos(Ident),

    //#[api(action = get)]
    #[strict_type(tag = 0x14)]
    History(Ident),

    //#[api(action = list)]
    #[strict_type(tag = 0x16)]
    Addresses {
        wallet: Ident,
        from: NormalIndex,
        count: NormalIndex,
        change: bool,
    },

    //#[api(action = update)]
    #[strict_type(tag = 0x20)]
    NextAddress { wallet: Ident, shift: bool },

    //#[api(action = update)]
    #[strict_type(tag = 0x22)]
    Pay {
        from: Ident,
        beneficiary: Address,
        amount: Sats,
        fee_rate: MilliSats,
    },

    //#[api(action = update)]
    #[strict_type(tag = 0x24)]
    PayAdvanced {
        from: Ident,
        beneficiary: Address,
        amount: Sats,
        fee_rate: MilliSats,
        spend: SmallVec<Outpoint>,
    },
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum ExplorerRequest {
    //#[api(action = get)]
    #[strict_type(tag = 0x00)]
    Info(BlockRef),

    //#[api(action = get)]
    #[strict_type(tag = 0x02)]
    Block(BlockRef),

    //#[api(action = get)]
    #[strict_type(tag = 0x04)]
    Tx(Txid),

    //#[api(action = get)]
    #[strict_type(tag = 0x06, dumb)]
    Fees,
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
#[derive(StrictType, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = order)]
pub enum BlockRef {
    #[default]
    Tip,
    Hash(BlockHash),
    Height(BlockHeight),
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
//#[derive(StrictApi)]
#[strict_type(lib = BP_RPC_LIB, tags = custom)]
pub enum WalletReply {
    #[strict_type(tag = 0x01, dumb)]
    Success,

    #[strict_type(tag = 0x00)]
    Failure(Failure),

    #[strict_type(tag = 0x10)]
    Created(Ident),

    #[strict_type(tag = 0x12)]
    Wallets(),

    #[strict_type(tag = 0x14)]
    Info(),

    #[strict_type(tag = 0x16)]
    Utxos(),

    #[strict_type(tag = 0x18)]
    History(),

    #[strict_type(tag = 0x20)]
    NewBlock(BlockHeader),

    #[strict_type(tag = 0x22)]
    NewTx(),
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB)]
pub struct Failure {
    code: FailureCode,
    message: TinyString,
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[derive(StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = BP_RPC_LIB, tags = custom, into_u8, try_from_u8)]
pub enum FailureCode {
    #[strict_type(tag = 500, dumb)]
    Internal,

    #[strict_type(tag = 400)]
    Unsupported,
}
