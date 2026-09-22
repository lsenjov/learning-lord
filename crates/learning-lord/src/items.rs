/*
Items are interesting...
Items need to do a number of things.
- consumed to fulfil a need (bread)
- consumed as part of a process (flour to make bread)
- used as part of a process (an oven)
- equipped to fulfil a need (clothing, weapon) (warmth, protection needs)
- can degrade over time/through use
- can be repaired
- have a quality (higher quality is worth more and more useful)
 */

use std::collections::HashMap;
use std::sync::LazyLock;

// This is just the item type declarations
struct Item {
    id: String,
}

//static ITEMS: std::collections::HashMap<String, Item> = std::collections::HashMap<String, Item>::new();

static ITEMS: LazyLock<HashMap<String, Item>> = LazyLock::new(|| HashMap::new());
