# Backbeat Inspector

This is a crate that takes in `.bb` files and "inspects" their chart contents to extract metadata.

## Usage

```sh
cargo add backbeat_inspector backbeat_core
```

Load a `.bb` file and inspect its chart metadata:

```rust,no_run
use backbeat_core::BackbeatFile;
use backbeat_inspector::inspect_bundle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let bundle = BackbeatFile::from_file("path/to/chart.bb")?;
	let inspection = inspect_bundle(&bundle)?;

	println!("{}", inspection.description);
	println!("gamemode: {}", inspection.gamemode);

	Ok(())
}
```
