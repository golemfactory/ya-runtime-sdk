use std::borrow::Cow;
use tokio::io::AsyncWriteExt;

pub trait IntoVec<T> {
    fn into_vec(self) -> Vec<T>;
}

impl<T> IntoVec<T> for Vec<T> {
    fn into_vec(self) -> Vec<T> {
        self
    }
}

impl<T> IntoVec<T> for Box<[T]> {
    fn into_vec(self) -> Vec<T> {
        self.into()
    }
}

impl<'a> IntoVec<u8> for &'a [u8] {
    fn into_vec(self) -> Vec<u8> {
        self.to_vec()
    }
}

impl IntoVec<u8> for String {
    fn into_vec(self) -> Vec<u8> {
        self.into_bytes()
    }
}

impl<'a> IntoVec<u8> for &'a str {
    fn into_vec(self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl<'a> IntoVec<u8> for Cow<'a, str> {
    fn into_vec(self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

pub(crate) async fn write_output(json: serde_json::Value) -> anyhow::Result<()> {
    let string = json.to_string();
    let mut stdout = tokio::io::stdout();
    stdout.write_all(string.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
