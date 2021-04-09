#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ReservableCurrency, ExistenceRequirement::KeepAlive},
	transactional, dispatch::DispatchResult
};
use sp_std::{fmt::Debug, vec::Vec};
use frame_system::pallet_prelude::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{CheckedAdd, Bounded, CheckedSub,
			 AccountIdConversion, StaticLookup, Zero, One, AtLeast32BitUnsigned},
	ModuleId, RuntimeDebug, SaturatedConversion,
};
use codec::FullCodec;

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The balance of an account.
		type Balance: Parameter + Member + AtLeast32BitUnsigned + codec::Codec + Default + Copy + MaybeSerializeDeserialize + Debug;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// XX
		XX,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// XX. \[owner\]
		XX(T::AccountId),
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_runtime_upgrade() -> Weight {
			0
		}

		fn integrity_test () {}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		_phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				_phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {}
	}

	/// Storage version of the pallet.
	#[pallet::storage]
	pub(super) type xx<T: Config> = StorageValue<_, bool, ValueQuery>;


	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dbbb
		///
		/// - `dest`: transfer reserve balance from sub_account to dest
		#[pallet::weight(100_000_000)]
		#[transactional]
		pub fn deposit(
			origin: OriginFor<T>,
			tx_hash: Vec<u8>,
			eth_addr: Vec<u8>,
			#[pallet::compact] value: T::Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {

}
