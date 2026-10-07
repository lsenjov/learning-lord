use bevy::prelude::Resource;
use learning_lord_simulation::Universe;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Resource)]
pub struct DebugExport {
    pub directory: PathBuf,
    pub status: String,
}

impl Default for DebugExport {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("debug"),
            status: String::new(),
        }
    }
}

impl DebugExport {
    pub fn export(&mut self, universe: &Universe, pressed_at: SystemTime) {
        self.status = match write_universe(&self.directory, universe, pressed_at) {
            Ok(path) => format!("Exported {}", path.display()),
            Err(error) => format!("Universe export failed: {error}"),
        };
    }
}

fn write_universe(
    directory: &Path,
    universe: &Universe,
    pressed_at: SystemTime,
) -> io::Result<PathBuf> {
    let timestamp = pressed_at
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    let stem = format!(
        "universe-{}.{:09}",
        timestamp.as_secs(),
        timestamp.subsec_nanos()
    );
    fs::create_dir_all(directory)?;
    for suffix in 0_u64.. {
        let name = if suffix == 0 {
            format!("{stem}.txt")
        } else {
            format!("{stem}-{suffix}.txt")
        };
        let path = directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(
                    file,
                    "Button press: {}.{:09} seconds since Unix epoch (UTC)\n\n{universe:#?}",
                    timestamp.as_secs(),
                    timestamp.subsec_nanos()
                )?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn export_uses_press_time_preserves_snapshot_and_never_overwrites() {
        let directory = std::env::temp_dir().join(format!(
            "learning-lord-export-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let universe = crate::simulation::new_universe().unwrap();
        let universe = universe.advance(1234).unwrap();
        let (id, _) = universe
            .agents()
            .iter()
            .find(|(_, agent)| agent.name == "Ada")
            .unwrap();
        let learning_lord_simulation::AgentKind::Citizen(citizen) = &universe.agents()[id].kind;
        let universe = universe
            .with_tax_rule(
                citizen.home(),
                "Weekly household fee",
                learning_lord_simulation::taxation::TaxKind::FlatFee {
                    payer: *id,
                    coins: 5,
                },
            )
            .unwrap()
            .0;
        let warehouse = universe
            .map()
            .public_place(learning_lord_simulation::locations::Location::Warehouse);
        let universe = universe
            .with_stored_good(
                warehouse,
                learning_lord_simulation::storage::GoodsOwner::Town,
                learning_lord_simulation::marketplace::Good::Wheat,
                17,
            )
            .unwrap();
        let source = universe.clone();
        let pressed_at = UNIX_EPOCH + Duration::new(1_800_000_000, 123_456_789);
        let first = write_universe(&directory, &universe, pressed_at).unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "universe-1800000000.123456789.txt"
        );
        let contents = fs::read_to_string(&first).unwrap();
        assert!(contents.contains(&format!("{universe:#?}")));
        for name in [
            "Ada's home",
            "Bram's mill",
            "Dara's bakery",
            "Eira's weavery",
            "Finn's tailory",
            "garment_condition",
            "Flax",
            "Thread",
            "Cloth",
            "FlaxBlock",
            "FlaxGarment",
            "PlaceId",
            "owner",
            "inventory",
            "Wheat",
            "Flour",
            "Wood",
            "Water",
            "Bread",
            "Town warehouse",
            "town_treasury",
            "storage",
            "Weekly household fee",
            "FlatFee",
            "taxation",
        ] {
            assert!(contents.contains(name), "missing {name}");
        }
        let second = write_universe(&directory, &Universe::default(), pressed_at).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read_to_string(&first).unwrap(), contents);
        let mut exporter = DebugExport {
            directory: first,
            status: String::new(),
        };
        exporter.export(&universe, pressed_at);
        assert!(exporter.status.starts_with("Universe export failed:"));
        assert_eq!(universe, source);
        fs::remove_dir_all(directory).unwrap();
    }
}
