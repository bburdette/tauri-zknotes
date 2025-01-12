Moving the tauri code here from the main zknotes repo, since I don't want the heavy build dependencies needed for tauri to be part of the zknotes flake.  

This is pre alpha and is only a development environment for now; the flake doesn't build anything.  

Submodules employed liberally so `git submodule update --init --recursive` before trying to build.

We're assuming you have the nix package manager installed.  If on nixos, you'll probably nix-ld enabled.  That's `programs.nix-ld.enable = true` in your configuration.nix.

Also, for adb to work you'll need:

users.users.<you>.extraGroups = ["adbusers"];

On the first build you'll need to init to make the tauri generated code.  Something like this:

```
git submodule update --init --recursive
cd tauri
nix develop
cd zknotes/elm
./buildprod.sh
cd ../tauri/src-tauri
cargo tauri android init
```

Media files will not work properly in the browser unless you go to the ApplicationManifest.xml in the gen/ folder, and change  
        `android:usesCleartextTraffic="${usesCleartextTraffic}">`  
to this:  
        `android:usesCleartextTraffic="true">`  

Then build:

```
cargo tauri android build --apk -t armv7
```

Then if you have adb set up on your system, and your phone plugged in, and usb debugging enabled on it.  Copy the path of the output file from the previous step, then:

```
adb install /home/bburdette/code/tzkn-test/tauri/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

Currently it does work sort of, but I have to restart it after starting it up the first time.  

### tauri desktop app:

Its handy to build the tauri desktop app for debugging, as you can right click there and inspect to get the js debugger.  Build the elm first.

You can build the tauri desktop app with regular `cargo build`, or go straight to running it with `cargo run`..
