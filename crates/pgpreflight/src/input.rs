use std::{
    fs,
    io::{self, Read},
    path::Path,
};

#[derive(Clone, Copy)]
pub(crate) enum InputFailure {
    Io,
    NotUtf8,
    Empty,
}

pub(crate) fn read_sql(input: &Path) -> Result<String, InputFailure> {
    let bytes = if input.as_os_str() == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|_| InputFailure::Io)?;
        bytes
    } else {
        fs::read(input).map_err(|_| InputFailure::Io)?
    };

    let mut sql = String::from_utf8(bytes).map_err(|_| InputFailure::NotUtf8)?;
    if let Some(without_bom) = sql.strip_prefix('\u{feff}') {
        sql = without_bom.to_owned();
    }
    if is_effectively_empty(&sql) {
        return Err(InputFailure::Empty);
    }

    Ok(sql)
}

fn is_effectively_empty(sql: &str) -> bool {
    let mut offset = 0;

    while offset < sql.len() {
        let rest = &sql[offset..];
        if rest.starts_with("--") {
            offset += 2;
            while offset < sql.len() {
                let character = sql[offset..]
                    .chars()
                    .next()
                    .expect("offset remains on a character boundary");
                offset += character.len_utf8();
                if matches!(character, '\n' | '\r') {
                    break;
                }
            }
            continue;
        }
        if rest.starts_with("/*") {
            let Some(after_comment) = skip_block_comment(sql, offset) else {
                return false;
            };
            offset = after_comment;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("offset remains on a character boundary");
        if character.is_whitespace() || character == ';' {
            offset += character.len_utf8();
            continue;
        }

        return false;
    }

    true
}

fn skip_block_comment(sql: &str, start: usize) -> Option<usize> {
    let mut offset = start + 2;
    let mut depth = 1_u32;

    while offset < sql.len() {
        let rest = &sql[offset..];
        if rest.starts_with("/*") {
            depth = depth.checked_add(1)?;
            offset += 2;
        } else if rest.starts_with("*/") {
            depth -= 1;
            offset += 2;
            if depth == 0 {
                return Some(offset);
            }
        } else {
            let character = rest
                .chars()
                .next()
                .expect("offset remains on a character boundary");
            offset += character.len_utf8();
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::is_effectively_empty;

    #[test]
    fn empty_detection_accepts_nested_comments_and_semicolons() {
        assert!(is_effectively_empty(
            " ; -- line\n /* outer /* nested */ comment */ ; "
        ));
    }

    #[test]
    fn empty_detection_rejects_sql_and_unterminated_comments() {
        assert!(!is_effectively_empty("SELECT 1;"));
        assert!(!is_effectively_empty("/* unfinished"));
    }
}
