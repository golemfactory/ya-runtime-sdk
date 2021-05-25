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

impl IntoVec<u8> for String {
    fn into_vec(self) -> Vec<u8> {
        self.into_bytes()
    }
}
