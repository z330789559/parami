#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use crate::mock::{Event, *};

#[test]
fn deposit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let tx_hash = ||{vec![11]};
		let tx_hash2 = ||{vec![12]};
		let eth_addr = ||{vec![22]};
		let value: Balance = 444;
		assert_ok!(Bridge::set_bridge_admin(Origin::root(), ALICE));

		assert_eq!(None, Bridge::erc20_txs(tx_hash()));
		assert_ok!(Bridge::deposit(Origin::signed(ALICE), tx_hash(), eth_addr(), value));
		assert_eq!(last_event(), Event::parami_bridge(crate::Event::Deposited(tx_hash())));
		assert_eq!(Some(crate::Erc20Transfer{
			value,
			from: eth_addr(),
		}), Bridge::erc20_txs(tx_hash()));

		assert_eq!(value, Bridge::erc20_balances(eth_addr()));

		assert_ok!(Bridge::deposit(Origin::signed(ALICE), tx_hash2(), eth_addr(), value));
		assert_eq!(last_event(), Event::parami_bridge(crate::Event::Deposited(tx_hash2())));

		assert_ok!(Bridge::deposit(Origin::signed(ALICE), tx_hash2(), eth_addr(), value));
		assert_ok!(Bridge::deposit(Origin::signed(ALICE), tx_hash2(), eth_addr(), value));

		assert_eq!(value*2, Bridge::erc20_balances(eth_addr()));
	});
}

#[test]
fn deposit_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Bridge::deposit(Origin::signed(ALICE), vec![11], vec![22], 444),
			Error::<Runtime>::BridgeAdminNotSet,
		);

		assert_ok!(Bridge::set_bridge_admin(Origin::root(), ALICE));
		assert_noop!(
			Bridge::deposit(Origin::signed(BOB), vec![11], vec![22], 444),
			Error::<Runtime>::NoPermission,
		);
	});
}

#[test]
fn set_bridge_admin_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(None, Bridge::bridge_admin());
		assert_ok!(Bridge::set_bridge_admin(Origin::root(), ALICE));
		assert_eq!(Some(ALICE), Bridge::bridge_admin());
	});
}

#[test]
fn set_bridge_admin_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Bridge::set_bridge_admin(Origin::signed(ALICE), BOB),
			DispatchError::BadOrigin,
		);
	});
}
