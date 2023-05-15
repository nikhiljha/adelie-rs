use async_trait::async_trait;
use color_eyre::eyre::Result;

#[async_trait]
pub trait HyperToString {
    async fn hyper_to_string(&mut self) -> Result<String>;
}

#[async_trait]
impl HyperToString for hyper::Body {
    async fn hyper_to_string(&mut self) -> Result<String> {
        Ok(String::from_utf8(hyper::body::to_bytes(self).await?.to_vec())?)
    }
}
