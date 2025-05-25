pub trait StylizedText {
    fn to_colored(&self) -> String;
    fn to_title(&self) -> String;
}

impl StylizedText for &str {
    fn to_colored(&self) -> String {
        format!("\x1b[32m{self}\x1b[0m")
    }
    fn to_title(&self) -> String {
        format!("{}\n{}", self, "=".repeat(self.len()))
    }
}
