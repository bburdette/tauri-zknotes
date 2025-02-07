{
  description = "zknotes, a web based zettelkasten";

  inputs = {
    nixpkgs = { url = "github:nixos/nixpkgs/nixos-24.05"; };
    flake-utils.url = "github:numtide/flake-utils";
    # naersk.url = "github:nmattia/naersk";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # rust-overlay.url = "github:oxalica/rust-overlay";

  };

  outputs = { self, nixpkgs, flake-utils, fenix  }:
    let
      # mytauri = { pkgs }: pkgs.callPackage ./tauri/my-tauri.nix { };
      # mytaurimobile = { pkgs }: pkgs.callPackage ./tauri/my-tauri-mobile.nix { };
      # mtpkgs =  nixpkgs // { rust = nixpkgs.rust_1_76; rustPackages = nixpkgs.rustPackages_1_76; } ;
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pname = "zknotes";
        # naersk-lib = naersk.lib."${system}";
        # elm-stuff = makeElmPkg { inherit pkgs; };
        # rust-stuff = naersk-lib.buildPackage {
        #     pname = pname;
        #     root = ./.;
        #     buildInputs = with pkgs; [
        #       cargo
        #       rustc
        #       sqlite
        #       pkgconfig
        #       openssl.dev
        #       ];
        #   };

        # fenix stuff for adding other compile targets
        mkToolchain = fenix.packages.${system}.combine;
        toolchain = fenix.packages.${system}.stable;
        # toolchain = fenix.packages.${system}.default;
        target1 = fenix.packages.${system}.targets."aarch64-linux-android".stable;
        target2 = fenix.packages.${system}.targets."armv7-linux-androideabi".stable;
        target3 = fenix.packages.${system}.targets."i686-linux-android".stable;
        target4 = fenix.packages.${system}.targets."x86_64-linux-android".stable;

        mobileTargets = mkToolchain (with toolchain; [
          cargo
          rustc
          target1.rust-std
          target2.rust-std
          target3.rust-std
          target4.rust-std
        ]);

        # pkgs = nixpkgs.legacyPackages."${system}" {
        pkgs = import nixpkgs {
          config.android_sdk.accept_license = true;
          config.allowUnfree = true;
          system = "${system}";
          # overlays = [ rust-overlay.overlays.default ];
          # overlays = [ (self: super: super // { cargo = toolchain.cargo; rustc = toolchain.rustc; }) ];
          # overlays = [ fenix.overlays.default ];
          # rustPlatform = nixpkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };
        };

        # mytauri = { pkgs }: pkgs.callPackage ./tauri/my-tauri.nix { };
        # my-tauri = mytauri { inherit pkgs; };
        # my-tauri = pkgs.callPackage ./tauri/my-tauri.nix {
        #   rustPlatform = pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };
        # };

        # my-tauri = pkgs.callPackage ./tauri/my-tauri.nix {
        #   inherit (pkgs.rustPackages_1_76) rustPlatform;
        # };

        # wasn't making `cargo tauri` work last time I tried
        # mtplatform = pkgs.makeRustPlatform { inherit (fenix.packages.${system}.stable) cargo rustc; };
        # mytauri = { pkgs }: pkgs.callPackage ./tauri/my-tauri.nix { rustPlatform = mtplatform; };
        # my-tauri = mytauri { inherit pkgs; };

        # my-tauri-mobile = mytaurimobile { inherit pkgs; };

        libraries = with pkgs;[
          gcc13
          webkitgtk
          gtk3
          cairo
          gdk-pixbuf
          glib
          # dbus
          openssl_3
          librsvg
        ];

      in
      rec {
        inherit pname;
        # `nix build`
        # packages.${pname} = pkgs.stdenv.mkDerivation {
        #   nativeBuildInputs = [ pkgs.makeWrapper ];
        #   name = pname;
        #   src = ./.;
        #   # building the 'out' folder
        #   installPhase = ''
        #     mkdir -p $out/share/zknotes/static
        #     mkdir $out/bin
        #     cp -r $src/server/static $out/share/zknotes
        #     cp ${elm-stuff}/main.js $out/share/zknotes/static
        #     cp -r ${rust-stuff}/bin $out
        #     mv $out/bin/zknotes-server $out/bin/.zknotes-server
        #     makeWrapper $out/bin/.zknotes-server $out/bin/zknotes-server --set ZKNOTES_STATIC_PATH $out/share/zknotes/static;
        #     '';
        # };
        # defaultPackage = packages.${pname};

        # `nix run`
        # apps.${pname} = flake-utils.lib.mkApp {
        #   drv = packages.${pname};
        # };
        # defaultApp = apps.${pname};

        # meh = pkgs.androidenv.androidPkgs_9_0 // { android_sdk.accept_license = true; };
        # androidComposition = pkgs.androidenv.androidPkgs_9_0 // { includeNDK = true; };

        androidEnv = pkgs.androidenv.override { licenseAccepted = true; };
        androidComposition = androidEnv.composeAndroidPackages {
          includeNDK = true;
          platformToolsVersion = "34.0.5";
          buildToolsVersions = [ "34.0.0" ];
          platformVersions = [ "34" ];
          extraLicenses = [
            "android-googletv-license"
            "android-sdk-arm-dbt-license"
            "android-sdk-license"
            "android-sdk-preview-license"
            "google-gdk-license"
            "intel-android-extra-license"
            "intel-android-sysimage-license"
            "mips-android-sysimage-license"
          ];
        };
        # `nix develop`
        devShell = pkgs.mkShell {

          NIX_LD = "${pkgs.stdenv.cc.libc}/lib/ld-linux-x86-64.so.2";
          ANDROID_HOME = "${androidComposition.androidsdk}/libexec/android-sdk";
          NDK_HOME = "${androidComposition.androidsdk}/libexec/android-sdk/ndk/${builtins.head (pkgs.lib.lists.reverseList (builtins.split "-" "${androidComposition.ndk-bundle}"))}";
          ANDROID_SDK_ROOT = "${androidComposition.androidsdk}/libexec/android-sdk";
          ANDROID_NDK_ROOT = "${androidComposition.androidsdk}/libexec/android-sdk/ndk-bundle";

          # enables file open dialog; without it get an error:
          # 'No GSettings schemas are installed on the system'
          shellHook =
            ''
              # export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
              export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS
            '';

          nativeBuildInputs = with pkgs; [
            androidComposition.androidsdk
            androidComposition.ndk-bundle
            # cargo
            # rustc
            cargo-watch
            rustfmt
            rust-analyzer
            sqlite
            openssl.dev
            # aarch64-linux-android-pkgs.sqlite
            # aarch64-linux-android-pkgs.openssl.dev
            pkg-config
            elm2nix
            elmPackages.elm
            elmPackages.elm-analyse
            elmPackages.elm-doc-preview
            elmPackages.elm-format
            elmPackages.elm-live
            elmPackages.elm-test
            elmPackages.elm-upgrade
            elmPackages.elm-xref
            elmPackages.elm-language-server
            elmPackages.elm-verify-examples
            elmPackages.elmi-to-json
            elmPackages.elm-optimize-level-2
            # extra stuff for tauri
            # my-tauri  
            curl
            wget
            # dbus
            # openssl_3
            # gcc13
            glib
            gtk3
            libsoup
            # webkitgtk
            librsvg
            #  wut
            cairo
            # cargo-tauri
            atk
            # glib
            # dbus
            # webkitgtk
            # librsvg

            # gst stuff for tauri AV
            # Video/Audio data composition framework tools like "gst-inspect", "gst-launch" ...
            gst_all_1.gstreamer
            # Common plugins like "filesrc" to combine within e.g. gst-launch
            gst_all_1.gst-plugins-base
            # Specialized plugins separated by quality
            gst_all_1.gst-plugins-good
            gst_all_1.gst-plugins-bad
            gst_all_1.gst-plugins-ugly
            # Plugins to reuse ffmpeg to play almost every video format
            gst_all_1.gst-libav
            # Support the Video Audio (Hardware) Acceleration API
            gst_all_1.gst-vaapi

            # for tauti-mobile
            # librsvg
            webkitgtk_4_1
            # tauri-mobile
            # my-tauri-mobile
            libxml2
            lldb
            nodejs
            # rustup # `cargo tauri android init` wants this, even though targets already installed.
            # should be fixed though, https://github.com/tauri-apps/tauri/issues/7044
            alsa-lib
            mobileTargets
            # they suggest using the jbr (jetbrains runtime?) from android-studio, but that is not accessible.
            jetbrains.jdk
          ];
        };
      }
    );
}

