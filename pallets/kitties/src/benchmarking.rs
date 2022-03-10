// This file is part of Substrate.

// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Kitties pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{benchmarks, whitelisted_caller, account};
use frame_system::RawOrigin;
use crate::pallet::BalanceOf;
use frame_support::sp_runtime::traits::Hash;

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	create_kitty {
		// set up an account that is of signed origin
		let caller: T::AccountId = whitelisted_caller();

	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		// verify that there is an hash(kitty_id) of a kitty already in storage
		let hash = <Kitties<T>>::iter_keys().next().unwrap();
		// assert that the last event that is being called when you run the extrinsic function
		assert_last_event::<T>(Event::Created(caller, hash).into());
	}

	// another benchmark of another extrinsic
	// set_price
	set_price {
		// set up an account
		let caller: T::AccountId = whitelisted_caller();

		let price: BalanceOf<T> = 100u32.into();

		// because we need the kitty id
		let kitty_id = crate::Pallet::<T>::mint(&caller,None,None).unwrap();

	}: set_price(RawOrigin::Signed(caller.clone()), kitty_id, Some(price))
	verify {
		assert_last_event::<T>(Event::PriceSet(caller, kitty_id, Some(price)).into());
	}

	transfer {
		// set up an account
		let caller: T::AccountId = whitelisted_caller();

		let to: T::AccountId = account("recipient", 0, SEED);

		// because we need the kitty id
		let kitty_id = crate::Pallet::<T>::mint(&caller,None,None).unwrap();


	}: _(RawOrigin::Signed(caller.clone()), to.clone(), kitty_id)
	verify {
		assert_last_event::<T>(Event::Transferred(caller, to, kitty_id).into());
	}
}

// ./target/release/node-kitties benchmark --chain=dev --steps=50 --repeat=20 --pallet=pallet_kitties --extrinsic=transfer --execution=wasm --wasm-execution=compiled --heap-pages=4096 --output=.

