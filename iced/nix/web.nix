# The same client compiled to wasm, and the shell that loads it: `nix build
# .#harken-web` pinned, `nix run .#web` on a laptop at localhost:8080.
{
  perSystem = { pkgs, lib, toolchain, rustPlatform, sources, script, ... }:
    let
      # wasm-bindgen's generated glue and the module it generates for carry
      # a schema version that must match *exactly*, so the CLI has to be the
      # version in Cargo.lock and not whatever a channel happens to ship —
      # nixos-25.05 has 0.2.100 and 0.2.104, and the lock wants 0.2.128.
      # Reading the version out of the lockfile keeps the two in step: bump
      # the crate and this follows, and only the hashes need a human.
      wasmBindgenVersion =
        let
          lock = builtins.readFile ../../Cargo.lock;
          after = pkgs.lib.strings.removePrefix
            "name = \"wasm-bindgen\"\nversion = \""
            (builtins.elemAt (builtins.split "name = \"wasm-bindgen\"\nversion = \"" lock) 2);
        in
        builtins.head (builtins.split "\"" after);

      # From static.crates.io rather than `fetchCrate`, for the same reason
      # the vendor directory is built by cargo: the `api/v1/…/download`
      # endpoint answers 403 to the User-Agent nixpkgs' fetchers send. This
      # host serves the identical tarball and has no such opinion.
      wasmBindgenSrc = pkgs.fetchzip {
        url = "https://static.crates.io/crates/wasm-bindgen-cli/wasm-bindgen-cli-${wasmBindgenVersion}.crate";
        hash = "sha256-a7lcXJnnZkYReja+iUO7NqqrWyv3toxnUgQb8s4IS5s=";
        extension = "tar.gz";
      };

      wasm-bindgen-cli = rustPlatform.buildRustPackage {
        pname = "wasm-bindgen-cli";
        version = wasmBindgenVersion;
        src = wasmBindgenSrc;
        cargoDeps = pkgs.stdenv.mkDerivation {
          name = "wasm-bindgen-cli-vendor";
          src = wasmBindgenSrc;
          nativeBuildInputs = [ toolchain pkgs.cacert pkgs.git ];
          buildPhase = ''
            export CARGO_HOME=$PWD/.cargo-home
            mkdir -p $out
            cargo vendor --versioned-dirs $out > $out/config.toml
            cp Cargo.lock $out/Cargo.lock
          '';
          dontInstall = true;
          dontFixup = true;
          outputHashMode = "recursive";
          outputHashAlgo = "sha256";
          outputHash = "sha256-xhQmH6oqGIEKdnovKaEwnwg6zMUkSyD6P9PPKjDx2ns=";
        };
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.openssl ];
        doCheck = false;
      };

      # The same client, compiled to wasm, plus the shell that loads it.
      # `nix run .#web-build` does this too; the difference is that this one
      # cannot reach the network, so every version is pinned rather than
      # fetched when it turns out not to match.
      harken-web = rustPlatform.buildRustPackage {
        pname = "harken-web";
        version = "0.1.0";
        src = sources.workspace;
        inherit (sources) cargoDeps;
        doCheck = false;

        nativeBuildInputs = [ wasm-bindgen-cli pkgs.llvmPackages.clang-unwrapped pkgs.llvmPackages.bintools ];

        buildPhase = ''
          runHook preBuild

          # SQLite's C is not compiled here: the browser peer keeps its
          # database in the page, so the crate needs the symbols to link
          # and never calls them. An empty archive satisfies the linker.
          mkdir -p sqlite-stub
          printf '!<arch>\n' > sqlite-stub/libsqlite3.a

          export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='--cfg getrandom_backend="wasm_js"'
          export CC_wasm32_unknown_unknown=clang
          export AR_wasm32_unknown_unknown=llvm-ar
          export CFLAGS_wasm32_unknown_unknown="-resource-dir ${pkgs.lib.getLib pkgs.llvmPackages.clang-unwrapped}/lib/clang/${pkgs.lib.versions.major pkgs.llvmPackages.clang-unwrapped.version}"
          export SQLITE3_LIB_DIR="$PWD/sqlite-stub"
          export SQLITE3_STATIC=1

          cargo build -p harken-iced --target wasm32-unknown-unknown --release --offline

          runHook postBuild
        '';

        installPhase = ''
          runHook preInstall
          mkdir -p $out
          cp iced/web/index.html $out/
          wasm-bindgen --target web --no-typescript --out-dir $out/pkg \
            target/wasm32-unknown-unknown/release/harken-iced.wasm
          runHook postInstall
        '';
      };

      # The iced client compiled to wasm, into `iced/web/pkg`.
      #
      # `CC_wasm32_unknown_unknown` because gcc cannot target wasm, and cc-rs
      # falls back to plain `CC` — which the devshell exports. An *unwrapped*
      # clang, since nix's wrapped one injects glibc include paths and fails
      # the same way; it then needs its own builtin headers passed in, because
      # nix keeps them in a separate output. `SQLITE3_LIB_DIR` because
      # `libsqlite3-sys` still emits `-lsqlite3` even when `sqlite-wasm-rs` is
      # the one providing SQLite, so it is pointed at an empty archive.
      #
      # wasm-bindgen's CLI must match the crate in Cargo.lock exactly; when
      # the one on PATH does not, the matching one is fetched into `target/`.
      wasmClang = pkgs.llvmPackages.clang-unwrapped;
      webBuild = ''
        want="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;q}' Cargo.lock)"
        have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
        if [ "$have" = "$want" ]; then
          bindgen=wasm-bindgen
        else
          bindgen="$PWD/target/wasm-tools/bin/wasm-bindgen"
          if [ "$("$bindgen" --version 2>/dev/null | awk '{print $2}' || true)" != "$want" ]; then
            echo "wasm-bindgen ''${have:-none} on PATH, need $want — fetching it into target/"
            cargo install wasm-bindgen-cli --locked --version "$want" --root "$PWD/target/wasm-tools"
          fi
        fi
        mkdir -p target/wasm-sqlite-stub iced/web/pkg
        printf '!<arch>\n' > target/wasm-sqlite-stub/libsqlite3.a
        CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
        CC_wasm32_unknown_unknown=${wasmClang}/bin/clang \
        AR_wasm32_unknown_unknown=${pkgs.llvmPackages.llvm}/bin/llvm-ar \
        CFLAGS_wasm32_unknown_unknown="-resource-dir ${lib.getLib wasmClang}/lib/clang/${lib.versions.major wasmClang.version}" \
        SQLITE3_LIB_DIR="$PWD/target/wasm-sqlite-stub" SQLITE3_STATIC=1 \
          cargo build -p harken-iced --target wasm32-unknown-unknown --release
        "$bindgen" --target web --no-typescript --out-dir iced/web/pkg \
          target/wasm32-unknown-unknown/release/harken-iced.wasm
      '';
    in
    {
      packages = { inherit harken-web; };

      apps = {
        web.program = script "web" {
          runtimeInputs = [ pkgs.wasm-bindgen-cli pkgs.python3 ];
          text = ''
            ${webBuild}
            echo "serving on http://localhost:8080"
            cd iced/web && python3 -m http.server 8080
          '';
        };
        web-build.program = script "web-build" {
          runtimeInputs = [ pkgs.wasm-bindgen-cli ];
          text = webBuild;
        };
      };
    };
}
