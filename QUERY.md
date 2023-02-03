# Query Language

Caveripper has a built-in query language for finding layouts with specific characteristics which is currently used in the `search` and `stats` commands. This language changes frequently as Caveripper is developed, but the fundamentals have remained consistent for a while now.

Queries are written as a sequence of 'clauses' joined by the character '&'. Caveripper will attempt to find a layout matching all clauses. At least the first clause must start with a sublevel to check, and any further clauses will implicitly check that same sublevel until a clause specifies a different sublevel.

Queries require you to use internal names for game entities at the moment. It can be hard to remember everything off the top of your head, so feel free to use the text-only Caveinfo command (CLI: `caveinfo -t`, Discord: `/caveinfo_text`) as necessary.

## Types of Query Clause
- `INTERNAL_NAME </=/> NUM`. Checks the number of the named entity present in each layout. This can include Teki, Treasures, "gate", "hole", "geyser", "ship", the internal name of a room tile, "alcove", "hallway", or "room".
    - Example: `BlackPom > 0` to check for layouts that have at least one Violet Candypop Bud.
- `INTERNAL_NAME straight dist INTERNAL_NAME </=/> NUM`. Checks whether the straight-line distance between the two named entities matches the (in)equality. Note that this is distance 'as the crow flies' rather than distance along carry paths.
- `INTERNAL_NAME carry dist </=/> NUM`. Checks whether the carry distance to the ship through the waypoint graph matches the (in)equality.
- `INTERNAL_NAME gated` or `INTERNAL_NAME not gated`. Checks whether the carry path between the ship and the specified entity has a gate blocking it.
- `ROOM_NAME (+ ENTITY_NAME / CARRYING)* -> <repeated>`. This is a 'room path' query where you can specify a chain of rooms that all must be connected to each other, each optionally containing specific entities. The room and entity names here accept the word "any" as a special case. This query has a lot of uses, so here are some illustrative examples:
    - `bk4 room + hole`: finds a layout where the hole is in a room.
    - `sh6 any + ship -> any + bluekochappy/bey_goma`: finds a layout where the lens bulborb is in a room next to the ship.
    - `fc6 room_north4_1_tsuchi + chess_king_white + chess_queen_black`: finds a fc6 layout where the two treasures are in the small round room.
    - `scx8 any + ship -> alcove + geyser`: finds a layout where the geyser is in an alcove immediately next to the ship.

## Example Queries
- Find a towerless seed: `scx7 minihoudai < 2`
- Find a towerless AND clackerless seed: `scx7 minihoudai < 2 & gk3 castanets = 0`
- Find a TAS FC4: `fc4 any + ship + toy_ring_c_green + be_dama_red + hole`
- Find a silly super large DD5: `dd5 hall > 80`
- Find a really good Concrete Maze seed: `cm2 key carry dist < 500 & hole carry dist < 500`
- Find the same type of SCx7 I got in my 1:34:42 PB: `scx7 any + ship -> room_hitode4x4_tower_3_metal + minihoudai/sinkukan_b -> alcove + denchi_1_black & denchi_1_black straight dist tape_red < 300`
- Find a fully gateless CoS: `cos2 hole not gated & ahiru_head not gated & kan_b_gold not gated & frog/g_futa_titiyas not gated & cos3 sinjyu not gated & kan_nichiro not gated & hole not gated & cos4 hole not gated`

As you can probably see, the query language is extremely powerful and can find very specific layout types if you're willing to craft the right query. Use it well!
