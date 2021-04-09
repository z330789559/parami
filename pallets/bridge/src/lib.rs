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
	traits::{CheckedAdd, Bounded, CheckedSub, Saturating,
			 AccountIdConversion, StaticLookup, Zero, One, AtLeast32BitUnsigned},
	ModuleId, RuntimeDebug, SaturatedConversion,
};
use codec::FullCodec;

mod mock;
mod tests;

pub use module::*;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Erc20Transfer<Balance> {
	/// Value
	#[codec(compact)]
	pub value: Balance,
	// From
	pub from: Vec<u8>,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The balance of an account.
		type Balance: Parameter + Member + AtLeast32BitUnsigned + codec::Codec + Default + Copy + MaybeSerializeDeserialize + Debug;

		/// The currency trait.
		type Currency: Currency<Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// bridge admin not set
		BridgeAdminNotSet,
		/// no permission
		NoPermission,
		/// duplicated tx hash
		DuplicatedTxHash,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Received Erc20 Transfer event \[tx_hash\]
		Deposited(Vec<u8>),
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

	/// The privileged account.
	#[pallet::storage]
	#[pallet::getter(fn bridge_admin)]
	pub(super) type BridgeAdmin<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	/// Erc20 transfer transactions.
	///
	/// `tx_hash` map to `Erc20Transfer`
	#[pallet::storage]
	#[pallet::getter(fn erc20_txs)]
	pub type Erc20Txs<T: Config> = StorageMap<_, Identity, Vec<u8>, Erc20Transfer<T::Balance>>;

	/// Erc20 balances in parami
	///
	/// `eth_addr` map to value.
	#[pallet::storage]
	#[pallet::getter(fn erc20_balances)]
	pub type Erc20Balances<T: Config> = StorageMap<_, Blake2_128Concat, Vec<u8>, T::Balance, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the privileged account
		#[pallet::weight((100_000, DispatchClass::Operational))]
		#[transactional]
		pub fn set_bridge_admin(
			origin: OriginFor<T>,
			admin: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let admin = T::Lookup::lookup(admin)?;
			BridgeAdmin::<T>::put(admin);
			Ok((None, Pays::No).into())
		}

		/// Received an `Transfer` event from ethereum erc20 contract.
		///
		/// - `tx_hash`: The transaction hash of this erc20 event in ethereum.
		/// - `value`: Amount transferred.
		/// - `eth_addr`: `value` was transferred from `eth_addr`.
		#[pallet::weight((100_000, DispatchClass::Operational, Pays::Yes))]
		#[transactional]
		pub fn deposit(
			origin: OriginFor<T>,
			tx_hash: Vec<u8>,
			eth_addr: Vec<u8>,
			#[pallet::compact] value: T::Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(who == Self::bridge_admin().ok_or(Error::<T>::BridgeAdminNotSet)?, Error::<T>::NoPermission);
			Erc20Txs::<T>::mutate_exists(tx_hash.clone(), |maybe_tx| {
				if maybe_tx.is_none() {
					Erc20Balances::<T>::mutate(&eth_addr, |balance| {
						*balance = balance.saturating_add(value);
					});

					*maybe_tx = Some(Erc20Transfer {
						value,
						from: eth_addr,
					});

					Self::deposit_event(Event::Deposited(tx_hash));
				}
			});
			Ok((None, Pays::No).into())
		}
	}
}
