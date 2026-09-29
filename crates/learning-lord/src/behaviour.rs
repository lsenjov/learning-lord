/* This figures out what goal the agent is trying to do.

The agent could be a number of things - animals, people, a family, the state, the church

These agents may have immediate needs - safety, food, hygiene, drugs (alcohol addiction?)
They may have long term goals - money, prestige

They may have loyalties - a person has loyalty to a family,
  and so will accomplish the family's goals when it does not interfere with their own immediate needs
The same can apply to other things - high loyalty to someone else means they might buy their goods at a higher price, or take jobs for lower prices
This can probably work well within a family - jobs are offerred for work, family members take them for almost no money, because of loyalty.
If the family is too small and there is too much work, jobs may be offerred to those outside the family

So each agent can adjust their own prices over time. How do they calculate against the world though?
The market can list what the lowest sale/highest buy available is? What if it's not available to buy/sell? It becomes worthless/not worth it? Caravans can provide bounds?

Game state becomes immutable, so we can do fast mutation checking on it. We can spin multiple threads out to do it fast.


Each agent has a list of actions it can take. Some are allowed to chain from each other.

Basic production
Goals are primarily to increase wealth. Can it see into the future on production chains?
Action possible:
Buy wheat for 2, sell for 1
Buy bread for 8, sell for 8? Only one bread seller?
Bake bread

If we have no wheat, buying wheat->bake bread->sell bread. But after the bread is made its worth is the same as its monetary value? Make things worth 80% of their value?
Or we just need to look further in the tree. Seeing another buy->bake moves the price up significantly. Other goods won't do it, since they'll sell for slightly less than bought.
But goods creation is fine here.

There's also satisfying food requirements. One goal of family is loyalty to the members' needs. Members get hungry, so we need to buy ingredients. Net worth goes down when eaten, but we know if we don't net worth will go down far more (or members needs will not get satisfied at all). A goal of members is to not get hungry, so eat, so that goal becomes one of the family's.

Need to factor time/skills into it. The amount of time it takes for someone unskilled vs skilled is very different profit margins. Maybe at some point is becomes profitable to do something unskilled, just because of how needed it is.

How does a family decide what's needed to buy together? When does it invest in capital goods?
Maybe an agent occasionally needs a long term plan, where it looks at purchasing capital goods/new buildings/asking for more land. These aren't usually part of day to day planning, but are allowed in these spots for longer-term lookaheads (and much longer chains).

During lookaheads we can mostly avoid changing things in other families - we look at buy/sell price for purchasing/selling. We can isolate down to this for performance and see what the forecast says. We need inventory locally, but outside of that? w/e

So we need game state then. Calculate market prices, and run the sim. Different agents have different action types.

Lookahead a certain amount of time? Prioritise needs first, then past there is whatever.

*/
use im::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
struct Universe {
    agents: HashMap<Uuid, Agent>,
}
impl Universe {
    pub fn new() -> Self {
        Self {
            agents: HashMap::<Uuid, Agent>::new(),
        }
    }
}

// Ticks this agent forward in the universe
trait Tick {
    fn tick(&self, u: Universe, ticks: u64) -> Universe;
}

#[derive(Clone, Debug, PartialEq)]
struct Citizen {}

#[derive(Clone, Debug, PartialEq)]
enum AgentType {
    //Animal,
    Citizen(),
    //Family,
}

#[derive(Clone, Debug, PartialEq)]
struct Agent {
    id: Uuid,
    name: String,
    agent_type: AgentType,
}

fn random_citizen() -> Agent {
    let uuid = Uuid::new_v4();
    Agent {
        id: uuid,
        name: String::from("Some citizen"),
        agent_type: AgentType::Citizen(),
    }
}

fn add_random_citizen_u(u: Universe) -> (Universe, Uuid) {
    let cit = random_citizen();
    let cit_id = cit.id;
    let mut u2 = u.clone();
    u2.agents = u.agents.clone().update(cit.id, cit);
    (u2, cit_id)
}

pub fn run_sim() {
    let mut agents = HashMap::<Uuid, Agent>::new();
    println!("{:#?}", agents);
}

#[cfg(test)]
mod tests {
    use super::Universe;
    use crate::behaviour::add_random_citizen_u;

    #[test]
    fn add_random_citizen_test() {
        let u = Universe::new();
        let u_base = u.clone();

        let (u2, cit_id) = add_random_citizen_u(u.clone());
        assert_eq!(u, u_base); // Ensure immutability
        assert_ne!(u_base, u2); // Things have changed
        assert!(u2.agents.get(&cit_id).unwrap().name.len() > 0) // Name exists
    }
}
