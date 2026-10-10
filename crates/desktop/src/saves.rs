use bevy::prelude::*;
use learning_lord_simulation::Universe;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Resource)]
pub struct Saves {
    pub directory: PathBuf,
    pub status: String,
    files: Vec<PathBuf>,
}

impl Default for Saves {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("saves"),
            status: String::new(),
            files: Vec::new(),
        }
    }
}

impl Saves {
    pub fn load_latest(&mut self) -> io::Result<Option<Universe>> {
        self.files = saved_files(&self.directory)?;
        let Some(path) = self.files.first() else {
            return Ok(None);
        };
        let universe = read_universe(path)?;
        self.status = format!("Loaded {}", path.display());
        Ok(Some(universe))
    }

    pub fn save(&mut self, universe: &Universe, pressed_at: SystemTime) {
        self.status = match write_universe(&self.directory, universe, pressed_at) {
            Ok(path) => format!("Saved {}", path.display()),
            Err(error) => format!("Save failed: {error}"),
        };
        self.refresh();
    }

    pub fn refresh(&mut self) {
        match saved_files(&self.directory) {
            Ok(files) => self.files = files,
            Err(error) => self.status = format!("Cannot list saves: {error}"),
        }
    }
}

fn saved_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".ron.gz"))
            && entry.file_type()?.is_file()
        {
            files.push((entry.metadata()?.modified()?, path));
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    Ok(files.into_iter().map(|(_, path)| path).collect())
}

pub(crate) fn read_universe(path: &Path) -> io::Result<Universe> {
    let mut decoder = flate2::read::MultiGzDecoder::new(fs::File::open(path)?);
    let mut contents = String::new();
    decoder.read_to_string(&mut contents)?;
    Universe::from_save_str(&contents).map_err(io::Error::other)
}

fn write_universe(
    directory: &Path,
    universe: &Universe,
    pressed_at: SystemTime,
) -> io::Result<PathBuf> {
    let contents = universe.to_save_string().map_err(io::Error::other)?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(contents.as_bytes())?;
    let compressed = encoder.finish()?;
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
            stem.clone()
        } else {
            format!("{stem}-{suffix}")
        };
        let path = directory.join(format!("{name}.ron.gz"));
        let temporary = directory.join(format!("{name}.pending"));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        // Publishing a hard link makes complete saves visible without replacing existing files.
        let result = file
            .write_all(&compressed)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::hard_link(&temporary, &path));
        drop(file);
        let cleanup = fs::remove_file(&temporary);
        match result {
            Ok(()) => {
                cleanup?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                cleanup?;
            }
            Err(error) => {
                let _ = cleanup;
                return Err(error);
            }
        }
    }
    unreachable!()
}

#[derive(Component)]
pub(crate) struct FileList;
#[derive(Component)]
pub(crate) struct LoadFile(PathBuf);

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            row_gap: px(8),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ScrollPosition::default(),
        bevy::ui::RelativeCursorPosition::default(),
        FileList,
    ));
}

pub fn refresh_window(
    mut commands: Commands,
    saves: Res<Saves>,
    lists: Query<Entity, With<FileList>>,
    added: Query<Entity, Added<FileList>>,
) {
    for entity in &lists {
        if !saves.is_changed() && !added.contains(entity) {
            continue;
        }
        commands
            .entity(entity)
            .despawn_related::<Children>()
            .with_children(|parent| {
                if saves.files.is_empty() {
                    parent.spawn(crate::text("No saved universes", 14.0, crate::MUTED));
                }
                for path in &saves.files {
                    parent
                        .spawn((
                            Button,
                            LoadFile(path.clone()),
                            Node {
                                padding: UiRect::all(px(10)),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(crate::PANEL),
                        ))
                        .with_children(|button| {
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            button.spawn(crate::text(format!("Load {name}"), 14.0, crate::TEXT));
                        });
                }
            });
    }
}

pub fn handle_load(
    buttons: Query<(Entity, &Interaction, &LoadFile), Changed<Interaction>>,
    mut saves: ResMut<Saves>,
    worker: Res<crate::simulation::SimulationWorker>,
    mut controls: ResMut<crate::Controls>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
) {
    for (entity, interaction, file) in &buttons {
        if *interaction != Interaction::Pressed || controls.restart_pending {
            continue;
        }
        focus.set(entity, bevy::input_focus::FocusCause::Pressed);
        match read_universe(&file.0).and_then(|universe| {
            worker
                .send(crate::simulation::Command::Load(Box::new(universe)))
                .map_err(io::Error::other)
        }) {
            Ok(()) => {
                controls.running = false;
                controls.speed = 1;
                controls.error = None;
                controls.restart_pending = true;
                saves.status = format!("Loaded {}", file.0.display());
            }
            Err(error) => saves.status = format!("Load failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "learning-lord-saves-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn startup_without_saves_returns_none() {
        let directory = directory();
        let mut saves = Saves {
            directory: directory.clone(),
            ..default()
        };
        assert_eq!(saves.load_latest().unwrap(), None);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("unfinished.pending"), "broken").unwrap();
        assert_eq!(saves.load_latest().unwrap(), None);
        assert!(saves.status.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn startup_loads_newest_complete_save_and_reports_failure_without_trying_older() {
        let directory = directory();
        let original = Universe::default();
        let older = write_universe(&directory, &original, UNIX_EPOCH).unwrap();
        let loaded = crate::simulation::new_universe()
            .unwrap()
            .advance(1234)
            .unwrap();
        let newest =
            write_universe(&directory, &loaded, UNIX_EPOCH + Duration::from_secs(1)).unwrap();
        fs::File::open(&older)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(10))
            .unwrap();
        fs::File::open(&newest)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(20))
            .unwrap();
        fs::write(directory.join("unfinished.pending"), "broken").unwrap();
        let mut saves = Saves {
            directory: directory.clone(),
            ..default()
        };
        assert_eq!(saves.load_latest().unwrap(), Some(loaded));
        assert_eq!(saves.files, vec![newest.clone(), older.clone()]);
        assert_eq!(saves.status, format!("Loaded {}", newest.display()));
        fs::write(&newest, "broken").unwrap();
        assert!(saves.load_latest().is_err());
        assert_eq!(read_universe(&older).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn startup_reports_save_directory_errors() {
        let directory = directory();
        fs::write(&directory, "not a directory").unwrap();
        let mut saves = Saves {
            directory: directory.clone(),
            ..default()
        };
        assert!(saves.load_latest().is_err());
        fs::remove_file(directory).unwrap();
    }

    #[test]
    fn files_round_trip_never_overwrite_and_exclude_incomplete_writes() {
        let directory = directory();
        let universe = crate::simulation::new_universe()
            .unwrap()
            .advance(1234)
            .unwrap();
        let time = UNIX_EPOCH + Duration::new(1_800_000_000, 123_456_789);
        let first = write_universe(&directory, &universe, time).unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "universe-1800000000.123456789.ron.gz"
        );
        assert_eq!(read_universe(&first).unwrap(), universe);
        let contents = fs::read(&first).unwrap();
        let second = write_universe(&directory, &Universe::default(), time).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), contents);
        fs::write(directory.join("unfinished.pending"), "broken").unwrap();
        assert_eq!(saved_files(&directory).unwrap().len(), 2);
        fs::write(directory.join("malformed.ron.gz"), "broken").unwrap();
        assert!(read_universe(&directory.join("malformed.ron.gz")).is_err());
        let mut corrupt = fs::read(&first).unwrap();
        corrupt.truncate(corrupt.len() - 4);
        fs::write(directory.join("truncated.ron.gz"), &corrupt).unwrap();
        assert!(read_universe(&directory.join("truncated.ron.gz")).is_err());
        let mut corrupt = fs::read(&first).unwrap();
        let checksum = corrupt.len() - 8;
        corrupt[checksum] ^= 0xff;
        fs::write(directory.join("corrupt.ron.gz"), &corrupt).unwrap();
        assert!(read_universe(&directory.join("corrupt.ron.gz")).is_err());
        assert!(write_universe(&first, &universe, time).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn load_buttons_decode_before_replacing_the_worker_and_leave_failures_untouched() {
        let directory = directory();
        let loaded = Universe::default().advance(4321).unwrap();
        let path = write_universe(&directory, &loaded, SystemTime::now()).unwrap();
        let invalid = directory.join("invalid.ron.gz");
        fs::write(&invalid, "not a save").unwrap();
        let worker =
            crate::simulation::SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(worker)
            .init_resource::<crate::Controls>()
            .init_resource::<bevy::input_focus::InputFocus>()
            .insert_resource(Saves {
                directory: directory.clone(),
                status: String::new(),
                files: vec![path.clone()],
            })
            .add_systems(Update, (refresh_window, handle_load).chain());
        let list = app.world_mut().spawn((Node::default(), FileList)).id();
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&LoadFile>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(app.world().entity(list).get::<Children>().is_some());
        let bad_button = app
            .world_mut()
            .spawn((Interaction::Pressed, LoadFile(invalid)))
            .id();
        app.update();
        assert!(
            app.world()
                .resource::<Saves>()
                .status
                .starts_with("Load failed:")
        );
        assert_eq!(
            app.world()
                .resource::<crate::simulation::SimulationWorker>()
                .snapshot()
                .generation,
            0
        );
        assert!(!app.world().resource::<crate::Controls>().restart_pending);
        app.world_mut()
            .entity_mut(bad_button)
            .insert(Interaction::None);
        app.world_mut()
            .spawn((Interaction::Pressed, LoadFile(path)));
        app.update();
        for _ in 0..100 {
            let snapshot = app
                .world()
                .resource::<crate::simulation::SimulationWorker>()
                .snapshot();
            if snapshot.generation == 1 {
                assert_eq!(snapshot.universe, loaded);
                assert!(app.world().resource::<crate::Controls>().restart_pending);
                fs::remove_dir_all(directory).unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("load was not applied");
    }
}
