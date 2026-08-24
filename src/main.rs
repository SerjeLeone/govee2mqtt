use clap::Parser;

fn setup_logger() {
    env_logger::builder()
        .format(|buf, record| {
            use std::io::Write;

            let timestamp = govee::service::timezone::now_rfc3339();
            let level_style = buf.default_level_style(record.level());
            write!(buf, "[{timestamp} ")?;
            write!(buf, "{level_style}{:<5}{level_style:#}", record.level())?;
            if let Some(path) = record.module_path() {
                write!(buf, " {}", path)?;
            }
            writeln!(buf, "] {}", record.args())?;

            // Capture into the log streaming system
            let level_str = format!("{}", record.level());
            let target = record.module_path().unwrap_or("").to_string();
            let message = format!("{}", record.args());
            govee::service::log_capture::push_log(&level_str, &target, &message);

            // Write to rotating log file
            let file_line = format!("[{timestamp} {:<5} {}] {}", record.level(), target, message);
            govee::service::file_logger::write_line(&file_line);

            Ok(())
        })
        .filter_level(log::LevelFilter::Info)
        .parse_env("RUST_LOG")
        .init();
}

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    color_backtrace::install();
    if let Ok(path) = dotenvy::dotenv() {
        eprintln!("Loading environment overrides from {path:?}");
    }

    govee::service::file_logger::init();
    setup_logger();

    let args = govee::Args::parse();
    args.run().await
}
