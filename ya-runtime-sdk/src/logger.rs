use chrono::{DateTime, Local};
use flexi_logger::{DeferredNow, Record};
use std::env::current_dir;
use std::time::SystemTime;

const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_FILE_LOG_LEVEL: &str = "debug";
const DEFAULT_LOG_DIR: &str = "logs";
const DEFAULT_LOG_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f%z";

pub fn start_file_logger() -> anyhow::Result<flexi_logger::LoggerHandle> {
    let log_dir = current_dir()?.join(DEFAULT_LOG_DIR);
    std::fs::create_dir_all(&log_dir)?;

    Ok(build_logger(Some(DEFAULT_FILE_LOG_LEVEL))?
        .log_to_file(flexi_logger::FileSpec::default().directory(log_dir))
        .duplicate_to_stderr(log_tty_dup_level()?)
        .start()?)
}

pub fn start_logger() -> anyhow::Result<flexi_logger::LoggerHandle> {
    Ok(build_logger(Option::<String>::None)?.start()?)
}

fn build_logger<S: ToString>(log_level: Option<S>) -> anyhow::Result<flexi_logger::Logger> {
    let level = match log_level {
        Some(level) => level.to_string(),
        None => std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string()),
    };

    Ok(flexi_logger::Logger::try_with_str(level)?
        .use_utc()
        .format(log_format)
        .format_for_stderr(flexi_logger::colored_opt_format))
}

fn log_tty_dup_level() -> anyhow::Result<flexi_logger::Duplicate> {
    use flexi_logger::Duplicate;
    use log::LevelFilter;

    let level_filter = flexi_logger::LogSpecification::env_or_parse(DEFAULT_LOG_LEVEL)?
        .module_filters()
        .iter()
        .find(|f| f.module_name.is_none())
        .map(|f| f.level_filter)
        .unwrap_or(LevelFilter::Off);

    Ok(match level_filter {
        LevelFilter::Off => Duplicate::None,
        LevelFilter::Trace => Duplicate::Trace,
        LevelFilter::Debug => Duplicate::Debug,
        LevelFilter::Info => Duplicate::Info,
        LevelFilter::Warn => Duplicate::Warn,
        LevelFilter::Error => Duplicate::Error,
    })
}

fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    //use DateTime::<Local> instead of DateTime::<UTC> to obtain local date
    let now = SystemTime::from(*now.now());
    let local_date = DateTime::<Local>::from(now);
    //format date as following: 2020-08-27T07:56:22.348+02:00 (local date + time zone with milliseconds precision)
    let date_format = local_date.format(DEFAULT_LOG_FORMAT);

    write!(
        w,
        "[{} {:5} {}] {}",
        date_format,
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
}
