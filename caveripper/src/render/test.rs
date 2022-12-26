use rand::{Rng, SeedableRng, rngs::SmallRng};
use paste::paste;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use crate::{
    assets::AssetManager,
    layout::Layout,
    render::*
};

macro_rules! test_render {
    ($($name: literal),+) => {
        paste!{
            $(
                #[test]
                fn [<test_render_layouts_ $name>] () {
                    AssetManager::init_global("../assets", "..").unwrap();

                    let num_layouts = 4;
                    let caveinfos = AssetManager::caveinfos_from_cave($name.replace('_', ":").as_str()).unwrap();

                    caveinfos.into_par_iter().panic_fuse().for_each(|caveinfo| {
                        let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
                        for _ in 0..num_layouts {
                            let seed: u32 = rng.gen();
                            let layout = Layout::generate(seed, &caveinfo);
                            if let Err(cause) = render_layout(&layout, LayoutRenderOptions::default()) {
                                panic!("({}, {:#010X}) {}", $name, seed, cause);
                            }
                        }
                    });
                }

                #[test]
                fn [<test_render_caveinfo_ $name>] () {
                    AssetManager::init_global("../assets", "..").unwrap();
                    let caveinfos = AssetManager::caveinfos_from_cave($name.replace('_', ":").as_str()).unwrap();
                    caveinfos.into_par_iter().panic_fuse().for_each(|caveinfo| {
                        render_caveinfo(&caveinfo, CaveinfoRenderOptions::default()).unwrap();
                    });
                }
            )*
        }
    }
}

test_render!("ec", "scx", "fc", "hob", "wfg", "bk", "sh", "cos", "gk", "sr", "smc", "coc", "hoh",
"dd", "exc", "nt", "ltb", "cg", "gh", "hh", "ba", "rc", "tg", "twg", "cc", "cm",
"cr", "dn", "ca", "sp", "tct", "ht", "csn", "gb", "rg", "sl", "hg", "ad", "str",
"bg", "cop", "bd", "snr", "er", "newyear_bg", "newyear_sk", "newyear_cwnn", "newyear_snd",
"newyear_ch", "newyear_rh", "newyear_ss", "newyear_sa", "newyear_aa", "newyear_ser",
"newyear_tc", "newyear_er", "newyear_cg", "newyear_sd", "newyear_ch1", "newyear_ch2",
"newyear_ch3", "newyear_ch4", "newyear_ch5", "newyear_ch6", "newyear_ch7", "newyear_ch8",
"newyear_ch9", "newyear_ch10", "newyear_ch11", "newyear_ch12", "newyear_ch13", "newyear_ch14",
"newyear_ch15", "newyear_ch16", "newyear_ch17", "newyear_ch18", "newyear_ch19", "newyear_ch20",
"newyear_ch21", "newyear_ch22", "newyear_ch23", "newyear_ch24", "newyear_ch25", "newyear_ch26",
"newyear_ch27", "newyear_ch28", "newyear_ch29", "newyear_ch30", "251_at", "251_aqd", "251_ft",
"251_gd", "251_gdd", "251_im", "251_sc", "251_wf", "251_aa", "251_ss", "251_ck", "251_potw");
