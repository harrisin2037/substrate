// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
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

//! Parallel tasks example

#![cfg_attr(not(feature = "std"), no_std)]

use frame_system::ensure_signed;
use frame_support::{
	dispatch::DispatchResult, decl_module, decl_storage, decl_event,
};
use sp_runtime::RuntimeDebug;

use codec::{Encode, Decode};
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
}

decl_storage! {
	trait Store for Module<T: Trait> as ExampleOffchainWorker {
		/// A vector of current participants
		///
		/// To enlist someone to participate, signed payload should be
		/// sent to `enlist`.
		Participants get(fn participants): Vec<Vec<u8>>;

		/// Current event id to enlist participants to.
		CurrentEventId get(fn get_current_event_id): Vec<u8>;
	}
}

decl_event!(
	/// Events generated by the module.
	pub enum Event {
		/// Whenn new event is drafted.
		NewEventDrafterd(Vec<u8>),
	}
);

/// Request to enlist participant.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct EnlistedParticipant {
	pub account: Vec<u8>,
	pub signature: Vec<u8>,
}

impl EnlistedParticipant {
	fn verify(&self, event_id: &[u8]) -> bool {
		use sp_core::Public;
		use std::convert::TryFrom;
		use sp_runtime::traits::Verify;

		match sp_core::sr25519::Signature::try_from(&self.signature[..]) {
			Ok(signature) => {
				let public = sp_core::sr25519::Public::from_slice(self.account.as_ref());
				signature.verify(event_id, &public)
			}
			_ => false
		}
	}
}

decl_module! {
	/// A public part of the pallet.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Get the new event running.
		#[weight = 0]
		pub fn run_event(origin, id: Vec<u8>) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Participants::kill();
			CurrentEventId::mutate(move |event_id| *event_id = id);
			Ok(())
		}

		/// Submit new price to the list via unsigned transaction.
		#[weight = 0]
		pub fn enlist_participants(origin, participants: Vec<EnlistedParticipant>)
			-> DispatchResult
		{
			let _ = ensure_signed(origin)?;

			if validate_participants_parallel(&CurrentEventId::get(), &participants[..]) {
				for participant in participants {
					Participants::append(participant.account);
				}
			}
			Ok(())
		}
	}
}

fn validate_participants_parallel(event_id: &[u8], participants: &[EnlistedParticipant]) -> bool {

	fn spawn_verify(data: Vec<u8>) -> Vec<u8> {
		let stream = &mut &data[..];
		let event_id = Vec::<u8>::decode(stream).expect("Failed to decode");
		let participants = Vec::<EnlistedParticipant>::decode(stream).expect("Failed to decode");

		for participant in participants {
			if !participant.verify(&event_id) {
				return false.encode()
			}
		}
		true.encode()
	}

	let mut async_payload = Vec::new();
	event_id.encode_to(&mut async_payload);
	participants[..participants.len() / 2].encode_to(&mut async_payload);

	let handle = sp_io::tasks::spawn(spawn_verify, async_payload).expect("failed to spawn");
	let mut result = true;

	for participant in &participants[participants.len()/2+1..] {
		if !participant.verify(event_id) {
			result = false;
			break;
		}
	}

	bool::decode(&mut &handle.join()[..]).expect("Failed to decode result") && result
}
