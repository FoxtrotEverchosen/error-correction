{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    rustup
    wasm-pack
    nodejs
  ];

  shellHook = ''
    # Install the toolchain if cargo can't run yet (first run only, needs network).
    if ! cargo --version >/dev/null 2>&1; then
      echo "Installing the Rust stable toolchain..."
      rustup toolchain install stable --profile default && rustup default stable
    fi
    rustup target add wasm32-unknown-unknown
  '';
}
