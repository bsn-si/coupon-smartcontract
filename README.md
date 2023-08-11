<h1 align="center">
    🎟️ ✨ OCEX 🎁 👛
</h1>

<p align="center">
This is an  <a href="https://github.com/paritytech/ink">Ink!</a> smartcontract implementing a coupon payments logic. <br>
With this contract you can emit coupons worth an amount of tokens for blockchain users to make exchange operations or payments.
</p>

## Design and features
* Contract set-up with owner, set by constructor's argument, or set caller by default.
* After initialization the contract can be replenished with tokens that will be used for coupon redemption.
* Adding new coupons:
  * Coupon is a `sr25519` public key, with defined balance
  * If contract balance is enough it puts the coupon into storage and reserves the appropriate funds for redemption.
  * If contract balance is not enough the coupon is rejected for registration.
  * Multiple coupons can be registered at a time.
* Coupon redemption:
  * User's coupon has a `sr25519` secret key.
  * Before activation a coupon can be checked in the contract using it's public key - coupon status ('active' or 'burned') and coupon value will be delivered.
  * For payout - set receiver, coupon public key and private signature.
    * Signature is a receiver's wallet address encrypted with the private key of the coupon.
    * After send request if params & signature are valid, the contract unlocks reserved tokens and transfers them to connected receiver's wallet.
* Owner methods:
  * An owner can get info about `'free'` funds on the smartcontract (that are not reserved for registered coupons)
  * Free tokens (unused by coupons) can be transferred to owner's private address.
  * Ownership of the contract with all funds and liabilities can be transferred to another user.

## How to
### Install Prerequisites
Please follow installation instructions provided [here](https://docs.substrate.io/tutorials/v3/ink-workshop/pt1/#prerequisites).

### Clone this repo
```
git clone https://github.com/bsn-si/ocex-smartcontract
```

### Compile + Run Tests
```
cd ocex-smartcontract
cargo +nightly test --features=test
```

### Build Contract + metadata
```
cargo +nightly contract build
```

### Video guide
Video guide link: [![mq1](https://user-images.githubusercontent.com/98888366/172255802-217fb893-4922-4901-8a4e-2396e8aa59a9.jpg)](https://youtu.be/_paGuN7VGl8)

### Resolve common errors
You may encounter compilation or optimization errors in wasm builds.
Build tested on `nightly-2022-08-08` toolchain, with `rustc 1.64.0 (a55dd71d5 2022-09-19)` version, if you have compilation errors try changing the toolkit and compiler version to the specified one. Also you need `wasm` target and `dylint`. For install specified toolchain & targets run:

``` bash
rustup toolchain install nightly-2022-08-08
rustup target add wasm32-unknown-unknown
cargo install cargo-dylint
```

In case you get a compilation error during the wasm optimization step, make sure you have [binaryen](https://github.com/WebAssembly/binaryen) installed.

### Building in Docker
As an alternative to the standard build, you can build the smartcontract in a docker. For contract build and copy run the following commands in the source code dir:

``` bash
docker build -t ocex-contract .
docker create -ti --name ocex ocex-contract bash
docker cp ocex:/contract/target/ink/ocex.contract ~/ocex.contract
```

### Build signature helper
Simple cli tool for making coupon redemption signature. Set a contract address, coupon secret key and funds receiver's address. 
```bash
cargo +nightly build --examples --features=test
```
...and then use it like
```bash
./target/debug/examples/make-coupon-signature \
  --contract 5Ev9VH31P4asHN11VkWRSsBZNFBy82PxU9TTLEehfKt27sQG \
  --coupon 0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89 \
  --receiver 5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y
```

### Deploy on testnet
First setup and start [substrate-contracts-node](https://github.com/paritytech/substrate-contracts-node), go to [Polkadot Portal UI](https://polkadot.js.org/apps/#/contracts) for setting up a test contract.

After creating an instance of the contract on the blockchain you can use a core `owner` from test accounts list like `Alice`.
After that you need to send funds to the contract. Now you can call contract's methods from Polkadot Portal UI.

#### Example usage
- Instantiate new contract with `Alice` owner.
- Send funds to the contract. Coupons can not be created without providing liquidity.
- From Polkadot Portal UI call `addCoupon` method, for field `coupon` set user `Bob` - this is our first coupon public key.
- For `amount` set the allowed coupon amount, if amount greater than contract balance - the sending will be rejected.
- Call method with `Execute` action on Polkadot Portal UI.
- After adding a coupon you can check coupon `Bob` with `checkCoupon` method that returns a tuple of the coupon's statuses, `(is_active, amount)` 
- Try to activate the coupon with `make-coupon-signature` helper, described in the past article. You can check `Bob` secret key with [subkey](https://docs.substrate.io/v3/tools/subkey/) tool.
```bash
➜  ~ subkey inspect //Bob
Secret Key URI `//Bob` is account:
  Network ID:        substrate 
  Secret seed:       0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89
  Public key (hex):  0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48
  Account ID:        0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48
  Public key (SS58): 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
  SS58 Address:      5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
```
- We know the secret key for `Bob` coupon is `0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89` and after that we can call command like that.
```bash
./target/debug/examples/make-coupon-signature \
  --contract <YOU_CONTRACT_ADDRESS> \
  --coupon 0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89 \
  --receiver <RECEIVER_ADDRESS>
```
- Now call `activateCoupon` method, set the selected receiver, select coupon `Bob` and set the signature from the previous command.
- `Execute` in Polkadot Portal UI and congrats! Funds were transferred to the receiver's account.

## Related repos
- OCEX [coupon activation dapp](https://github.com/bsn-si/ocex-activation)
- OCEX smart contract [API client](https://github.com/bsn-si/ocex-api)
- OCEX [CLI management tool](https://github.com/bsn-si/ocex-cli)
- JS/Wasm [library](https://github.com/bsn-si/ocex-coupon-signature) for coupon activation signatures
- [CosmWasm! smart contract](https://github.com/bsn-si/figaro-contract) for ZK delivery evidence based on the principles of the OCEX

## License

[Apache License 2.0](https://github.com/bsn-si/ocex-smartcontract/blob/main/LICENSE) © Bela Supernova ([bsn.si](https://bsn.si))
