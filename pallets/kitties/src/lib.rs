
// Every FRAME pallet has:
//
// A set of frame_support and frame_system dependencies.
// Required attribute macros (i.e. configuration traits, storage items and function calls).

#![cfg_attr(not(feature = "std"), no_std)]
//Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod weight;
pub use weight::KittiesWeight;

#[frame_support::pallet]
pub mod pallet {
	use frame_support:: {
		sp_runtime::traits::{Hash, Zero},
		dispatch::{DispatchResultWithPostInfo, DispatchResult},
		traits::{Currency, ExistenceRequirement, Randomness},
		pallet_prelude::*,
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use sp_io::hashing::blake2_128;
	use scale_info::TypeInfo;
	use crate::weight::WeightInfo;

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};
	use frame_system::{Origin};

	// We define <BalanceOf<T>> and AccountOf<T> types, and use them in the Kitty.
	// If you wonder what the first line means in Rust,
	// it is to define a type alias AccountOf<T> which is just a shorthand pointing to the associated type AccountId of trait frame_system::Config that generic type T is required to be bound of.
	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	// Struct for holding Kitty information.
	// A Struct in Rust is a useful construct to help store data that have things in common.
	// For our purposes, our Kitty will carry multiple properties which we can store in a single struct instead of using separate storage items.
	// This comes in handy when trying to optimize for storage reads and writes so our runtime can perform less read/writes to update multiple values.
	// Let's first go over what information a single Kitty will carry:
	//
	// dna: the hash used to identify the DNA of a Kitty, which corresponds to its unique features. DNA is also used to breed new Kitties and to keep track of different Kitty generations.
	// price: this is a balance that corresponds to the amount needed to buy a Kitty and set by its owner.
	// gender: an enum that can be either Male or Female.
	// owner: an account ID designating a single owner.

	// Looking at the items of our struct from above, we can deduce the following types:
	//
	// [u8; 16] for dna - to use 16 bytes to represent a Kitty's DNA.
	// BalanceOf for price - this is a custom type using FRAME's Currency trait.
	// Gender for gender - we are going to create this!
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Kitty<T: Config> {
		pub dna: [u8; 16],
		pub price: Option<BalanceOf<T>>,
		pub gender: Gender,
		pub owner: AccountOf<T>,
	}

	// Enum declaration for Gender
	//
	// Notice the use of the derive macro which must precede the enum declaration. This wraps our enum in the data structures it will need to interface with other types in our runtime.
	// In order to use Serialize and Deserialize, you will need to add the serde crate in pallets/kitties/Cargo.toml:
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[cfg_attr(feature="std", derive(Serialize, Deserialize))]
	pub enum Gender {
		Male,
		Female,
	}

	// implementation to handle Gender type in Kitty struct.


	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types it depends on.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emit events, it depends on the runtime's definition of an event
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The Currency handler for the Kitties pallet.
		type Currency: Currency<Self::AccountId>;

		// Specify the custom types for our runtime.
		// The Randomness trait from frame_support requires specifying it with a parameter to replace the Output and BlockNumber generics.
		// Take a look at the documentation and the source code implementation to understand how this works. For our purposes,
		// we want the output of functions using this trait to be Blake2 128-bit hash which you'll notice should already be declared at the top of your working codebase.
		type KittyRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type MaxKittyOwned: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		/// Handles arithmetic overflow when incrementing the Kitty counter.
		KittyCntOverflow,
		/// An account cannot own more Kitties than `MaxKittyCount`.
		ExceedMaxKittyOwned,
		/// Buyer cannot be the owner.
		BuyerIsKittyOwner,
		/// Cannot transfer a kitty to its owner.
		TransferToSelf,
		/// Handles checking whether the Kitty exists.
		KittyNotExist,
		/// Handles checking that the Kitty is owned by the account transferring,
		/// buying or setting a price for it,
		NotKittyOwner,
		/// Ensures the Kitty is for sale.
		KittyNotForSale,
		/// Ensures that the buying price is greater than the asking price.
		KittyBidPriceTooLow,
		/// Ensures that an account has enough funds to purchase a Kitty.
		NotEnoughBalance,
	}

	// Events.
	// As you can see in the above snippet, we use attribute macro:
	//
	// #[pallet::generate_deposit(pub(super) fn deposit_event)]
	//
	// This allows us to deposit a specific event using the pattern below:
	//
	//
	// COPY
	// Self::deposit_event(Event::Success(var_time, var_day));
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new Kitty was successfully created. \[sender, kitty_id\]
		Created(T::AccountId, T::Hash),
		/// Kitty price was successfully set. \[sender, kitty_id, new_price\]
		PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
		/// A Kitty was successfully transferred. \[from, to, kitty_id\]
		Transferred(T::AccountId, T::AccountId, T::Hash),
		/// A Kitty was successfully bought. \[buyer, seller, kitty_id, bid_price\]
		Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
	}

	// let's add the most simple logic we can to our runtime: a function that stores a variable in our runtime. To do this we'll use StorageValue from Substrate's storage API which is a trait that depends on the storage macro.
	//
	// All that means for our purposes is that for any storage item we want to declare, we must include the #[pallet::storage] macro beforehand.
	// ACTION: Storage item to keep a count of all existing Kitties in existence.
	#[pallet::storage]
	#[pallet::getter(fn kitty_cnt)]
	/// Keeps track of the number of Kitties in existence
	pub(super) type KittyCnt<T: Config> = StorageValue<_, u64, ValueQuery>;

	// Remaining storage items.
	// Our runtime needs to be made aware of:
	//
	// Unique assets, like currency or Kitties (this will be held by a storage map called Kitties).
	// Ownership of those assets, like account IDs (this will be handled a new storage map called KittiesOwned).
	// To create a storage instance for the Kitty struct, we'll be usingStorageMap — a hash-map provided to us by FRAME.
	//
	// Here's what the Kitties storage item looks like:

	// Breaking it down, we declare the storage type and assign a StorageMap that takes:
	//
	//
	// - The [`Twox64Concat`][2x64-rustdocs] hashing algorithm.
	//    - A key of type `T::Hash`.
	//    - A value of type `Kitty<T>`.
	#[pallet::storage]
	#[pallet::getter(fn kitties)]
	pub(super) type Kitties<T:Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		Kitty<T>,
	>;

	// The KittiesOwned storage item is similar except that we'll be using a BoundedVec to keep track of some maximum number of Kitties we'll configure in runtime/src/lib.s
	#[pallet::storage]
	#[pallet::getter(fn kitties_owned)]
	/// Keeps track of what accounts own what Kitty
	pub(super) type KittiesOwned<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxKittyOwned>,
		ValueQuery,
	>;

	// Our pallet's genesis configuration.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// create_kitty
		#[pallet::weight(<T as Config>::WeightInfo::create_kitty())]
		pub fn create_kitty(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let kitty_id = Self::mint(&sender, None, None)?;
			log::info!("A kitty is born with ID: {:?}.", kitty_id);

			// Deposit `Created` event
			Self::deposit_event(Event::Created(sender, kitty_id));
			Ok(())
		}
		// set_price
		// set the price for a Kitty
		// updates Kitty and updates storage.
		#[pallet::weight(<T as Config>::WeightInfo::set_price())]
		pub fn set_price(
			origin: OriginFor<T>,
			kitty_id: T::Hash,
			new_price: Option<BalanceOf<T>>
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure the kitty exists and is called by the kitty owner
			ensure!(Self::is_kitty_owner(&kitty_id, &sender)?,<Error<T>>::NotKittyOwner);

			// To update the price of a Kitty, we'll need to:
			//
			// Get the Kitty object in storage.
			// Update the object with the new price.
			// Save it back into storage.
			let mut kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;
			kitty.price = new_price.clone();
			<Kitties<T>>::insert(&kitty_id, kitty);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet(sender, kitty_id, new_price));

			Ok(())
		}

		// transfer
		#[pallet::weight(<T as Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			kitty_id: T::Hash
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// Ensure the kitty exists and is called by the kitty owner
			ensure!(Self::is_kitty_owner(&kitty_id, &from)?, <Error<T>>::NotKittyOwner);

			// Verify the kitty is not transferring back to its owner.
			ensure!(from != to, <Error<T>>::TransferToSelf);

			// Verify the recipient has the capacity to receive one more kitty
			let to_owned = <KittiesOwned<T>>::get(&to);
			ensure!((to_owned.len() as u32) < T::MaxKittyOwned::get(), <Error<T>>::ExceedMaxKittyOwned);

			Self::transfer_kitty_to(&kitty_id, &to)?;

			Self::deposit_event(Event::Transferred(from, to, kitty_id));

			Ok(())
		}

		// buy_kitty
		#[transactional]
		#[pallet::weight(100)]
		pub fn buy_kitty(
			origin: OriginFor<T>,
			kitty_id: T::Hash,
			bid_price: BalanceOf<T>
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			// Check the kitty exists and buyer is not the current kitty owner
			let kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;
			ensure!(kitty.owner != buyer, <Error<T>>::BuyerIsKittyOwner);

			// check the kitty is for sale and the kitty ask price <= bid_price
			if let Some(ask_price) = kitty.price {
				ensure!(ask_price <= bid_price, <Error<T>>::KittyBidPriceTooLow);
			} else {
				Err(<Error<T>>::KittyNotForSale)?;
			}

			// Check the buyer has enough free balance
			ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

			// Verify the buyer has the capacity to receive one more kitty
			let to_owned = <KittiesOwned<T>>::get(&buyer);
			ensure!((to_owned.len() as u32) < T::MaxKittyOwned::get(), <Error<T>>::ExceedMaxKittyOwned);

			let seller = kitty.owner.clone();

			// Transfer the amount from buyer to seller
			T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;

			// Transfer the kitty from seller to buyer
			Self::transfer_kitty_to(&kitty_id, &buyer)?;

			Self::deposit_event(Event::Bought(buyer, seller, kitty_id, bid_price));

			Ok(())
		}

		// breed_kitty
		// Breed two kitties to create a new generation
		// of kitties.
		#[pallet::weight(100)]
		pub fn breed_kitty(
			origin: OriginFor<T>,
			kid1: T::Hash,
			kid2: T::Hash
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Check: Verify `sender` owns both kitties (and both kitties exist).
			ensure!(Self::is_kitty_owner(&kid1, &sender)?, <Error<T>>::NotKittyOwner);
			ensure!(Self::is_kitty_owner(&kid2, &sender)?, <Error<T>>::NotKittyOwner);

			let new_dna = Self::breed_dna(&kid1, &kid2)?;
			Self::mint(&sender, Some(new_dna), None)?;

			Ok(())
		}
	}

	// our helper functions
	impl<T: Config> Pallet<T> {
		// helper function for Kitty Struct
		// Configuring a struct is useful in order to pre-define a value in our struct.
		// For example, when setting a value in relation to what another function returns.
		// In our case we have a similar situation where we need to configure our Kitty struct in such a way that sets Gender according to a Kitty's DNA.
		//
		// We'll only be using this function when we get to creating Kitties.
		// Regardless, let us learn how to write it now and get it out of the way. We'll create a public function called gen_gender that returns the Gender type and uses a random function to choose between Gender enum values.
		fn gen_gender() -> Gender {
			let random = T::KittyRandomness::random(&b"gender"[..]).0;
			match random.as_ref()[0] % 2 {
				0 => Gender::Male,
				_ => Gender::Female,
			}
		}

		// helper functions for dispatchable functions

		// function to randomly generate DNA
		// Generating DNA is similar to using randomness to randomly assign a gender type.
		// The difference is that we'll be making use of blake2_128 we imported in the previous part
		fn gen_dna() -> [u8; 16] {
			let payload = (
					T::KittyRandomness::random(&b"dna"[..]).0,
					<frame_system::Pallet<T>>::block_number(),
				);
			payload.using_encoded(blake2_128)
		}

		// mint
		// Let's get right to it. Our mint() function will take the following arguments:
		//
		// owner: of type &T::AccountId - this indicates whom the kitty belongs to.
		// dna: of type Option<[u8; 16]> - this specifies the DNA of the kitty going to be minted. If None is passed in, a random DNA will be generated.
		// gender: of type Option<Gender> - ditto.
		// And it will return Result<T::Hash, Error<T>>.

		// The first thing we're doing is creating a new Kitty object. Then, we create a unique kitty_id using a hashing function based on the current properties of the kitty.
		//
		// Next, we increment the KittyCnt using the storage getter function Self::kitty_cnt(). We also checking for overflow with check_add() function.
		//
		// Once we've done with the check, we proceed with updating our storage items by:
		//
		// Making use of try_mutate to update the kitty's owner vector.
		// Using the insert method provided by Substrate's StorageMap API to store the actually Kitty object and associate it with its kitty_id.
		// Using put provided by the StorageValue API to store the latest Kitty count.

		// A Quick Recap Of Our Storage Items
		// <Kitties<T>>: Stores a Kitty's unique traits and price, by storing the Kitty object and associating it with its Kitty ID.
		// <KittyOwned<T>>: Keeps track of what accounts own what Kitties.
		// <KittyCnt<T>>: A count of all Kitties in existence.
		pub fn mint(
			owner: &T::AccountId,
			dna: Option<[u8; 16]>,
			gender: Option<Gender>,
		) -> Result<T::Hash, Error<T>> {
			let kitty = Kitty::<T> {
				dna: dna.unwrap_or_else(Self::gen_dna),
				price: None,
				gender: gender.unwrap_or_else(Self::gen_gender),
				owner: owner.clone(),
			};

			let kitty_id = T::Hashing::hash_of(&kitty);

			// Performs this operation first as it may fail
			let new_cnt = Self::kitty_cnt().checked_add(1).ok_or(<Error<T>>::KittyCntOverflow)?;

			// Performs this operation first because as it may fail
			<KittiesOwned<T>>::try_mutate(&owner, |kitty_vec| {
				kitty_vec.try_push(kitty_id)
			}).map_err(|_| <Error<T>>::ExceedMaxKittyOwned)?;

			<Kitties<T>>::insert(kitty_id, kitty);
			<KittyCnt<T>>::put(new_cnt);
			Ok(kitty_id)
		}

		// In case Self::is_kitty_owner() returns an error object Err(<Error<T>>::KittyNotExist),
		// it is returned early with <Error<T>>::KittyNotExist by the ?.
		// If it returns Ok(bool_val), the bool_val is extracted, and if false, returns <Error<T>>::NotKittyOwner error.
		pub fn is_kitty_owner(kitty_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::kitties(kitty_id) {
				Some(kitty) => Ok(kitty.owner == *acct),
				None => Err(<Error<T>>::KittyNotExist)
			}
		}

		// transfer_kitty_io
		// The transfer_kitty_to function will be a helper to perform all storage updates once a Kitty is transferred (and it is going to be called when a kitty is bought and sold too). All it needs to do is perform safety checks and update the following storage items:
		//
		// KittiesOwned: to update the owner of the Kitty.
		// Kitties: to reset the price in the Kitty object to None.

		// Notice the use of #[transactional] which we imported at the very beginning of this tutorial.
		// It allows us to write dispatchable functions that commit changes to the storage only if the annotated function returns Ok.
		// Otherwise all changes are discarded.

		#[transactional]
		pub fn transfer_kitty_to(
			kitty_id: &T::Hash,
			to: &T::AccountId,
		) -> Result<(), Error<T>> {
			let mut kitty = Self::kitties(&kitty_id).ok_or(<Error<T>>::KittyNotExist)?;

			let prev_owner = kitty.owner.clone();

			// Remove `kitty_id` from the KittyOwned vector of `prev_kitty_owner`
			<KittiesOwned<T>>::try_mutate(&prev_owner, |owned| {
				if let Some(ind) = owned.iter().position(|&id| id == *kitty_id) {
					owned.swap_remove(ind);
					return Ok(());
				}
				Err(())
			}).map_err(|_| <Error<T>>::KittyNotExist)?;

			// Update the kitty owner
			kitty.owner = to.clone();
			// Reset the ask price so the kitty is not for sale until `set_price()` is called
			// by the current owner
			kitty.price = None;

			<Kitties<T>>::insert(kitty_id, kitty);

			<KittiesOwned<T>>::try_mutate(to, |vec| {
				vec.try_push(*kitty_id)
			}).map_err(|_| <Error<T>>::ExceedMaxKittyOwned)?;

			Ok(())
		}

		// The logic behind breeding two Kitties is to multiply each corresponding DNA segment from two Kitties,
		// which will produce a new DNA sequence.
		// Then, that DNA is used when minting a new Kitty.
		pub fn breed_dna(kid1: &T::Hash, kid2: &T::Hash) -> Result<[u8; 16], Error<T>> {
			let dna1 = Self::kitties(kid1).ok_or(<Error<T>>::KittyNotExist)?.dna;
			let dna2 = Self::kitties(kid2).ok_or(<Error<T>>::KittyNotExist)?.dna;

			let mut new_dna = Self::gen_dna();
			for i in 0..new_dna.len() {
				new_dna[i] = new_dna[i] & dna1[i] | (!new_dna[i] & dna2[i]);
			}
			Ok(new_dna)
		}


	}

}
