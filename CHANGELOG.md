# Changelog

## [1.0.0] - 2024-06-12

- Updated MSRV to 1.69.0.
- Updated `libc` dependency to 0.2.155.
- Updated `linux-futex` dependency to 1.0.0.
- Updated `lock_api` dependency to 0.4.12.
- Add support for `aarch64-linux-android`.

## [0.3.0] - 2023-11-26

- Updated MSRV to 1.65.0.
- Mark `RawPriorityInheritingLock::new()` and `gettid()` as `#[must_use]`.

## [0.2.3] - 2023-06-09

- Updated `linux-futex` dependency to 0.2.0.

## [0.2.2] - 2022-11-12

- Remove `once_cell` dependency.

## [0.2.1] - 2022-10-24

- Remove `thread_local` dependency.

## [0.2.0] - 2022-08-16

- Added try-lock timeout support via `lock_api::RawMutexTimed`.
- Updated `linux-futex` dependency to 0.1.2.

## [0.1.1] - 2022-06-19

- Updated `lock_api` dependency to 0.4.7.

## [0.1.0] - 2022-06-18

Initial release.
