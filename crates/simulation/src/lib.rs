use imbl::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Agent {
    pub name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Universe {
    pub agents: HashMap<AgentId, Agent>,
}
