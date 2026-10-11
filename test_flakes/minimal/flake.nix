{
  description = "No inputs block and a single-line outputs pattern";

  outputs = { self }: {
    devShells.x86_64-linux.default = { packages = [ ]; };
  };
}
