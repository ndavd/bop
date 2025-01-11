use std::fmt::Display;

use super::text::StylizedText;

#[derive(Debug)]
pub struct Table {
    rows: Vec<Vec<String>>,
    color_first_row: bool,
    spacing: usize,
    pub title: String,
}

impl From<Vec<Vec<String>>> for Table {
    fn from(value: Vec<Vec<String>>) -> Self {
        Self {
            rows: value,
            ..Default::default()
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self {
            title: String::default(),
            rows: Vec::default(),
            color_first_row: true,
            spacing: 2,
        }
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.rows.len() == 0 {
            return write!(f, "");
        }
        if self.title != String::default() {
            let title = self.title.as_str().to_title();
            writeln!(f, "{title}")?;
        }
        let row_count = self.rows.len();
        let col_count = self.rows[0].len();
        let mut col_widths: Vec<usize> = vec![0; col_count];
        for i in 0..row_count {
            for j in 0..col_count {
                col_widths[j] = (self.rows[i][j].len() + self.spacing).max(col_widths[j]);
            }
        }
        for i in 0..row_count {
            let row = self.rows[i]
                .iter()
                .enumerate()
                .map(|(j, r)| format!("{:width$}", r, width = col_widths[j]))
                .collect::<Vec<_>>()
                .join("");
            write!(
                f,
                "{}",
                if self.color_first_row && i == 0 {
                    row.as_str().to_colored()
                } else {
                    row
                }
            )?;
            writeln!(f)?;
        }
        Ok(())
    }
}
