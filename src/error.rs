use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdelieError {
    #[error("{0}")]
    Misc(String),
}
