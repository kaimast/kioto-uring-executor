# A simple multi-threaded async runtime for io_uring 

*NOTE: This crate is not part of the tokio project*

This crate provides a thread-per-core async runtime using io_uring. It either uses monoio or tokio-uring as its backend, with monoio being the default.
Similar to other thread-per-core runtimes, this crate does not implement work stealing. This can cause problems if some futures run longer than others.

Once tokio-uring or monoio native support for multi-threading, this crate will not be maintained aymore. See the discussion [here](https://github.com/tokio-rs/tokio-uring/issues/258) for more information.

The API is aimed to be similar to that of tokio. Please see the tests for some examples how to use the crate.

One core difference to conventional async runtimes is that it has a `spawn_with` and a `block_with` method, that take a function returning a non-send future as its result. The latter is needed as most io_uring futures are not Send. The only safe way to share data between tthreads is then to send a generator fucntion that creates the future on the other thread.

## Name
To indicate this being an unofficial hack, the crate is prefixed with kioto. Kyoto is often spelled as Kioto in German, similar to how Tokyo is often spelled Tokio.
