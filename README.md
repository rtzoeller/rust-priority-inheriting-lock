# Priority-Inheriting Lock for Rust

A priority-inheriting lock implementation based on Linux futexes.

It uses [@m-ou-se](https://github.com/m-ou-se/)'s [`linux-futex`](https://docs.rs/linux-futex/latest/linux_futex/) crate to implement [@Amanieu](https://github.com/Amanieu/)'s [`lock_api`](https://docs.rs/lock_api/latest/lock_api/).

In general, you should consider using the lock implementations provided by `std` or `parking_lot`, unless your application is intended to run on a real-time system where [priority inversions](https://en.wikipedia.org/wiki/Priority_inversion) must be avoided.

## Minimum Rust version

The current minimum supported Rust version (MSRV) is 1.69. The MSRV will not be changed in the future without bumping the major or minor version.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.