let
    pkgs = import <nixpkgs> {};
in pkgs.mkShell {
    nativeBuildInputs = with pkgs; [
        rustc
        rust-analyzer
        cargo
        pkg-config
        openssl
    ];
}
