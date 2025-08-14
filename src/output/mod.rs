use colored::{Color, Colorize};

pub struct ColoredOutput;

impl ColoredOutput {
    pub fn log_header(text: &str) {
        println!("{}", text.color(Color::Blue));
    }

    pub fn log_repo(text: &str) {
        println!("{}", text.color(Color::Green));
    }

    pub fn log_branch(text: &str) {
        println!("  {}", text.color(Color::Cyan));
    }

    pub fn log_path(text: &str) {
        println!("    {}", text.color(Color::BrightBlack));
    }

    pub fn log_status(text: &str) {
        println!("    {}", text.color(Color::BrightBlack));
    }

    pub fn log_summary(text: &str) {
        println!("{}", text.color(Color::Yellow));
    }
}
