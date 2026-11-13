//! A port of the layout "Judge" from JHawk's CaveGen (Judge.java).
//!
//! The Judge gives each layout a heuristic score, where lower is better. Most
//! judge types estimate how many seconds a layout takes to complete for a
//! particular kind of run:
//!
//! - `score`: sum of the game's own unit/door scores for every treasure, plus a
//!   large penalty for each missing treasure.
//! - `pod`: Pikmin 2 "Pay off Debt" speedruns (seconds, adjusted for pokos).
//! - `at`: "All Treasures" speedruns (seconds, with penalties for missing
//!   purple candypops and treasures).
//! - `attk`: Challenge Mode score attack (estimated final score, negated so
//!   that lower is better).
//! - `cmat`: Challenge Mode all treasures (seconds).
//! - `key`: Challenge Mode, time to get the key and reach the exit (seconds).
//! - `colossal`: Colossal Caverns.
//! - `mapunitcount`: the number of map units in the layout.
//!
//! The code follows CaveGen closely, including its f32 arithmetic, so scores
//! should match CaveGen's to within rounding. Notes mark places where CaveGen's
//! behavior is kept even though it looks unintended.

mod graph;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    rc::Rc,
    str::FromStr,
    sync::OnceLock,
};

use graph::{Graph, INF, V3, dist};

use crate::{
    caveinfo::{CaveInfo, RoomType},
    layout::{Layout, SpawnObject},
};

/// Which heuristic to score layouts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JudgeType {
    Score,
    Pod,
    At,
    Attk,
    Cmat,
    Key,
    Colossal,
    MapUnitCount,
}

impl JudgeType {
    pub const ALL: [JudgeType; 8] = [
        JudgeType::Score,
        JudgeType::Pod,
        JudgeType::At,
        JudgeType::Attk,
        JudgeType::Cmat,
        JudgeType::Key,
        JudgeType::Colossal,
        JudgeType::MapUnitCount,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            JudgeType::Score => "score",
            JudgeType::Pod => "pod",
            JudgeType::At => "at",
            JudgeType::Attk => "attk",
            JudgeType::Cmat => "cmat",
            JudgeType::Key => "key",
            JudgeType::Colossal => "colossal",
            JudgeType::MapUnitCount => "mapunitcount",
        }
    }
}

impl Display for JudgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for JudgeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "score" => Ok(JudgeType::Score),
            "pod" => Ok(JudgeType::Pod),
            "at" => Ok(JudgeType::At),
            "attk" | "sa" | "scoreattack" => Ok(JudgeType::Attk),
            "cmat" => Ok(JudgeType::Cmat),
            "key" => Ok(JudgeType::Key),
            "colossal" => Ok(JudgeType::Colossal),
            "mapunitcount" => Ok(JudgeType::MapUnitCount),
            _ => Err(format!(
                "Unknown judge type '{s}'. Expected one of: {}",
                JudgeType::ALL.map(|j| j.name()).join(", ")
            )),
        }
    }
}

// ----------------------------------------------------------------------------
// Data tables (from CaveGen's files/gc/config folder)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
struct CarryInfo {
    min_carry: i32,
    max_carry: i32,
    pokos: i32,
    depth: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct TekiDifficulty {
    sec: i32,
    piki: i32,
}

#[derive(Debug, Clone, Default)]
struct ChallengeStage {
    piki_counts: [i32; 21],
    spicy: i32,
    floor_times: Vec<i32>,
}

struct JudgeData {
    carry: HashMap<String, CarryInfo>,
    difficulty: HashMap<String, TekiDifficulty>,
    challenge: HashMap<String, ChallengeStage>,
}

fn data() -> &'static JudgeData {
    static DATA: OnceLock<JudgeData> = OnceLock::new();
    DATA.get_or_init(|| {
        // name,minCarry,maxCarry,seeds,seedsMin,pokos,depth,height
        let carry = include_str!("../../../resources/jhawk_judge/treasure_teki_config.csv")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.trim().split(',').collect();
                (
                    f[0].to_ascii_lowercase(),
                    CarryInfo {
                        min_carry: f[1].parse().unwrap(),
                        max_carry: f[2].parse().unwrap(),
                        pokos: f[5].parse().unwrap(),
                        depth: f[6].parse().unwrap(),
                        height: f[7].parse().unwrap(),
                    },
                )
            })
            .collect();
        // id,hexId,commonName,internalName,sec,piki
        let difficulty = include_str!("../../../resources/jhawk_judge/teki_difficulty_judge.csv")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.trim().split(',').collect();
                (
                    f[3].to_ascii_lowercase(),
                    TekiDifficulty {
                        sec: f[4].parse().unwrap(),
                        piki: f[5].parse().unwrap(),
                    },
                )
            })
            .collect();
        // CHn,21 piki counts,bitter,spicy,floor_count,floor times...
        let challenge = include_str!("../../../resources/jhawk_judge/challenge_stages.csv")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| {
                let f: Vec<&str> = l.trim().split(',').collect();
                let mut piki_counts = [0; 21];
                for (i, c) in piki_counts.iter_mut().enumerate() {
                    *c = f[1 + i].parse().unwrap();
                }
                (
                    f[0].to_string(),
                    ChallengeStage {
                        piki_counts,
                        spicy: f[23].parse().unwrap(),
                        floor_times: f[25..].iter().map(|t| t.parse().unwrap()).collect(),
                    },
                )
            })
            .collect();
        JudgeData {
            carry,
            difficulty,
            challenge,
        }
    })
}

// CaveGen crashes when a name is missing from its tables; here it counts as zero.
fn carry_info(name: &str) -> CarryInfo {
    data().carry.get(&name.to_ascii_lowercase()).copied().unwrap_or_default()
}
fn pokos(name: &str) -> i32 {
    carry_info(name).pokos
}
fn difficulty(name: &str) -> TekiDifficulty {
    data().difficulty.get(&name.to_ascii_lowercase()).copied().unwrap_or_default()
}

fn set(names: &'static str) -> HashSet<&'static str> {
    names.split(',').collect()
}

struct NameSets {
    plants: HashSet<&'static str>,
    hazards: HashSet<&'static str>,
    no_carcass: HashSet<&'static str>,
    poms: HashSet<&'static str>,
    ignore_treasures_pod: HashSet<&'static str>,
    optional_treasures_pod: HashSet<&'static str>,
    purple20: HashSet<&'static str>,
}

fn names() -> &'static NameSets {
    static NAMES: OnceLock<NameSets> = OnceLock::new();
    NAMES.get_or_init(|| NameSets {
        plants: set("ooinu_s,ooinu_l,wakame_s,wakame_l,kareooinu_s,kareooinu_l,daiodored,daiodogreen,clover,hikarikinoko,tanpopo,zenmai,nekojarashi,tukushi,magaret,watage,chiyogami"),
        hazards: set("gashiba,hiba,elechiba,rock"),
        no_carcass: set("wealthy,fart,kogane,mar,hanachirashi,damagumo,bigfoot,bigtreasure,qurione,baby,bomb,egg,kurage,onikurage,bombotakara,blackman,tyre,houdai,ooinu_s,ooinu_l,wakame_s,wakame_l,kareooinu_s,kareooinu_l,daiodored,daiodogreen,clover,hikarikinoko,tanpopo,zenmai,nekojarashi,tukushi,magaret,watage,chiyogami,gashiba,hiba,elechiba,rock,bluepom,redpom,yellowpom,blackpom,whitepom,randpom,pom"),
        poms: set("bluepom,redpom,yellowpom,blackpom,whitepom,randpom,pom"),
        ignore_treasures_pod: set("kinoko_doku,bird_hane,futa_a_silver,flower_red,flower_blue,g_futa_kyodo,dia_a_green,makigai,mojiban,nut,dashboots,cookie_m_l,bane_red,chocolate,tape_blue"),
        optional_treasures_pod: set("chess_queen_white,gum_tape_s,chess_king_white,chess_queen_black,leaf_normal,locket"),
        purple20: set("EC-2,FC-1,HoB-2,CoS-2,GK-2,SR-2"),
    })
}

// ----------------------------------------------------------------------------
// Constants (see the notes in Judge.java for where these come from)
// ----------------------------------------------------------------------------

const OLIMAR_SPEED_UNITS_NO_RUSH: f32 = 1.17;
const OLIMAR_SPEED_UNITS_RUSH: f32 = 1.17 * 205.0 / 160.0;
const CARRY_MULTIPLIER: f32 = 8.5;
const GATE_LIFE_MULTIPLIER: f32 = 2.0;
const GEYSER_HEALTH: f32 = 2700.0;
const DIGGING_HEALTH: f32 = 3900.0;
const LOAD_TIME_SECONDS: f32 = 0.5;
const LOAD_TIME_SECONDS_STORY: f32 = 3.0;

// ----------------------------------------------------------------------------
// Per-sublevel setup (CaveGen's setupJudge)
// ----------------------------------------------------------------------------

/// Scores layouts of one sublevel. Create one per sublevel and judge type.
#[derive(Debug, Clone)]
pub struct JhawkJudge {
    pub judge_type: JudgeType,
    /// e.g. "BK-4" or "CH12-1"
    sublevel_id: String,
    cave_short_name: String,
    floor: usize,
    expected_num_treasures: i32,
    expected_num_purple_candypop: i32,
    expected_num_pokos_pod: i32,
    piki_count: i32,
    hole_work_multiplier: f32,
    geyser_work_multiplier: f32,
    candypop_work_multiplier: f32,
    piki_carry_value: f32,
    piki_attack_value: f32,
    piki_strength_value: f32,
    piki_dig_value: f32,
    fastest_piki_carry_value: f32,
    olimar_speed_cm: f32,
    challenge_time: i32,
    hole_clogged: bool,
}

impl JhawkJudge {
    pub fn new(judge_type: JudgeType, caveinfo: &CaveInfo) -> Self {
        let n = names();
        let cave_short_name = caveinfo.cave_cfg.shortened_names.first().cloned().unwrap_or_default();
        let floor = caveinfo.floor_num as usize + 1;
        let sublevel_id = format!("{cave_short_name}-{floor}");

        let mut expected_num_treasures = 0;
        let mut expected_num_pokos_pod = 0;
        let mut expected_max_carry = 0;
        let mut add_treasure = |name: &str, min: i32| {
            let name = name.to_ascii_lowercase();
            expected_num_treasures += min;
            expected_max_carry += carry_info(&name).max_carry;
            if !n.ignore_treasures_pod.contains(name.as_str()) && !n.optional_treasures_pod.contains(name.as_str()) {
                expected_num_pokos_pod += pokos(&name);
            } else if judge_type == JudgeType::Pod {
                expected_max_carry -= carry_info(&name).max_carry;
            }
        };
        for item in caveinfo.item_info.iter() {
            add_treasure(&item.internal_name, item.min_amount as i32);
        }
        // spawnTekiConsolidated is TekiInfo followed by CapInfo.
        for (carrying, min) in caveinfo
            .teki_info
            .iter()
            .map(|t| (&t.carrying, t.minimum_amount))
            .chain(caveinfo.cap_info.iter().map(|t| (&t.carrying, t.minimum_amount)))
        {
            if let Some(c) = carrying {
                add_treasure(c, min as i32);
            }
        }
        match sublevel_id.as_str() {
            "CH8-1" | "CH21-1" => expected_num_treasures += 3,
            "CH29-1" => expected_num_treasures -= 1,
            _ => {}
        }

        let mut expected_num_queen_candypop = 0;
        let mut expected_num_purple_candypop = 0;
        for (name, min) in caveinfo
            .teki_info
            .iter()
            .map(|t| (&t.internal_name, t.minimum_amount))
            .chain(caveinfo.cap_info.iter().map(|t| (&t.internal_name, t.minimum_amount)))
        {
            if name.eq_ignore_ascii_case("randpom") {
                expected_num_queen_candypop += min as i32;
            }
            if name.eq_ignore_ascii_case("blackpom") {
                expected_num_purple_candypop += min as i32;
            }
        }
        let previous_num_queen_candypop = match sublevel_id.as_str() {
            "CH6-2" => 1,
            "CH26-2" => 3,
            "CH26-3" => 8,
            "CH30-4" | "CH30-5" => 3,
            _ => 0,
        };

        let mut piki_count;
        let mut piki_carry_value;
        let mut piki_attack_value;
        let mut piki_strength_value;
        let mut piki_dig_value;
        let mut fastest_piki_carry_value;
        let mut challenge_time = 0;
        if let Some(stage) = data().challenge.get(&cave_short_name) {
            let mut carry_sum = 0.0f32;
            let mut attack_sum = 0.0f32;
            let mut strength_sum = 0.0f32;
            let mut dig_sum = 0.0f32;
            fastest_piki_carry_value = 0.0f32;
            piki_count = 0;
            let spicy = stage.spicy > 0;
            for i in 0..18usize {
                let num = stage.piki_counts[i];
                piki_count += num;
                let happa = if i % 3 == 2 || spicy {
                    1.0
                } else if i % 3 == 1 {
                    0.5
                } else {
                    0.0
                };
                let color = if i / 3 == 3 {
                    0.6
                } else if i / 3 == 4 {
                    3.0
                } else {
                    1.0
                };
                let carry_val: f32 = happa + color;
                carry_sum += num as f32 * carry_val;
                attack_sum += (num
                    * if i / 3 == 3 || spicy {
                        20
                    } else if i / 3 == 1 {
                        15
                    } else {
                        10
                    }) as f32;
                strength_sum += (num * if i / 3 == 3 { 10 } else { 1 }) as f32;
                dig_sum += (num
                    * if i / 3 == 3 || spicy {
                        20
                    } else if i / 3 == 1 {
                        15
                    } else {
                        10
                    }) as f32;
                if num > 0 {
                    fastest_piki_carry_value = fastest_piki_carry_value.max(carry_val);
                }
            }
            let extra_piki = (expected_num_queen_candypop + previous_num_queen_candypop) * 8;
            piki_count += extra_piki;
            carry_sum += (extra_piki * if spicy { 2 } else { 1 }) as f32;
            attack_sum += (extra_piki * if spicy { 18 } else { 15 }) as f32;
            strength_sum += extra_piki as f32;
            piki_carry_value = carry_sum / piki_count as f32;
            piki_attack_value = attack_sum / piki_count as f32;
            piki_strength_value = strength_sum / piki_count as f32;
            piki_dig_value = dig_sum / piki_count as f32;
            challenge_time = stage.floor_times.get(floor - 1).copied().unwrap_or(0);
        } else {
            piki_count = 60;
            piki_carry_value = 2.5;
            piki_attack_value = 13.0;
            piki_strength_value = 1.0;
            piki_dig_value = 13.0;
            fastest_piki_carry_value = 4.0;
        }

        let mut rush_boots = false;
        if judge_type == JudgeType::At {
            let whites = match cave_short_name.as_str() {
                "EC" => 0,
                "HoB" => 15,
                "WFG" => {
                    if floor >= 3 {
                        15
                    } else {
                        0
                    }
                }
                _ => {
                    rush_boots = true;
                    35
                }
            };
            piki_count = 80;
            expected_max_carry += 1;
            piki_carry_value = (2.0 * whites.min(expected_max_carry) as f32 + 2.0 * expected_max_carry as f32) / expected_max_carry as f32;
            piki_attack_value = 18.0;
            piki_strength_value = 1.0;
            piki_dig_value = 16.0;
        }
        if judge_type == JudgeType::Pod {
            let whites = match cave_short_name.as_str() {
                "EC" | "HoB" => 0,
                "WFG" => {
                    if floor >= 3 {
                        15
                    } else {
                        0
                    }
                }
                "SH" | "BK" => 20,
                _ => 30,
            };
            rush_boots = false;
            piki_count = 60;
            expected_max_carry += 1;
            piki_carry_value = (2.0 * whites.min(expected_max_carry) as f32 + 2.0 * expected_max_carry as f32) / expected_max_carry as f32;
            piki_attack_value = 13.0;
            piki_strength_value = 1.0;
            piki_dig_value = 13.0;
        }

        let olimar_speed_cm = if rush_boots {
            OLIMAR_SPEED_UNITS_RUSH * 170.0
        } else {
            OLIMAR_SPEED_UNITS_NO_RUSH * 170.0
        };

        // CaveGen: the caveinfo's plugged-exit flag, except on final floors where
        // only Challenge Mode geysers are clogged.
        let hole_clogged = if caveinfo.is_final_floor {
            cave_short_name.starts_with("CH")
        } else {
            caveinfo.exit_plugged
        };

        JhawkJudge {
            judge_type,
            sublevel_id,
            cave_short_name,
            floor,
            expected_num_treasures,
            expected_num_purple_candypop,
            expected_num_pokos_pod,
            piki_count,
            hole_work_multiplier: 1.0,
            geyser_work_multiplier: 10.0,
            candypop_work_multiplier: 1.0,
            piki_carry_value,
            piki_attack_value,
            piki_strength_value,
            piki_dig_value,
            fastest_piki_carry_value,
            olimar_speed_cm,
            challenge_time,
            hole_clogged,
        }
    }
}

// ----------------------------------------------------------------------------
// The layout, flattened into CaveGen's terms
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Sp {
    pos: V3,
    /// Map unit index, or None for spawn points on doors (seams).
    unit: Option<usize>,
    is_alcove: bool,
    /// Index of the spawn point in its map unit's list.
    list_idx: usize,
    /// The map unit's score, or the door's score for seams.
    unit_score: i32,
    door_score: i32,
}

#[derive(Debug, Clone)]
struct Teki {
    /// Name as CaveGen has it, e.g. "Fkabuto".
    name: String,
    item_inside: Option<String>,
    sp: usize,
    pos: V3,
}

#[derive(Debug, Clone)]
struct Item {
    name: String,
    sp: usize,
}

#[derive(Debug, Clone)]
struct Gate {
    life: f32,
    sp: usize,
}

struct Placed {
    sps: Vec<Sp>,
    tekis: Vec<Teki>,
    items: Vec<Item>,
    gates: Vec<Gate>,
    start: usize,
    hole: Option<usize>,
    geyser: Option<usize>,
    onions: Vec<usize>,
    /// Positions of each map unit's first door (used for alcove gate checks).
    first_door_pos: Vec<Option<V3>>,
}

fn v3(p: crate::point::Point<3, f32>) -> V3 {
    V3::new(p[0], p[1], p[2])
}

fn teki_name(internal_name: &str, stationary: bool) -> String {
    if stationary {
        // Undo the normalization of "Fkabuto" -> "Kabuto" and "FminiHoudai" -> "MiniHoudai".
        let mut c = internal_name.chars();
        match c.next() {
            Some(first) => format!("F{}{}", first.to_ascii_lowercase(), c.as_str()),
            None => String::new(),
        }
    } else {
        internal_name.to_string()
    }
}

impl Placed {
    fn from_layout(layout: &Layout) -> Placed {
        let mut placed = Placed {
            sps: Vec::new(),
            tekis: Vec::new(),
            items: Vec::new(),
            gates: Vec::new(),
            start: 0,
            hole: None,
            geyser: None,
            onions: Vec::new(),
            first_door_pos: Vec::new(),
        };

        for (ui, unit) in layout.map_units.iter().enumerate() {
            placed.first_door_pos.push(unit.doors.first().map(|d| v3(d.borrow().center())));
            for (si, sp) in unit.spawnpoints.iter().enumerate() {
                if sp.contains.is_empty() {
                    continue;
                }
                let id = placed.sps.len();
                let sp_pos = v3(sp.pos);
                placed.sps.push(Sp {
                    pos: sp_pos,
                    unit: Some(ui),
                    is_alcove: matches!(unit.unit.room_type, RoomType::DeadEnd),
                    list_idx: si,
                    unit_score: unit.total_score,
                    door_score: 0,
                });
                for so in sp.contains.iter() {
                    match so {
                        SpawnObject::Teki(info, offset) => placed.tekis.push(Teki {
                            name: teki_name(&info.internal_name, info.stationary),
                            item_inside: info.carrying.clone(),
                            sp: id,
                            pos: sp_pos.add(v3(*offset)),
                        }),
                        SpawnObject::CapTeki(info, num) => {
                            // CaveGen raises falling cap teki by 100 units.
                            let pos = if info.is_falling() && !info.is_candypop() {
                                sp_pos.add(V3::new(0.0, 100.0, 0.0))
                            } else {
                                sp_pos
                            };
                            for _ in 0..*num {
                                placed.tekis.push(Teki {
                                    name: teki_name(&info.internal_name, info.stationary),
                                    item_inside: info.carrying.clone(),
                                    sp: id,
                                    pos,
                                });
                            }
                        }
                        SpawnObject::Item(info) => placed.items.push(Item {
                            name: info.internal_name.clone(),
                            sp: id,
                        }),
                        SpawnObject::Hole(_) => placed.hole = Some(id),
                        SpawnObject::Geyser(_) => placed.geyser = Some(id),
                        SpawnObject::Ship => placed.start = id,
                        SpawnObject::Onion(_) => placed.onions.push(id),
                        SpawnObject::Gate(..) => {}
                    }
                }
            }
        }

        // Seam spawn points are shared by the two doors of a seam.
        let mut seen = HashSet::new();
        for unit in layout.map_units.iter() {
            for door in unit.doors.iter() {
                let door = door.borrow();
                if !seen.insert(Rc::as_ptr(&door.seam_spawnpoint)) {
                    continue;
                }
                let Some(so) = door.seam_spawnpoint.as_ref() else { continue };
                let id = placed.sps.len();
                let pos = v3(door.center());
                placed.sps.push(Sp {
                    pos,
                    unit: None,
                    is_alcove: false,
                    list_idx: 0,
                    unit_score: 0,
                    door_score: door.door_score.max(0),
                });
                match so {
                    SpawnObject::Teki(info, _) => placed.tekis.push(Teki {
                        name: teki_name(&info.internal_name, info.stationary),
                        item_inside: info.carrying.clone(),
                        sp: id,
                        pos,
                    }),
                    SpawnObject::Gate(info, _) => placed.gates.push(Gate { life: info.health, sp: id }),
                    _ => {}
                }
            }
        }
        placed
    }
}

// ----------------------------------------------------------------------------
// Judging
// ----------------------------------------------------------------------------

struct Ctx<'a> {
    j: &'a JhawkJudge,
    p: Placed,
    g: Graph,
}

impl Ctx<'_> {
    fn sp_pos(&self, sp: usize) -> V3 {
        self.p.sps[sp].pos
    }
    fn dist_to_start(&mut self, sp: usize) -> f32 {
        let pos = self.sp_pos(sp);
        self.g.dist_to_start(pos)
    }
    fn sp_dist(&self, a: usize, b: usize) -> f32 {
        dist(self.sp_pos(a), self.sp_pos(b))
    }
    fn mark(&mut self, sp: usize) {
        let pos = self.sp_pos(sp);
        self.g.mark_carryable(pos);
    }

    /// CaveGen's computeWaypointCarryableGraph.
    fn compute_carryable(&mut self, config: &str) {
        let n = names();
        if config.contains('h') {
            if let Some(h) = self.p.hole {
                self.mark(h);
            }
            if let Some(h) = self.p.geyser {
                self.mark(h);
            }
        }
        let mut to_mark = Vec::new();
        if config.contains('t') {
            to_mark.extend(self.p.items.iter().map(|i| i.sp));
            to_mark.extend(self.p.tekis.iter().filter(|t| t.item_inside.is_some()).map(|t| t.sp));
        }
        if config.contains('c') {
            // CaveGen checks the name without lowercasing it here, so every teki counts.
            to_mark.extend(
                self.p
                    .tekis
                    .iter()
                    .filter(|t| !n.no_carcass.contains(t.name.as_str()))
                    .map(|t| t.sp),
            );
        }
        for (flag, target) in [('k', "key"), ('m', "map01")] {
            if config.contains(flag) {
                to_mark.extend(self.p.items.iter().filter(|i| i.name.eq_ignore_ascii_case(target)).map(|i| i.sp));
                to_mark.extend(
                    self.p
                        .tekis
                        .iter()
                        .filter(|t| t.item_inside.as_ref().is_some_and(|c| c.eq_ignore_ascii_case(target)))
                        .map(|t| t.sp),
                );
            }
        }
        if config.contains('p') {
            to_mark.extend(
                self.p
                    .tekis
                    .iter()
                    .filter(|t| t.name.eq_ignore_ascii_case("blackpom"))
                    .map(|t| t.sp),
            );
        }
        for sp in to_mark {
            self.mark(sp);
        }
        self.g.propagate_carryable();
    }

    fn teki_in_the_way(&mut self, t: usize) -> bool {
        let teki = &self.p.tekis[t];
        let sp = &self.p.sps[teki.sp];
        let Some(unit) = sp.unit else {
            let pos = sp.pos;
            return self.g.is_carryable(pos);
        };
        let p = teki.pos;
        for k in 0..self.g.unit_wps[unit].len() {
            let w = self.g.unit_wps[unit][k];
            let wp = &self.g.wps[w];
            if wp.is_start || !wp.carryable_behind {
                continue;
            }
            let Some(back) = wp.back else { continue };
            if self.g.wps[back].unit != unit {
                continue;
            }
            let (a, b) = (wp.pos, self.g.wps[back].pos);
            if self.g.point_to_segment_dist(p, a, b, 0.0, 0.0) < 120.0 {
                return true;
            }
        }
        false
    }

    fn gate_in_the_way(&mut self, gate: usize) -> bool {
        let pos = self.sp_pos(self.p.gates[gate].sp);
        self.g.is_carryable(pos)
    }

    fn gate_cost(&self, life: f32, wall_count: f32) -> f32 {
        (3.0 + life * GATE_LIFE_MULTIPLIER / (self.j.piki_attack_value * wall_count)).max(6.0)
    }

    fn dig_time(&self, name: &str) -> f32 {
        let c = carry_info(name);
        if c.depth > 0.0 {
            (c.depth / c.height).min(1.0) * DIGGING_HEALTH / self.j.piki_dig_value
        } else {
            0.0
        }
    }

    /// Carry speed denominator: 220 + 180 * (carry_value * carry - min_carry + 1) / max_carry
    fn carry_speed(carry_value: f32, carry: i32, min_carry: i32, max_carry: i32) -> f32 {
        220.0f32 + 180.0f32 * (carry_value * carry as f32 - min_carry as f32 + 1.0) / max_carry as f32
    }

    /// Prim's algorithm over the ship and these spawn points, in captain walking seconds.
    fn walking_mst(&mut self, targets: &[usize]) -> f32 {
        let start = self.p.start;
        let mut visited = vec![start];
        let mut unvisited: Vec<usize> = Vec::new();
        for &t in targets {
            if !unvisited.contains(&t) && t != start {
                unvisited.push(t);
            }
        }
        let mut walking = 0.0f32;
        while !unvisited.is_empty() {
            let mut min_dist = INF;
            let mut min_u = 0;
            for vi in 0..visited.len() {
                for ui in 0..unvisited.len() {
                    let (v, u) = (visited[vi], unvisited[ui]);
                    let d = if v == start { self.dist_to_start(u) } else { self.sp_dist(v, u) };
                    if d < min_dist {
                        min_u = ui;
                        min_dist = d;
                    }
                }
            }
            walking += min_dist / self.j.olimar_speed_cm;
            visited.push(unvisited.remove(min_u));
        }
        walking
    }

    /// Units are pikmin*seconds. Used by attk and cmat.
    fn work_function(&mut self, name: &str, sp: Option<usize>) -> f32 {
        let Some(sp) = sp else { return 0.0 };
        let n = names();
        let j = self.j;
        let name = name.to_ascii_lowercase();
        let d = self.dist_to_start(sp);
        match name.as_str() {
            "hole" => return d / j.olimar_speed_cm * j.hole_work_multiplier,
            "geyser" => return d / j.olimar_speed_cm * j.geyser_work_multiplier,
            "egg" => {
                return 10.0 * d / j.olimar_speed_cm
                    + 10.0 * LOAD_TIME_SECONDS
                    + 10.0 * d * CARRY_MULTIPLIER / (220.0f32 + 180.0f32 * j.piki_carry_value);
            }
            _ => {}
        }
        if n.poms.contains(name.as_str()) {
            return d / j.olimar_speed_cm * j.candypop_work_multiplier;
        }
        if n.no_carcass.contains(name.as_str()) {
            return 0.0;
        }
        let c = carry_info(&name);
        let carry = (c.min_carry as f32 / j.piki_strength_value).ceil() as i32;
        let dig = self.dig_time(&name);
        carry as f32 * LOAD_TIME_SECONDS
            + dig
            + carry as f32 * d / j.olimar_speed_cm
            + carry as f32 * d * CARRY_MULTIPLIER / Self::carry_speed(j.piki_carry_value, carry, c.min_carry, c.max_carry)
    }
}

impl JhawkJudge {
    /// Scores a layout. Lower is better.
    pub fn judge(&self, layout: &Layout) -> f64 {
        let placed = Placed::from_layout(layout);
        let start_pos = placed.sps[placed.start].pos;
        let graph = Graph::build(layout, start_pos);
        let mut ctx = Ctx {
            j: self,
            p: placed,
            g: graph,
        };
        match self.judge_type {
            JudgeType::Score => self.judge_score(&mut ctx),
            JudgeType::Pod => self.judge_pod(&mut ctx),
            JudgeType::At => self.judge_at(&mut ctx),
            JudgeType::Attk => self.judge_attk(&mut ctx),
            JudgeType::Cmat => self.judge_cmat(&mut ctx),
            JudgeType::Key => self.judge_key(&mut ctx),
            JudgeType::Colossal => self.judge_colossal(&mut ctx),
            JudgeType::MapUnitCount => layout.map_units.len() as f64,
        }
    }

    fn judge_score(&self, c: &mut Ctx) -> f64 {
        let mut score = 0f64;
        let mut treasure_count = 0;
        for i in c.p.items.iter() {
            score += c.p.sps[i.sp].unit_score as f64;
            treasure_count += 1;
        }
        for t in c.p.tekis.iter().filter(|t| t.item_inside.is_some()) {
            let sp = &c.p.sps[t.sp];
            score += if sp.unit.is_none() { sp.door_score } else { sp.unit_score } as f64;
            treasure_count += 1;
        }
        score + (100000 * (self.expected_num_treasures - treasure_count)) as f64
    }

    /// Carry time for one treasure in pikmin*seconds, as used by pod, at, and colossal.
    fn story_carry(&self, c: &mut Ctx, name: &str, sp: usize, dig: bool, carry_value: f32) -> f32 {
        let info = carry_info(name);
        let carry = info.max_carry;
        let dig = if dig { c.dig_time(name) } else { 0.0 };
        let d = c.dist_to_start(sp);
        carry as f32 * LOAD_TIME_SECONDS_STORY
            + dig
            + carry as f32 * d * CARRY_MULTIPLIER / Ctx::carry_speed(carry_value, carry, info.min_carry, info.max_carry)
    }

    fn judge_pod(&self, c: &mut Ctx) -> f64 {
        let n = names();
        let counted = |name: &str| !n.ignore_treasures_pod.contains(name) && !n.optional_treasures_pod.contains(name);

        let mut num_pokos_collected = 0;
        let mut num_treasures_collected = 0;
        let mut targets = Vec::new();
        for i in c.p.items.iter() {
            let name = i.name.to_ascii_lowercase();
            if counted(&name) {
                num_treasures_collected += 1;
                num_pokos_collected += pokos(&name);
                targets.push(i.sp);
            }
        }
        for t in c.p.tekis.iter() {
            if let Some(name) = &t.item_inside {
                if counted(name) {
                    num_treasures_collected += 1;
                    num_pokos_collected += pokos(name);
                    targets.push(t.sp);
                }
            }
        }

        // pikmin*seconds required to collect all treasures
        let mut pikmin_seconds = 0.0f32;
        let mut active_pikmin = 0.0f32;
        let items: Vec<(String, usize, bool)> =
            c.p.items
                .iter()
                .map(|i| (i.name.to_ascii_lowercase(), i.sp, true))
                .chain(
                    c.p.tekis
                        .iter()
                        .filter_map(|t| t.item_inside.as_ref().map(|n| (n.to_ascii_lowercase(), t.sp, false))),
                )
                .collect();
        for (name, sp, dig) in items.iter() {
            if !counted(name) {
                continue;
            }
            c.mark(*sp);
            active_pikmin += carry_info(name).max_carry as f32;
            pikmin_seconds += self.story_carry(c, name, *sp, *dig, self.piki_carry_value);
        }
        let carry_seconds = if pikmin_seconds == 0.0 {
            0.0
        } else {
            pikmin_seconds / (self.piki_count as f32).min(active_pikmin)
        };

        let mut walking_seconds = c.walking_mst(&targets);

        // walking to the hole
        if let Some(h) = c.p.hole.or(c.p.geyser) {
            walking_seconds += c.dist_to_start(h) / self.olimar_speed_cm / (1 + num_treasures_collected) as f32;
        } else {
            walking_seconds += 300.0;
        }

        // Teki/hazards in the way of treasures, and bug pokos
        let mut hazard_seconds = 0.0f32;
        let mut bug_pokos = 0;
        c.compute_carryable("");
        for t in 0..c.p.tekis.len() {
            if c.teki_in_the_way(t) {
                let name = c.p.tekis[t].name.to_ascii_lowercase();
                hazard_seconds += difficulty(&name).sec as f32 / 2.0;
                let sp = c.p.tekis[t].sp;
                if !n.no_carcass.contains(name.as_str()) && c.dist_to_start(sp) / self.olimar_speed_cm < walking_seconds * 0.4 {
                    bug_pokos += pokos(&name);
                }
            }
        }

        // optional treasures (these don't check for enemies in the way)
        let mut optional_seconds = 0.0f32;
        // CaveGen divides integers here: pikiCount / (numTreasuresCollected + 1)
        let dig_divisor = (self.piki_count / (num_treasures_collected + 1)) as f32;
        for k in 0..c.p.items.len() {
            let (name, sp) = (c.p.items[k].name.to_ascii_lowercase(), c.p.items[k].sp);
            if !n.optional_treasures_pod.contains(name.as_str()) {
                continue;
            }
            let info = carry_info(&name);
            let carry = info.max_carry;
            let dig = c.dig_time(&name);
            let min_dist_visited = Self::min_dist_to_visited(c, sp, true);
            let mut gate_check = 0.0f32;
            if !c.p.gates.is_empty() && c.p.sps[sp].is_alcove {
                // only checking if in an alcove
                if let Some(door) = c.p.first_door_pos[c.p.sps[sp].unit.unwrap()] {
                    for g in c.p.gates.iter() {
                        let gp = c.sp_pos(g.sp);
                        if gp.x == door.x && gp.z == door.z {
                            gate_check += c.gate_cost(g.life, self.piki_count as f32);
                        }
                    }
                }
            }
            // FC6 water override
            let carry_value = if self.cave_short_name == "FC" && self.floor == 6 {
                2.4
            } else {
                self.piki_carry_value
            };
            let d = c.dist_to_start(sp);
            let time_needed = 10.0
                + 2.0 * min_dist_visited / self.olimar_speed_cm
                + LOAD_TIME_SECONDS_STORY
                + dig / dig_divisor
                + gate_check
                + d * CARRY_MULTIPLIER / Ctx::carry_speed(carry_value, carry, info.min_carry, info.max_carry);
            optional_seconds -= Self::optional_time_save(pokos(&name), time_needed);
        }
        for k in 0..c.p.tekis.len() {
            let Some(name) = c.p.tekis[k].item_inside.as_ref().map(|n| n.to_ascii_lowercase()) else {
                continue;
            };
            if !n.optional_treasures_pod.contains(name.as_str()) {
                continue;
            }
            let (teki_name, sp) = (c.p.tekis[k].name.clone(), c.p.tekis[k].sp);
            let info = carry_info(&name);
            let carry = info.max_carry;
            let dig = 0.0f32;
            let min_dist_visited = Self::min_dist_to_visited(c, sp, false);
            let d = c.dist_to_start(sp);
            let time_needed = 10.0
                + difficulty(&teki_name).sec as f32
                + 2.0 * min_dist_visited / self.olimar_speed_cm
                + LOAD_TIME_SECONDS_STORY
                + dig / dig_divisor
                + d * CARRY_MULTIPLIER / Ctx::carry_speed(self.piki_carry_value, carry, info.min_carry, info.max_carry);
            optional_seconds -= Self::optional_time_save(pokos(&name), time_needed);
        }

        // gates in the way of holes/treasures
        let mut gate_seconds = 0.0f32;
        if !c.p.gates.is_empty() {
            c.compute_carryable("h");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    gate_seconds += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
        }

        let poko_seconds = (self.expected_num_pokos_pod - num_pokos_collected - bug_pokos) as f32 / 2.0;

        // adjustments
        let mut adj_seconds = 0.0f32;
        if self.cave_short_name == "GK" {
            if self.floor == 2 {
                for i in c.p.items.iter() {
                    if i.name.eq_ignore_ascii_case("g_futa_kyusyu") {
                        let idx = c.p.sps[i.sp].list_idx;
                        if idx == 6 || idx == 7 {
                            walking_seconds = 4.0;
                        }
                    }
                }
                hazard_seconds = 0.0;
            }
            // high treasures
            for i in c.p.items.iter() {
                if n.ignore_treasures_pod.contains(i.name.to_ascii_lowercase().as_str()) {
                    continue;
                }
                let y = c.sp_pos(i.sp).y;
                if y == 100.0 || y == 125.0 {
                    adj_seconds += 12.0;
                }
            }
            // breadbugs
            for ii in 0..c.p.items.len() {
                let name = c.p.items[ii].name.to_ascii_lowercase();
                let isp = c.p.items[ii].sp;
                if n.ignore_treasures_pod.contains(name.as_str()) {
                    continue;
                }
                if c.sp_pos(isp).y >= 30.0 {
                    continue;
                }
                if carry_info(&name).min_carry > 10 {
                    continue;
                }
                let d_item = c.dist_to_start(isp);
                if d_item < 170.0 {
                    continue;
                }
                let mut worst = 0.0f32;
                for t in c.p.tekis.iter() {
                    if t.name.eq_ignore_ascii_case("panmodoki") {
                        let closeness = c.sp_dist(t.sp, isp) / d_item;
                        if closeness < 0.25 {
                            worst = worst.max((1.5 / closeness).min(15.0));
                        }
                    }
                }
                adj_seconds += worst;
            }
        }

        (walking_seconds
            + carry_seconds
            + hazard_seconds
            + gate_seconds
            + optional_seconds
            + poko_seconds
            + adj_seconds
            + 10.0 * num_treasures_collected as f32) as f64
    }

    /// Distance from a spawn point to the nearest waypoint already on a carry path.
    fn min_dist_to_visited(c: &Ctx, sp: usize, include_start: bool) -> f32 {
        let sp_pos = c.sp_pos(sp);
        let mut min_dist = 128000.0f32;
        for w in c.g.wps.iter() {
            if !w.carryable_behind && !(include_start && w.is_start) {
                continue;
            }
            min_dist = min_dist.min(dist(sp_pos, w.pos));
        }
        min_dist
    }

    /// Seconds saved by collecting an optional treasure (1.5 extra pokos = 1s), or 0.
    fn optional_time_save(pokos: i32, time_needed: f32) -> f32 {
        let time_save = ((pokos / 4) as f32).min(pokos as f32 / 1.5 - time_needed);
        if time_save > 0.0 { time_save } else { 0.0 }
    }

    fn judge_at(&self, c: &mut Ctx) -> f64 {
        let n = names();
        let treasure_count = (c.p.items.len() + c.p.tekis.iter().filter(|t| t.item_inside.is_some()).count()) as i32;
        let num_purple_candypop = c.p.tekis.iter().filter(|t| t.name.eq_ignore_ascii_case("blackpom")).count() as i32;

        let targets: Vec<usize> =
            c.p.items
                .iter()
                .map(|i| i.sp)
                .chain(c.p.tekis.iter().filter(|t| t.item_inside.is_some()).map(|t| t.sp))
                .collect();
        let mut walking_seconds = c.walking_mst(&targets);

        if let Some(h) = c.p.hole.or(c.p.geyser) {
            walking_seconds += c.dist_to_start(h) / self.olimar_speed_cm / (1 + self.expected_num_treasures) as f32;
        } else {
            walking_seconds += 300.0;
        }

        // purple candypops
        if !n.purple20.contains(self.sublevel_id.as_str()) && self.expected_num_purple_candypop > 0 {
            for t in 0..c.p.tekis.len() {
                if c.p.tekis[t].name.eq_ignore_ascii_case("blackpom") {
                    let sp = c.p.tekis[t].sp;
                    walking_seconds += c.dist_to_start(sp) / self.olimar_speed_cm / (2 + self.expected_num_treasures) as f32;
                }
            }
        }

        let carry_seconds = self.collect_all_treasures(c, 1.0, 0.0);

        let mut hazard_seconds = 0.0f32;
        c.compute_carryable("t");
        for t in 0..c.p.tekis.len() {
            if c.teki_in_the_way(t) {
                hazard_seconds += difficulty(&c.p.tekis[t].name).sec as f32 / 2.0;
            }
        }

        let mut gate_seconds = 0.0f32;
        if !c.p.gates.is_empty() {
            c.compute_carryable("htp");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    gate_seconds += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
        }

        let purple_penalty = if n.purple20.contains(self.sublevel_id.as_str()) {
            0.0
        } else {
            (60 * (self.expected_num_purple_candypop - num_purple_candypop)) as f32
        };
        let treasure_penalty = if treasure_count < self.expected_num_treasures { 300.0 } else { 0.0 };

        (walking_seconds + carry_seconds + hazard_seconds + gate_seconds + treasure_penalty + purple_penalty + 10.0 * treasure_count as f32)
            as f64
    }

    /// Carry seconds for every treasure (used by at and colossal). `slowdown`
    /// divides each treasure's carry time (colossal uses 1.3), and
    /// `initial_pikmin_seconds` starts the sum (colossal adds its onions).
    fn collect_all_treasures(&self, c: &mut Ctx, slowdown: f32, initial_pikmin_seconds: f32) -> f32 {
        let mut pikmin_seconds = initial_pikmin_seconds;
        let mut active_pikmin = 0.0f32;
        let items: Vec<(String, usize, bool)> =
            c.p.items
                .iter()
                .map(|i| (i.name.to_ascii_lowercase(), i.sp, true))
                .chain(
                    c.p.tekis
                        .iter()
                        .filter_map(|t| t.item_inside.as_ref().map(|n| (n.to_ascii_lowercase(), t.sp, false))),
                )
                .collect();
        for (name, sp, dig) in items {
            let info = carry_info(&name);
            let carry = info.max_carry;
            active_pikmin += carry as f32;
            let dig = if dig { c.dig_time(&name) } else { 0.0 };
            let d = c.dist_to_start(sp);
            pikmin_seconds += carry as f32 * LOAD_TIME_SECONDS_STORY
                + dig
                + carry as f32 * d * CARRY_MULTIPLIER
                    / Ctx::carry_speed(self.piki_carry_value, carry, info.min_carry, info.max_carry)
                    / slowdown;
        }
        if pikmin_seconds == 0.0 {
            0.0
        } else {
            pikmin_seconds / (self.piki_count as f32).min(active_pikmin)
        }
    }

    fn judge_attk(&self, c: &mut Ctx) -> f64 {
        let n = names();
        let mut pokos_available = 0;
        for t in c.p.tekis.iter() {
            let name = t.name.to_ascii_lowercase();
            if name == "egg" {
                pokos_available += 10; // mitites
            } else if !n.no_carcass.contains(name.as_str()) {
                pokos_available += pokos(&name);
            }
            if let Some(item) = &t.item_inside {
                pokos_available += pokos(item);
            }
        }
        for i in c.p.items.iter() {
            pokos_available += pokos(&i.name);
        }

        let mut pikmin_seconds = 0.0f32;
        let mut num_hazards = 0;
        for t in 0..c.p.tekis.len() {
            let teki = c.p.tekis[t].clone();
            let name = teki.name.to_ascii_lowercase();
            if n.plants.contains(name.as_str()) {
                continue;
            }
            if n.hazards.contains(name.as_str()) {
                num_hazards += 1;
                continue;
            }
            let d = difficulty(&name);
            pikmin_seconds += (d.piki * d.sec) as f32;
            pikmin_seconds += c.work_function(&teki.name, Some(teki.sp));
            if let Some(item) = &teki.item_inside {
                pikmin_seconds += c.work_function(item, Some(teki.sp));
            }
        }
        pikmin_seconds += self.items_and_exits_work(c);

        if num_hazards > 0 {
            c.compute_carryable("tc");
            pikmin_seconds += self.hazards_in_the_way(c);
        }
        if !c.p.gates.is_empty() {
            c.compute_carryable("htc");
            pikmin_seconds += self.gates_in_the_way_work(c);
        }
        if c.p.geyser.is_some() && self.hole_clogged {
            pikmin_seconds += GEYSER_HEALTH / self.piki_attack_value;
        }

        let pikmin_efficiency_rate = 1.0f32;
        (-pokos_available * 10 - self.piki_count * 10 + self.challenge_time) as f64
            + 0.5 * pikmin_seconds as f64 / (self.piki_count as f32 * pikmin_efficiency_rate) as f64
    }

    fn items_and_exits_work(&self, c: &mut Ctx) -> f32 {
        let mut w = 0.0f32;
        for k in 0..c.p.items.len() {
            let (name, sp) = (c.p.items[k].name.clone(), c.p.items[k].sp);
            w += c.work_function(&name, Some(sp));
        }
        let (hole, geyser) = (c.p.hole, c.p.geyser);
        w += c.work_function("hole", hole);
        w += c.work_function("geyser", geyser);
        w
    }

    fn hazards_in_the_way(&self, c: &mut Ctx) -> f32 {
        let n = names();
        let mut w = 0.0f32;
        for t in 0..c.p.tekis.len() {
            let name = c.p.tekis[t].name.to_ascii_lowercase();
            if n.hazards.contains(name.as_str()) && c.teki_in_the_way(t) {
                let d = difficulty(&name);
                w += (d.piki * d.sec) as f32;
            }
        }
        w
    }

    fn gates_in_the_way_work(&self, c: &mut Ctx) -> f32 {
        let mut w = 0.0f32;
        for g in 0..c.p.gates.len() {
            if c.gate_in_the_way(g) {
                let wall_count = self.piki_count as f32 / 2.0;
                w += wall_count * c.gate_cost(c.p.gates[g].life, wall_count);
            }
        }
        w
    }

    fn has_key(c: &Ctx) -> bool {
        c.p.items.iter().any(|i| i.name == "key") || c.p.tekis.iter().any(|t| t.item_inside.as_deref() == Some("key"))
    }

    fn judge_cmat(&self, c: &mut Ctx) -> f64 {
        let n = names();
        let mut pikmin_seconds = 0.0f32;
        let mut num_hazards = 0;
        for t in 0..c.p.tekis.len() {
            let teki = c.p.tekis[t].clone();
            let name = teki.name.to_ascii_lowercase();
            if n.hazards.contains(name.as_str()) {
                num_hazards += 1;
                continue;
            }
            let Some(item) = &teki.item_inside else { continue };
            let d = difficulty(&name);
            pikmin_seconds += (d.piki * d.sec) as f32;
            pikmin_seconds += c.work_function(item, Some(teki.sp));
        }
        pikmin_seconds += self.items_and_exits_work(c);

        if num_hazards > 0 {
            c.compute_carryable("t");
            pikmin_seconds += self.hazards_in_the_way(c);
        }
        if !c.p.gates.is_empty() {
            c.compute_carryable("ht");
            pikmin_seconds += self.gates_in_the_way_work(c);
        }
        if c.p.geyser.is_some() && self.hole_clogged {
            pikmin_seconds += GEYSER_HEALTH / self.piki_attack_value;
        }

        let pikmin_efficiency_rate = 1.0f32;
        let score = pikmin_seconds / (self.piki_count as f32 * pikmin_efficiency_rate);
        if !Self::has_key(c) {
            return 500.0;
        }
        score as f64
    }

    fn judge_key(&self, c: &mut Ctx) -> f64 {
        let mut trip_key_gates = 0.0f32;
        let mut trip_hole_gates = 0.0f32;
        if !c.p.gates.is_empty() {
            c.compute_carryable("k");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    trip_key_gates += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
            c.compute_carryable("h");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    trip_hole_gates += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
            trip_hole_gates -= trip_key_gates;
        }

        let key_walk;
        let mut key_carry = 0.0f32;
        let mut hole_walk = 0.0f32;
        let key_sp = c.p.items.iter().find(|i| i.name == "key").map(|i| (i.sp, 0.0f32)).or_else(|| {
            c.p.tekis
                .iter()
                .find(|t| t.item_inside.as_deref() == Some("key"))
                .map(|t| (t.sp, difficulty(&t.name).sec as f32))
        });
        match key_sp {
            Some((sp, fight)) => {
                let carry = self.piki_count.min(3);
                let d = c.dist_to_start(sp);
                key_walk = d / self.olimar_speed_cm + fight;
                key_carry = d * CARRY_MULTIPLIER / (220.0f32 + 180.0f32 * (self.fastest_piki_carry_value * carry as f32 - 1.0 + 1.0) / 3.0);
                if let Some(h) = c.p.hole {
                    hole_walk = c.sp_dist(h, sp) / self.olimar_speed_cm;
                }
                if let Some(g) = c.p.geyser {
                    hole_walk = c.sp_dist(g, sp) / self.olimar_speed_cm;
                }
                hole_walk = hole_walk.min(c.sp_dist(c.p.start, sp) / self.olimar_speed_cm);
            }
            None => key_walk = 300.0,
        }

        let geyser_break = if c.p.geyser.is_some() {
            GEYSER_HEALTH / self.piki_attack_value / self.piki_count as f32
        } else {
            0.0
        };

        ((key_walk + trip_key_gates) + key_carry.max(hole_walk + trip_hole_gates) + geyser_break) as f64
    }

    fn judge_colossal(&self, c: &mut Ctx) -> f64 {
        let mut trip_key_gates = 0.0f32;
        if !c.p.gates.is_empty() {
            c.compute_carryable("m");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    trip_key_gates += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
        }
        let mut key_walk = 100000.0f32;
        let mut key_carry = 0.0f32;
        if let Some(t) = c.p.tekis.iter().find(|t| t.item_inside.as_deref() == Some("map01")).cloned() {
            let carry = self.piki_count.min(3);
            let d = c.dist_to_start(t.sp);
            key_walk = d / self.olimar_speed_cm + difficulty(&t.name).sec as f32;
            key_carry = d * CARRY_MULTIPLIER / (220.0f32 + 180.0f32 * (self.fastest_piki_carry_value * carry as f32 - 40.0 + 1.0) / 100.0);
        }

        let treasure_count = (c.p.items.len() + c.p.tekis.iter().filter(|t| t.item_inside.is_some()).count()) as i32;

        let mut onion_seconds = 0.0f32;
        for o in c.p.onions.clone() {
            onion_seconds += 10.0 * self.piki_count as f32 * c.dist_to_start(o) / 170.0;
        }
        // The onion term goes into the same pikmin*seconds sum as the treasures.
        let carry_seconds = self.collect_all_treasures(c, 1.3, onion_seconds);

        let mut hazard_seconds = 0.0f32;
        c.compute_carryable("t");
        for t in 0..c.p.tekis.len() {
            if c.teki_in_the_way(t) {
                hazard_seconds += difficulty(&c.p.tekis[t].name).sec as f32 / 2.0;
            }
        }

        let mut gate_seconds = 0.0f32;
        if !c.p.gates.is_empty() {
            c.compute_carryable("htp");
            for g in 0..c.p.gates.len() {
                if c.gate_in_the_way(g) {
                    gate_seconds += c.gate_cost(c.p.gates[g].life, self.piki_count as f32);
                }
            }
        }

        let treasure_penalty = if treasure_count < self.expected_num_treasures {
            (10000 * (self.expected_num_treasures - treasure_count)) as f32
        } else {
            0.0
        };

        (trip_key_gates + key_walk + key_carry + carry_seconds + hazard_seconds + gate_seconds + treasure_penalty) as f64
    }
}

// ----------------------------------------------------------------------------
// Ranks and filters
// ----------------------------------------------------------------------------

/// Score breakpoints for converting a score to a percentile rank, like the
/// lines of CaveGen's rank_file.txt.
#[derive(Debug, Clone)]
pub struct RankTable {
    breakpoints: Vec<f64>,
}

impl RankTable {
    /// Builds the 1000 breakpoints from a sample of scores, the same way
    /// CaveGen's `-judge rankfile` does.
    pub fn from_scores(mut scores: Vec<f64>) -> Option<RankTable> {
        if scores.is_empty() {
            return None;
        }
        scores.sort_by(|a, b| a.total_cmp(b));
        let n = scores.len();
        let breakpoints = (1..=1000)
            .map(|i| {
                let s = scores[((i as f64 / 1000.0 - 0.0005) * n as f64) as usize];
                (s * 100.0).round() / 100.0
            })
            .collect();
        Some(RankTable { breakpoints })
    }

    /// Parses one line of CaveGen's rank_file.txt (after the "<sublevel>-<type>;" key).
    pub fn from_rank_file_line(values: &str) -> Option<RankTable> {
        let breakpoints: Vec<f64> = values.split(';').filter_map(|v| v.trim().parse().ok()).collect();
        (breakpoints.len() == 1000).then_some(RankTable { breakpoints })
    }

    /// Percentile rank of this score (0 = best, 100 = worst). CaveGen's scoreToRank.
    pub fn rank(&self, score: f64) -> f64 {
        let b = &self.breakpoints;
        let score = (100.0 * score).round() / 100.0;
        if score < b[0] {
            return 0.0;
        }
        if score > b[999] {
            return 100.0;
        }
        let (mut low, mut hi) = (0usize, 999usize);
        while low + 1 < hi {
            let mid = (hi + low) / 2;
            if score <= b[mid] {
                hi = mid;
            } else {
                low = mid;
            }
        }
        if b[hi] == score {
            low = hi;
        }
        let mut num_dupes = 0;
        for &x in b.iter().skip(low + 1) {
            if x == score {
                num_dupes += 1;
            } else {
                break;
            }
        }
        low += num_dupes / 2;
        low as f64 / 10.0
    }
}

/// A filter on judge results, written like CaveGen's: "<1%" keeps the best 1%
/// by rank, ">99%" the worst 1%, and "<120" / ">120" filter on the score itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JudgeFilter {
    RankBelow(f64),
    RankAbove(f64),
    ScoreBelow(f64),
    ScoreAbove(f64),
}

impl FromStr for JudgeFilter {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let err = || format!("Bad judge filter '{s}'. Examples: \"<1%\", \">99%\", \"<120\"");
        let (op, rest) = s.split_at(s.find(|c: char| c != '<' && c != '>').ok_or_else(err)?);
        let (value, percent) = match rest.strip_suffix('%') {
            Some(v) => (v, true),
            None => (rest, false),
        };
        let value: f64 = value.trim().parse().map_err(|_| err())?;
        match (op, percent) {
            ("<", true) => Ok(JudgeFilter::RankBelow(value)),
            (">", true) => Ok(JudgeFilter::RankAbove(value)),
            ("<", false) => Ok(JudgeFilter::ScoreBelow(value)),
            (">", false) => Ok(JudgeFilter::ScoreAbove(value)),
            _ => Err(err()),
        }
    }
}

impl JudgeFilter {
    pub fn needs_rank(&self) -> bool {
        matches!(self, JudgeFilter::RankBelow(_) | JudgeFilter::RankAbove(_))
    }

    /// Whether a result passes. `rank` is ignored for score filters.
    pub fn passes(&self, score: f64, rank: f64) -> bool {
        match *self {
            JudgeFilter::RankBelow(r) => rank <= r,
            JudgeFilter::RankAbove(r) => rank >= r,
            JudgeFilter::ScoreBelow(v) => score <= v,
            JudgeFilter::ScoreAbove(v) => score >= v,
        }
    }
}
