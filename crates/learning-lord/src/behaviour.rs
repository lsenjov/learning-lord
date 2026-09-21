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

const DIST_TO_MARKET: u32 = 100;

#[derive(Clone, Debug)]
enum AgentType {
    Animal,
    Citizen,
    Family,
}

#[derive(Clone, Debug)]
struct Agent {
    id: Uuid,
    name: String,
    agent_type: AgentType,
    // Assets and goals are named
    assets: HashMap<String, u32>,
    goals: HashMap<String, u32>,
    loyalties: HashMap<Uuid, u32>,
}

fn random_citizen() -> Agent {
    let uuid = Uuid::new_v4();
    Agent {
        id: uuid,
        name: String::from("Some citizen"),
        agent_type: AgentType::Citizen,
        assets: HashMap::<String, u32>::new(),
        goals: HashMap::<String, u32>::new(),
        loyalties: HashMap::<Uuid, u32>::new(),
    }
}
//impl fmt::Display for Agent {
//    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//        write!(f, "{} ({})", self.name, self.id)
//    }
//}

fn add_random_citizen(m: &mut HashMap<Uuid, Agent>) -> Uuid {
    let cit = random_citizen();
    m.insert(cit.id, cit.clone());
    cit.id
}

fn create_family(m: &mut HashMap<Uuid, Agent>, family_name: String, family_size: u32) {
    let family_id = Uuid::new_v4();
    let family = Agent {
        id: family_id,
        name: String::from(format!("Family {family_name}")),
        agent_type: AgentType::Family,
        assets: HashMap::<String, u32>::new(),
        goals: HashMap::<String, u32>::new(),
        loyalties: HashMap::<Uuid, u32>::new(),
    };
    m.insert(family.id, family);
    for i in 0..family_size {
        println!(
            "Creating family member num {} for family {}",
            i.to_string(),
            family_name
        );
        let cit_id = add_random_citizen(m);
        let Some(cit_agent) = m.get_mut(&cit_id) else {
            continue;
        };
        cit_agent.loyalties.insert(family_id, 50);
        let Some(fam_agent) = m.get_mut(&family_id) else {
            continue;
        };
        fam_agent.loyalties.insert(cit_id, 50);
    }
}

pub fn run_sim() {
    let mut agents = HashMap::<Uuid, Agent>::new();
    add_random_citizen(&mut agents);
    create_family(&mut agents, "Arble".into(), 3);
    create_family(&mut agents, "Orgle".into(), 5);
    println!("{:#?}", agents);
}
