//! Base module containing common structs and traits

/// Trait that defines a remote imagery model.
pub trait Model {
    /// Type of self of implemented model.
    type MyType;
    /// Returns Result<self type> for given file.
    ///
    /// # Arguments
    ///
    /// * `filename` - A string of the path to the model's file.
    fn new(filename: String) -> std::io::Result<Self::MyType>;

}
