use learning_lord_simulation::{
    AgentKind, Citizen, SimulationError, Universe, marketplace::Good, storage::GoodsOwner,
};

pub trait HomeStock {
    fn with_home_good(&self, good: Good, units: u64) -> Result<Citizen, SimulationError>;
}

impl HomeStock for Citizen {
    fn with_home_good(&self, good: Good, units: u64) -> Result<Citizen, SimulationError> {
        if self.active_action().is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut goods: Vec<_> = Good::ALL
            .into_iter()
            .map(|item| (item, self.stored_available_units(self.home(), item)))
            .collect();
        goods[good as usize].1 = units;
        Ok(pantry(self.clone(), &goods))
    }
}

pub fn pantry(citizen: Citizen, goods: &[(Good, u64)]) -> Citizen {
    let (mut world, id) = Universe::with_map(citizen.map())
        .with_prices(citizen.prices())
        .with_citizen("Fixture", citizen)
        .unwrap();
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    let home = citizen.home();
    for &(good, units) in goods {
        world = world
            .with_stored_good(home, GoodsOwner::Agent(id), good, units)
            .unwrap();
    }
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen.clone()
}
