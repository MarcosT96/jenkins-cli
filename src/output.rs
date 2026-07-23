//! Colorized output helpers.
//!
//! Handlers build a `serde_json::Value` and hand it to [`print_value`], the
//! same rendering rule used for every command so there is no bespoke
//! `Display` per endpoint. `owo-colors` auto-disables ANSI when stdout is not
//! a TTY.

use owo_colors::OwoColorize;
use serde_json::Value;

/// Print a JSON value: object entries as `Key: value` (key cyan, value
/// yellow), arrays/objects recursing, scalars printed directly.
pub fn print_value(value: &Value) {
    print_inner(value, "white");
}

fn print_inner(value: &Value, color: &str) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                if val.is_object() || val.is_array() {
                    print_inner(val, color);
                } else {
                    print!("{}", format!("{}: ", ucfirst(key)).cyan());
                    println!("{}", scalar_to_string(val).yellow());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                print_inner(item, color);
            }
        }
        scalar => print_colored(&scalar_to_string(scalar), color),
    }
}

/// Print a plain line in one of the palette colors.
pub fn line(text: &str, color: &str) {
    print_colored(text, color);
}

/// Print with no trailing newline, used by wait-loop progress dots.
pub fn inline(text: &str, color: &str) {
    match color {
        "red" => print!("{}", text.red()),
        "green" => print!("{}", text.green()),
        "yellow" => print!("{}", text.yellow()),
        "blue" => print!("{}", text.blue()),
        "magenta" => print!("{}", text.magenta()),
        "cyan" => print!("{}", text.cyan()),
        "gray" => print!("{}", text.bright_black()),
        _ => print!("{text}"),
    }
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn print_colored(text: &str, color: &str) {
    match color {
        "red" => println!("{}", text.red()),
        "green" => println!("{}", text.green()),
        "yellow" => println!("{}", text.yellow()),
        "blue" => println!("{}", text.blue()),
        "magenta" => println!("{}", text.magenta()),
        "cyan" => println!("{}", text.cyan()),
        "gray" => println!("{}", text.bright_black()),
        _ => println!("{text}"),
    }
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn ucfirst(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ucfirst_capitalizes() {
        assert_eq!(ucfirst("name"), "Name");
        assert_eq!(ucfirst(""), "");
    }
}
