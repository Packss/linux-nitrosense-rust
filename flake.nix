{
  description = "Acer NitroSense for Linux (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "linux-nitrosense";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.wrapGAppsHook4
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.glib
            pkgs.gtk4
            pkgs.libadwaita
            pkgs.gsettings-desktop-schemas
            pkgs.kmod
          ];

          postInstall = ''
            install -D -m 644 linux-nitrosense.desktop $out/share/applications/linux-nitrosense.desktop
            install -D -m 644 linux-nitrosense.service $out/lib/systemd/system/linux-nitrosense.service
          '';

          meta = with pkgs.lib; {
            description = "Acer NitroSense fan/power/keyboard control for Linux";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [ pkgs.pkg-config pkgs.wrapGAppsHook4 ];
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rustfmt
            pkgs.libclang
            pkgs.clippy
            pkgs.gtk4
            pkgs.glib
            pkgs.gsettings-desktop-schemas
          ];
          
           shellHook = ''
            export XDG_DATA_DIRS=$XDG_DATA_DIRS:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}
            echo "NitroSense Rust dev shell"
          '';
        };
      }
    ) // {
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.linux-nitrosense;
        in
        {
          options.services.linux-nitrosense = {
            enable = lib.mkEnableOption "Acer NitroSense Service";
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ 
              self.packages.${pkgs.system}.default 
              pkgs.gsettings-desktop-schemas
              pkgs.gtk4
            ];

            systemd.services.linux-nitrosense = {
              description = "NitroSense Service";
              wantedBy = [ "multi-user.target" ];
              serviceConfig = {
                ExecStart = "${self.packages.${pkgs.system}.default}/bin/linux-nitrosense --daemon";
                Restart = "on-failure";
                Type = "simple";
              };
            };
          };
        };
    };
}
