{
  self,
  system,
  flake-utils,
}:
let
  gathers-api = flake-utils.lib.mkApp {
    drv = self.packages.${system}.gathers-api;
    exePath = "/bin/server";
  };

  gathers-cli = flake-utils.lib.mkApp {
    drv = self.packages.${system}.gathers-api;
    exePath = "/bin/gathers";
  };
in
{
  inherit gathers-api gathers-cli;
  default = gathers-cli;
}
