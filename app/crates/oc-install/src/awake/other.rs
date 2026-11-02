pub struct Guard;

impl Guard {
    pub fn acquire(_reason: &str) -> Result<Self, String> {
        Err("no keep-awake mechanism on this OS".into())
    }
}
