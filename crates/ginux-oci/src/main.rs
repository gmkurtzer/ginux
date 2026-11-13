use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let [command, image, cache_root, destination] = arguments.as_slice() else {
        eprintln!("usage: ginux-image materialize IMAGE CACHE_ROOT DESTINATION");
        return ExitCode::from(64);
    };
    if command != "materialize" {
        eprintln!("ginux-image: unsupported private command");
        return ExitCode::from(64);
    }
    let Some(image) = image.to_str() else {
        eprintln!("ginux-image: image reference is not UTF-8");
        return ExitCode::from(64);
    };
    match ginux_oci::materialize(
        image,
        Path::new(cache_root),
        Path::new(destination),
        std::io::stderr(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ginux-image: {error}");
            ExitCode::from(1)
        }
    }
}
