#[cfg(feature = "clack-runtime")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: scan_clap <path-to-plugin.clap>")?;
    let records = ghost_host::clack_runtime::scan_clap_file(&path)?;

    if records.is_empty() {
        return Err("CLAP plugin factory returned no descriptors".into());
    }

    for record in records {
        println!(
            "id={} name={} vendor={} version={}",
            record.id,
            record.name,
            record.vendor.as_deref().unwrap_or(""),
            record.version.as_deref().unwrap_or("")
        );
    }

    Ok(())
}

#[cfg(not(feature = "clack-runtime"))]
fn main() {
    eprintln!("scan_clap requires --features clack-runtime");
    std::process::exit(2);
}
