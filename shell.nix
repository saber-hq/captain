{ pkgs }:
pkgs.mkShell {
  buildInputs = with pkgs;
    (pkgs.lib.optionals pkgs.stdenv.isLinux ( [
      solana
      # anchor
      spl-token-cli
      libudev
      rustup
    ])) ++ [
      cargo-deps
      cargo-watch
      cargo-udeps

      # sdk
      (yarn.override { nodejs = nodejs-14_x; })
      nodejs-14_x
      python3

      pkgconfig
      openssl
      jq

      libiconv
    ];
  shellHook = ''
    export PATH=$HOME/.cargo/bin:$PATH
  '';
}
