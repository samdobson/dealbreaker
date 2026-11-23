# Embed in C, Swift, or Kotlin

`crates/dealbreaker-ffi` exposes a small JSON-in/JSON-out C ABI with thread-safe cooperative cancellation—making it straightforward to embed Dealbreaker in C, C++, Swift (iOS/macOS), Kotlin/JNI (Android), Go, or Zig.

## Build the Native Library

```sh
cargo build --release -p dealbreaker-ffi
```

This produces `target/release/libdealbreaker_ffi.{a,dylib,so,dll}` alongside the header at [`crates/dealbreaker-ffi/include/dealbreaker.h`](https://github.com/samdobson/dealbreaker/blob/main/crates/dealbreaker-ffi/include/dealbreaker.h).

## Build an Apple XCFramework (iOS + macOS)

```sh
./scripts/build-xcframework.sh
```

This builds `Dealbreaker.xcframework` ready to drop into an Xcode project or Swift Package.

## Minimal C Example

```c
#include <stdio.h>
#include "dealbreaker.h"

int main(void) {
    DealbreakerSolver *solver = dealbreaker_solver_from_preset("freecell");
    char *state = dealbreaker_solver_deal_microsoft(solver, 1);
    char *result = dealbreaker_solver_solve(solver, state, NULL, NULL);

    printf("%s\n", result);

    dealbreaker_string_free(result);
    dealbreaker_string_free(state);
    dealbreaker_solver_free(solver);
    return 0;
}
```

## Thread Safety & Memory Ownership

- **`DealbreakerSolver` is thread-safe** for concurrent `solve` / `hint` / `best` calls once created.
- **Free returned `char *` strings** with `dealbreaker_string_free`—*except* `dealbreaker_last_error()`, which returns a thread-local pointer owned by Dealbreaker.
- **Cancel from another thread** by passing a `DealbreakerCancel *` to `solve`/`best`/`hint` and calling `dealbreaker_cancel_trigger()` when needed.

See the auto-generated [C ABI Reference](/reference/c-abi) for every function prototype.
