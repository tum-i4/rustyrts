use env_logger::{
    fmt::{Color, Formatter},
    Builder, Env,
};
use log::{Level, Record};
use std::io::Result;
use std::io::Write;

pub fn create_logger<'a>() -> Builder {
    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("warn"));

    builder.format(colored_record);

    builder
}

fn colored_record(buf: &mut Formatter, record: &Record) -> Result<()> {
    let mut level_style = buf.style();

    match record.level() {
        Level::Error => level_style.set_color(Color::Red).set_bold(true),
        Level::Warn => level_style.set_color(Color::Yellow).set_bold(true),
        Level::Info => level_style.set_color(Color::Magenta).set_bold(true),
        Level::Debug => level_style.set_color(Color::Blue).set_bold(true),
        Level::Trace => level_style.set_color(Color::Cyan).set_bold(true),
    };

    writeln!(
        buf,
        "     {}: {}",
        level_style.value(record.level()),
        record.args()
    )
}
