use std::borrow::Cow;

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
