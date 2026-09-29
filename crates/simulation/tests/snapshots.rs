use learning_lord_simulation::{Agent, AgentId, Universe};
use std::thread;

#[test]
fn branches_can_change_on_other_threads_without_changing_shared_snapshots() {
    let universe = Universe {
        agents: (0..128)
            .map(|id| {
                (
                    AgentId(id),
                    Agent {
                        name: format!("Agent {id}"),
                    },
                )
            })
            .collect(),
    };

    let branches = thread::scope(|scope| {
        let tasks = ["First branch", "Second branch"].map(|name| {
            let mut branch = universe.clone();
            let original = &universe;

            scope.spawn(move || {
                branch.agents.get_mut(&AgentId(0)).unwrap().name = name.into();
                branch.agents.remove(&AgentId(1));
                branch.agents.insert(
                    AgentId(128),
                    Agent {
                        name: "New agent".into(),
                    },
                );

                assert_eq!(original.agents[&AgentId(0)].name, "Agent 0");
                branch
            })
        });

        tasks.map(|task| task.join().unwrap())
    });

    assert_eq!(universe.agents.len(), 128);
    assert_eq!(universe.agents[&AgentId(0)].name, "Agent 0");
    assert!(universe.agents.contains_key(&AgentId(1)));
    assert!(!universe.agents.contains_key(&AgentId(128)));
    assert_eq!(branches[0].agents[&AgentId(0)].name, "First branch");
    assert_eq!(branches[1].agents[&AgentId(0)].name, "Second branch");

    for branch in branches {
        assert_eq!(branch.agents.len(), 128);
        assert!(!branch.agents.contains_key(&AgentId(1)));
        assert_eq!(branch.agents[&AgentId(128)].name, "New agent");
        assert_eq!(branch.agents[&AgentId(127)].name, "Agent 127");
    }
}
