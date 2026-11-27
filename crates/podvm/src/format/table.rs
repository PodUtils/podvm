//! docker's `table` prefix of `--format` (TODO/cli.md T-2087, OR-B40-15):
//! a header line from the names of the fields, and the columns aligned at
//! each tab as Go's `tabwriter` aligns them in docker's CLI (a minimum
//! width of 10, a padding of 3 spaces).

/// The smallest width of a column.
pub const MIN_WIDTH: usize = 10;
/// The spaces after the widest cell of a column.
pub const PADDING: usize = 3;

/// The column name of a field: the name of the verb's own table where it
/// has one (`overrides`), else each word of the name in capitals, as docker
/// writes them (`CreatedAt` is `CREATED AT`).
pub fn header(chain: &[String], overrides: &[(&str, &str)]) -> String {
    let name = chain.join(".");
    if let Some((_, h)) = overrides.iter().find(|(k, _)| *k == name) {
        return h.to_string();
    }
    chain
        .iter()
        .map(|part| words(part).join(" "))
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// The words of a name in camel case: `CreatedAt` gives `Created`, `At`;
/// `ID` stays one word; `IDName` gives `ID`, `Name`.
fn words(name: &str) -> Vec<String> {
    let c: Vec<char> = name.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for i in 0..c.len() {
        let starts = i > 0
            && c[i].is_uppercase()
            && (c[i - 1].is_lowercase()
                || c[i - 1].is_ascii_digit()
                || (c[i - 1].is_uppercase() && c.get(i + 1).is_some_and(|n| n.is_lowercase())));
        if starts && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c[i]);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Align `text` at its tabs. A cell is the text before a tab; the text after
/// the last tab of a line is not a cell. The cells of one column in
/// consecutive lines have one width: the widest plus [`PADDING`], and at
/// least [`MIN_WIDTH`].
pub fn align(text: &str) -> String {
    let lines: Vec<Vec<&str>> = text.split('\n').map(|l| l.split('\t').collect()).collect();
    // The count of cells of each line: each part but the last.
    let cells = |l: &Vec<&str>| l.len() - 1;
    let max_cols = lines.iter().map(cells).max().unwrap_or(0);
    let mut width = vec![vec![0usize; max_cols]; lines.len()];
    for col in 0..max_cols {
        let mut i = 0;
        while i < lines.len() {
            if cells(&lines[i]) <= col {
                i += 1;
                continue;
            }
            // A block: the consecutive lines with a cell in this column.
            let start = i;
            while i < lines.len() && cells(&lines[i]) > col {
                i += 1;
            }
            let w = lines[start..i]
                .iter()
                .map(|l| l[col].chars().count() + PADDING)
                .max()
                .unwrap_or(0)
                .max(MIN_WIDTH);
            for row in width.iter_mut().take(i).skip(start) {
                row[col] = w;
            }
        }
    }
    let mut out = String::with_capacity(text.len());
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for (col, cell) in l.iter().enumerate() {
            out.push_str(cell);
            if col < cells(l) {
                let pad = width[i][col].saturating_sub(cell.chars().count());
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
    }
    out
}
