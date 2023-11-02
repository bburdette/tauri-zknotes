Moving the tauri code here from the main zknotes repo, since I don't want the heavy build dependencies needed for tauri to be part of the zknotes flake.  

This is pre alpha and is only a development environment for now; the flake doesn't build anything.  

Submodules employed liberally so `git submodule update --init --recursive` before trying to build.

```
cd tauri/src-tauri
cargo tauri android build --apk -t armv7
```
