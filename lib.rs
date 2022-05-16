// (c) 2022 Anton Shramko (anton.shramko@bsn.si)
//
//! Ocex - coupons bounties implemented with Ink! smartcontract

#![cfg_attr(not(feature = "std"), no_std)]
extern crate core;
use ink_lang as ink;

#[ink::contract]
mod ocex {
    use ink_storage::traits::SpreadAllocate;
    use schnorrkel::{signing_context, PublicKey, Signature};

    use ink_env::AccountId as ReceiverAddress;
    use ink_env::AccountId as CouponId;

    // Coupons list arguments of request/response
    type OptCoupons = [Option<CouponId>; 5];

    /// Result for inserted and declined coupons
    /// when balance is not enough to guarantee payout
    #[derive(Debug, Default, PartialEq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct CouponsResult {
        accepted: OptCoupons,
        declined: OptCoupons,
    }

    impl From<scale::Error> for CouponsResult {
        fn from(_: scale::Error) -> Self {
            panic!("encountered unexpected invalid SCALE encoding")
        }
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    /// Error types
    pub enum Error {
        /// Caller is not the owner of the contract
        AccessOwner,
        /// Contract balance doesn't have enough
        /// liquidity to reserve for a new coupon or payout
        ContractBalanceNotEnough,
        /// Invalid coupon, parse failed
        InvalidParseCoupon,
        /// Invalid coupon payout signature, parse failed
        InvalidParseCouponSignature,
        /// Verify signature failed
        VerifySignatureFailed,
        /// Coupon already exists
        CouponAlreadyExists,
        /// Coupon already burned
        CouponAlreadyBurned,
        /// Coupon not found
        CouponNotFound,
        /// Transfer Errors
        TransferFailed,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Ocex {
        // Coupons are addresses with tokens balances
        coupons: ink_storage::Mapping<CouponId, Balance>,
        // Burned coupons after activation
        burned: ink_storage::Mapping<CouponId, bool>,
        // Smart-contract owner by default is the contract publisher
        owner: ink_env::AccountId,
        // Reserved balance for coupons payout
        reserved: Balance,
    }

    impl Ocex {
        /// You can set a contract owner while deploying the contract
        #[ink(constructor)]
        pub fn new(owner: ink_env::AccountId) -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                contract.owner = owner;
                contract.reserved = 0;
            })
        }

        /// Owner is the contract publisher by default
        #[ink(constructor)]
        pub fn default() -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                contract.owner = Self::env().caller();
                contract.reserved = 0;
            })
        }

        /// Set new `coupon` with declared amount.
        /// - Coupon is accepted only if the contract has enough balance.
        /// - Only the `owner` can set a new `coupon`.
        /// Returns: if added - return `amount`, otherwise return none
        #[ink(message)]
        pub fn add_coupon(&mut self, coupon: CouponId, amount: Balance) -> Result<Balance, Error> {
            (Self::env().caller() == self.owner)
                .then(|| true)
                .ok_or(Error::AccessOwner)
                .and_then(|_| {
                    (self.rest_balance() >= amount)
                        .then(|| true)
                        .ok_or(Error::ContractBalanceNotEnough)
                        .and_then(|_| self.insert_coupon(&coupon, amount))
                })
        }

        /// Set array `max 5 items` of `coupon` with declared per key.
        /// - Accept only if the contract has enough balance.
        /// - Only the `owner` can set a new `coupon`.
        /// Returns: returns struct with accepted (added & active) and declined coupons (if balance is not enough)
        #[ink(message)]
        pub fn add_coupons(&mut self, coupons: OptCoupons, amount: Balance) -> Result<CouponsResult, Error> {
            (Self::env().caller() == self.owner)
                .then(|| true)
                .ok_or(Error::AccessOwner)
                .and_then(|_| {
                    (self.rest_balance() >= amount)
                        .then(|| true)
                        .ok_or(Error::ContractBalanceNotEnough)
                })
                .and_then(|_| {
                    Ok(coupons.into_iter().fold(
                        (
                            CouponsResult::default(),
                            self.rest_balance(),
                            0 as usize,
                            0 as usize,
                        ),
                        |(mut result, mut rest_balance, mut la, mut ld), opt| {
                            if let (Some(coupon), Some(true)) = (opt, Some(rest_balance >= amount)) {
                                if self.insert_coupon(&coupon, amount.clone()).is_ok() {
                                    result.accepted[la] = Some(coupon);
                                    rest_balance -= amount;
                                    la += 1;
                                } else {
                                    result.declined[ld] = Some(coupon);
                                    ld += 1;
                                }
                            } else {
                                result.declined[ld] = opt;
                                ld += 1;
                            }

                            return (result, rest_balance, la, ld);
                        },
                    ))
                })
                .and_then(|(result, _, _, _)| Ok(result))
        }

        /// Activate `coupon` with transfer of appropriate liquidity to a receiver's address.
        /// Verified by `sr25519` `signature` with `receiver address`
        /// with `contract id` context
        ///
        /// Returns: boolean success if all valid
        #[ink(message)]
        pub fn activate_coupon(
            &mut self,
            transfer_to: ReceiverAddress,
            coupon: CouponId,
            sign: [u8; 64],
        ) -> Result<bool, Error> {
            self.coupons
                .get(&coupon)
                .ok_or(Error::InvalidParseCoupon)
                .and_then(|coupon_amount| {
                    // check that coupons aren't burned
                    self.burned
                        .get(&coupon)
                        .is_none()
                        .then(|| coupon_amount)
                        .ok_or(Error::CouponAlreadyBurned)
                })
                .and_then(|coupon_amount| {
                    // parsing & cast coupon key
                    let public_key =
                        PublicKey::from_bytes(coupon.as_ref()).or(Err(Error::InvalidParseCoupon))?;

                    Ok((coupon_amount, public_key))
                })
                .and_then(|(coupon_amount, public_key)| {
                    // parsing & cast signature
                    let signature =
                        Signature::from_bytes(&sign).or(Err(Error::InvalidParseCouponSignature))?;

                    Ok((coupon_amount, public_key, signature))
                })
                .and_then(|(coupon_amount, public_key, signature)| {
                    let context = signing_context(Self::env().account_id().as_ref());

                    // verify signature payload with context by coupon key
                    public_key
                        .verify(context.bytes(transfer_to.as_ref()), &signature)
                        .or(Err(Error::VerifySignatureFailed))
                        .and_then(|_| Ok(coupon_amount))
                })
                .and_then(|coupon_amount| {
                    // check that contract balance is enough for transfer
                    (coupon_amount <= self.env().balance())
                        .then(|| coupon_amount)
                        .ok_or(Error::ContractBalanceNotEnough)
                })
                .and_then(|coupon_amount| {
                    // transfer funds to verified receiver
                    self.env()
                        .transfer(transfer_to, coupon_amount)
                        .or_else(|_| Err(Error::TransferFailed))
                        .and_then(|_| Ok(coupon_amount))
                })
                .and_then(|_| self.burn_coupon(&coupon))
        }

        /// Method for transferring spare balance (not reserved for coupons)
        /// to owner's wallet. (for example, if you've transferred more funds
        /// to the smart-contract that was necessary)
        #[ink(message)]
        pub fn payback_not_reserved_funds(&mut self) -> Result<bool, Error> {
            (Self::env().caller() == self.owner)
                .then(|| true)
                .ok_or(Error::AccessOwner)
                .and_then(|_| Ok(self.rest_balance()))
                .and_then(|rest_balance| {
                    // transfer funds to verified receiver
                    self.env()
                        .transfer(self.owner, rest_balance)
                        .or_else(|_| Err(Error::TransferFailed))
                })
                .and_then(|_| Ok(true))
        }

        /// Method for disabling and burning registered (but not redeemed) coupons.
        /// The contract unlocks reserved funds. Burned coupons can't be reactivated later.
        #[ink(message)]
        pub fn burn_coupons(&mut self, coupons: OptCoupons) -> Result<CouponsResult, Error> {
            (Self::env().caller() == self.owner)
                .then(|| true)
                .ok_or(Error::AccessOwner)
                .and_then(|_| {
                    Ok(coupons.into_iter().fold(
                        (CouponsResult::default(), 0 as usize, 0 as usize),
                        |(mut result, mut la, mut ld), opt| {
                            if let Some(coupon) = opt {
                                if self.burn_coupon(&coupon).is_ok() {
                                    result.accepted[la] = Some(coupon);
                                    la += 1;
                                } else {
                                    result.declined[ld] = Some(coupon);
                                    ld += 1;
                                }
                            }

                            return (result, la, ld);
                        },
                    ))
                })
                .and_then(|(result, _, _)| Ok(result))
        }

        /// Verification that the coupon is registered and it's value
        #[ink(message)]
        pub fn check_coupon(&self, coupon: CouponId) -> (bool, Balance) {
            self.coupons
                .get(&coupon)
                .and_then(|exists_amount| Some((Self::env().balance() >= exists_amount, exists_amount)))
                .and_then(|(enough_funds, exists_amount)| {
                    Some((enough_funds && self.burned.get(&coupon).is_none(), exists_amount))
                })
                .unwrap_or_else(|| (false, 0))
        }

        /// Get info on spare funds of the contract (not reserved for coupons)
        /// available for withdrawal
        /// Allow request only from the contract owner, otherwise return zero
        #[ink(message)]
        pub fn available_balance(&mut self) -> Balance {
            (Self::env().caller() == self.owner)
                .then(|| self.rest_balance())
                .unwrap_or_default()
        }

        /// Transfer contract ownership to another user
        #[ink(message)]
        pub fn transfer_ownership(&mut self, account: ink_env::AccountId) -> Result<bool, Error> {
            (Self::env().caller() == self.owner)
                .then(|| {
                    self.owner = account;
                    true
                })
                .ok_or(Error::AccessOwner)
        }

        #[inline]
        fn insert_coupon(&mut self, coupon: &CouponId, amount: Balance) -> Result<Balance, Error> {
            self.coupons
                .get(&coupon)
                .is_none()
                .then(|| true)
                .ok_or(Error::CouponAlreadyExists)
                .and_then(|_| {
                    // insert new coupon to the storage
                    self.coupons.insert(coupon, &amount);
                    // reserve balance for payout
                    self.reserved += amount;

                    Ok(amount)
                })
        }

        #[inline]
        fn burn_coupon(&mut self, coupon: &CouponId) -> Result<bool, Error> {
            self.coupons
                .get(&coupon)
                .ok_or(Error::CouponNotFound)
                .and_then(|amount| {
                    // mark coupon as burned
                    self.burned.insert(&coupon, &true);
                    // cancellation of funds reservation
                    self.reserved -= amount;

                    Ok(true)
                })
        }

        #[inline]
        fn rest_balance(&self) -> Balance {
            Self::env().balance() - self.reserved
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use schnorrkel::{Keypair, MiniSecretKey};
        use ink_env::AccountId;
        use ink_lang as ink;

        #[ink::test]
        // Simple check for coupon insertion
        // And coupon activation with it's secret key
        fn insert_coupon_activation() {
            let accounts = default_accounts();

            // setup contract
            let contract_balance = 1000;
            let mut contract = create_contract(contract_balance);

            // setup sender (by default `alice` also publisher and can add coupons)
            set_sender(accounts.alice);

            // setup coupon
            let (coupon_one, coupon_signer) = get_coupon();
            let coupon_amount: u128 = 500;

            // adding one coupon with target amount
            assert_eq!(
                contract.add_coupon(coupon_one.clone(), coupon_amount),
                Ok(coupon_amount)
            );

            // Check funds are reserved
            set_sender(accounts.alice);
            assert_eq!(contract.available_balance(), 500);

            // Check methods by client
            set_sender(accounts.eve);
            set_balance(accounts.eve, 0);

            // check added coupon & amount;
            assert_eq!(
                contract.check_coupon(coupon_one.clone()),
                (true, coupon_amount)
            );

            // Activate coupon
            let context = signing_context(contract_id().as_ref());
            let signature = coupon_signer.sign(context.bytes(accounts.eve.as_ref()));

            assert_eq!(
                contract.activate_coupon(accounts.eve, coupon_one.clone(), signature.to_bytes()),
                Ok(true)
            );

            // Coupon activated - amount transfered to caller
            assert_eq!(get_balance(accounts.eve), 500);

            // Check available funds (not reserved) and withdraw
            set_sender(accounts.alice);
            assert_eq!(contract.available_balance(), 500);

            // transfer the rest of funds back to the owner
            assert_eq!(contract.payback_not_reserved_funds(), Ok(true));
        }

        #[ink::test]
        fn insert_coupons() {
            let accounts = default_accounts();

            // setup contract
            let contract_balance = 1000;
            let mut contract = create_contract(contract_balance);

            // setup sender, (by default `alice` also publisher and can add coupons)
            set_sender(accounts.alice);

            // setup coupon
            let (coupon_one, _) = get_coupon();
            let coupon_amount: u128 = 500;

            let test_coupons = [
                Some(coupon_one.clone()),
                Some(accounts.charlie),
                Some(accounts.django),
                Some(accounts.frank),
                Some(accounts.bob),
            ];

            // insert multiple coupons with total amount
            // that exceeds the contract spare liquidity
            assert_eq!(
                contract.add_coupons(test_coupons, coupon_amount),
                Ok(CouponsResult {
                    accepted: [Some(coupon_one.clone()), Some(accounts.charlie), None, None, None,],
                    declined: [Some(accounts.django), Some(accounts.frank), Some(accounts.bob), None, None,]
                })
            );

            // Check that funds are reserved and withdrawal is restricted
            assert_eq!(contract.available_balance(), 0);

            // Burn inserted coupons
            assert_eq!(
                contract.burn_coupons([Some(coupon_one.clone()), Some(accounts.charlie), None, None, None]),
                Ok(CouponsResult {
                    accepted: [Some(coupon_one.clone()), Some(accounts.charlie), None, None, None,],
                    declined: [None, None, None, None, None]
                })
            );

            assert_eq!(contract.available_balance(), 1000);
        }

        #[ink::test]
        fn check_transfer_ownership() {
            let accounts = default_accounts();

            // setup contract
            let contract_balance = 1000;
            let mut contract = create_contract(contract_balance);

            // setup sender, (by default `alice` also publisher and can add coupons)
            set_sender(accounts.alice);
            assert_eq!(contract.owner, accounts.alice);
            
            // Transfer ownership to bob
            assert_eq!(contract.transfer_ownership(accounts.bob), Ok(true));
            assert_eq!(contract.owner, accounts.bob);

            // try payback rest funds from old owner
            assert_eq!(contract.payback_not_reserved_funds(), Err(Error::AccessOwner));

            // try payback rest funds from new owner
            set_balance(accounts.bob, 0);
            set_sender(accounts.bob);
    
            assert_eq!(contract.payback_not_reserved_funds(), Ok(true));
            assert_eq!(get_balance(accounts.bob), 1000);
            assert_eq!(contract.available_balance(), 0);
        }

        fn create_contract(initial_balance: Balance) -> Ocex {
            let accounts = default_accounts();

            set_sender(accounts.alice);
            set_balance(contract_id(), initial_balance);

            // Alice is the publisher and owner by default
            Ocex::default()
        }

        fn contract_id() -> AccountId {
            ink_env::test::callee::<ink_env::DefaultEnvironment>()
        }

        fn set_sender(sender: AccountId) {
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(sender);
        }

        fn default_accounts() -> ink_env::test::DefaultAccounts<ink_env::DefaultEnvironment> {
            ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
        }

        fn set_balance(account_id: AccountId, balance: Balance) {
            ink_env::test::set_account_balance::<ink_env::DefaultEnvironment>(account_id, balance)
        }

        fn get_balance(account_id: AccountId) -> Balance {
            ink_env::test::get_account_balance::<ink_env::DefaultEnvironment>(account_id)
                .expect("Cannot get account balance")
        }

        fn get_coupon() -> (CouponId, Keypair) {
            let coupon = MiniSecretKey::generate();
            let keypair: Keypair = Keypair::from(coupon.expand(MiniSecretKey::ED25519_MODE));
            let coupon: CouponId = keypair.secret.to_public().to_bytes().clone().into();

            (coupon, keypair)
        }
    }
}
