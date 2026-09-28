//! Preserve Python 1.x shlex.split(..., comments=False, posix=True) semantics.
//! This only constructs argv; it never expands or executes shell syntax.
use crate::config::Result;
pub fn split(text: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                } else {
                    word.push(c);
                }
            }
            Some('"') => match c {
                '"' => quote = None,
                '\\' => {
                    let next = chars.next().ok_or("No escaped character")?;
                    if !matches!(next, '"' | '\\') {
                        word.push('\\');
                    }
                    word.push(next);
                }
                _ => word.push(c),
            },
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    started = true;
                }
                '\\' => {
                    word.push(chars.next().ok_or("No escaped character")?);
                    started = true;
                }
                ' ' | '\t' | '\r' | '\n' => {
                    if started {
                        args.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                _ => {
                    word.push(c);
                    started = true;
                }
            },
            _ => unreachable!(),
        }
    }
    if quote.is_some() {
        return Err("No closing quotation".into());
    }
    if started {
        args.push(word);
    }
    Ok(args)
}
#[cfg(test)]
mod tests {
    #[test]
    fn matches_legacy_python_argument_vectors() {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../tests/shlex-vectors.json")).unwrap();
        for row in rows.as_array().unwrap() {
            let actual = super::split(row["input"].as_str().unwrap());
            if row["expected"].is_null() {
                assert!(actual.is_err(), "{row}");
            } else {
                assert_eq!(serde_json::json!(actual.unwrap()), row["expected"], "{row}");
            }
        }
    }
}
