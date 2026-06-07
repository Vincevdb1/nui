use crate::nix::model::{Input, Output, Package};
use std::path::Path;

#[allow(dead_code)]
pub trait NixParser {
    type Error;
    fn parse_flake(path: &Path) -> Result<Vec<Input>, Self::Error>;
    fn parse_outputs(path: &Path) -> Result<Vec<Output>, Self::Error>;
}

#[allow(dead_code)]
pub trait NixQuery {
    type Error;
    fn get_packages(flake_path: &Path, output: &Output) -> Result<Vec<Package>, Self::Error>;
    fn get_inputs(flake_path: &Path) -> Result<Vec<Input>, Self::Error>;
}

pub trait NixEditor {
    type Error;
    fn add_package(flake_path: &Path, output: &Output, package: &str) -> Result<(), Self::Error>;
    fn remove_package(flake_path: &Path, output: &Output, package: &str)
    -> Result<(), Self::Error>;
    #[allow(dead_code)]
    fn add_input(flake_path: &Path, input: &Input) -> Result<(), Self::Error>;
    fn remove_input(flake_path: &Path, input_name: &str) -> Result<(), Self::Error>;
}
