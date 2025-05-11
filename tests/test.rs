#[cfg(test)]
mod tests {
    use bible::EnglishVersion::KingJames;
    use bible::{Bible, Language::English};

    #[test]
    fn bible() {
        let _ = Bible::from(English(KingJames));

        println!("Done");
    }
}
