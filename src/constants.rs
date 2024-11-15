use enum_map::EnumMap;

use crate::{
    command::AttributeType,
    gamestate::{character::Class, dungeons::LightDungeon},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monster {
    pub level: u16,
    pub class: Class,
    pub attributes: EnumMap<AttributeType, u32>,
    pub hp: u32,
    pub xp: u32,
}

impl Monster {
    pub const fn new(
        level: u16,
        class: Class,
        attribs: [u32; 5],
        hp: u32,
        xp: u32,
    ) -> Self {
        Monster {
            level,
            class,
            attributes: EnumMap::from_array(attribs),
            hp,
            xp,
        }
    }
}

// Values sourced from: https://www4m7de
#[rustfmt::skip]
pub const LIGHT_ENEMIES: EnumMap<LightDungeon, &'static [Monster]> = EnumMap::from_array([
    // DesecratedCatacombs
    &[
        Monster::new(10, Class::Mage, [48, 52, 104, 77, 47], 1694, 1287),
        Monster::new(12, Class::Warrior, [120, 68, 59, 101, 51], 6565, 1785),
        Monster::new(14, Class::Warrior, [149, 78, 69, 124, 65], 9300, 2395),
        Monster::new(16, Class::Scout, [84, 195, 83, 131, 94], 8908, 3146),
        Monster::new(18, Class::Warrior, [214, 135, 122, 260, 142], 16055, 4050),
        Monster::new(22, Class::Mage, [97, 99, 303, 198, 137], 9108, 6412),
        Monster::new(26, Class::Warrior, [359, 135, 122, 260, 142], 35100, 9631),
        Monster::new(30, Class::Mage, [126, 130, 460, 279, 193], 17298, 13952),
        Monster::new(40, Class::Warrior, [614, 207, 191, 445, 238], 91225, 30909),
        Monster::new(50, Class::Scout, [221, 847, 213, 561, 292], 114444, 60343),
    ] as &[Monster],
    // MinesOfGloria
    &[
        Monster::new(20, Class::Scout, [101, 264, 101, 174, 119], 14616, 2124),
        Monster::new(24, Class::Warrior, [317, 126, 117, 238, 130], 29750, 7909),
        Monster::new(28, Class::Warrior, [393, 138, 125, 284, 152], 41180, 11652),
        Monster::new(34, Class::Scout, [143, 554, 144, 303, 216], 42420, 19539),
        Monster::new(38, Class::Warrior, [592, 178, 162, 398, 195], 77610, 26652),
        Monster::new(44, Class::Mage, [191, 190, 780, 411, 259], 36990, 40886),
        Monster::new(48, Class::Warrior, [744, 243, 230, 563, 246], 137935, 53228),
        Monster::new(56, Class::Scout, [250, 960, 240, 680, 345], 155040, 86309),
        Monster::new(66, Class::Scout, [300, 1160, 290, 880, 420], 235840, 148282),
        Monster::new(70, Class::Warrior, [1240, 385, 360, 960, 340], 340800, 181085),
    ] as &[Monster],
    // RuinsOfGnark
    &[
        Monster::new(32, Class::Scout, [155, 486, 161, 276, 205], 36432, 16557),
        Monster::new(36, Class::Scout, [141, 602, 149, 344, 230], 50912, 22893),
        Monster::new(42, Class::Scout, [205, 726, 224, 403, 247], 69316, 35642),
        Monster::new(46, Class::Warrior, [768, 215, 183, 539, 249], 126665, 46757),
        Monster::new(54, Class::Warrior, [920, 265, 240, 640, 260], 176000, 76872),
        Monster::new(60, Class::Scout, [270, 1040, 260, 760, 375], 185400, 108013),
        Monster::new(64, Class::Warrior, [1120, 340, 315, 840, 310], 273000, 133734),
        Monster::new(76, Class::Scout, [350, 1360, 340, 1080, 495], 332640, 240784),
        Monster::new(86, Class::Warrior, [1560, 505, 480, 1280, 420], 556800, 374041),
        Monster::new(90, Class::Warrior, [1640, 535, 510, 1360, 440], 618800, 441608),
    ] as &[Monster],
    // CutthroatGrotto
    &[
        Monster::new(52, Class::Scout, [230, 880, 220, 601, 315], 127412, 68234),
        Monster::new(58, Class::Scout, [260, 1000, 250, 720, 360], 169920, 96706),
        Monster::new(62, Class::Warrior, [1080, 325, 300, 800, 300], 252000, 120287),
        Monster::new(68, Class::Warrior, [1200, 370, 345, 920, 330], 317400, 163994),
        Monster::new(74, Class::Warrior, [1320, 415, 390, 1040, 360], 390000, 163994),
        Monster::new(82, Class::Scout, [380, 1480, 370, 1200, 540], 398400, 315135),
        Monster::new(84, Class::Warrior, [1520, 490, 465, 1240, 410], 527000, 343618),
        Monster::new(96, Class::Warrior, [1760, 580, 555, 1480, 470], 717800, 560797),
        Monster::new(102, Class::Scout, [480, 1880, 470, 1600, 690], 659200, 704509),
        Monster::new(110, Class::Scout, [520, 2040, 510, 1760, 750], 781440, 940791),
    ] as &[Monster],
    // EmeraldScaleAltar
    &[
        Monster::new(72, Class::Scout, [330, 1280, 320, 1000, 465], 292000, 199497),
        Monster::new(78, Class::Scout, [360, 1400, 350, 1120, 510], 353920, 263817),
        Monster::new(80, Class::Scout, [370, 1440, 360, 1160, 525], 375840, 288496),
        Monster::new(88, Class::Scout, [410, 1600, 400, 1320, 585], 469920, 406744),
        Monster::new(94, Class::Warrior, [1720, 565, 540, 1440, 460], 684000, 518518),
        Monster::new(100, Class::Scout, [470, 1840, 460, 1560, 675], 630240, 653687),
        Monster::new(108, Class::Warrior, [2000, 670, 645, 1720, 530], 937400, 876584),
        Monster::new(114, Class::Mage, [520, 540, 2200, 1760, 775], 404800, 1081088),
        Monster::new(122, Class::Warrior, [2280, 775, 750, 2000, 600], 1230000, 1412064),
        Monster::new(130, Class::Scout, [620, 2440, 610, 2160, 900], 1131840, 1821461),
    ] as &[Monster],
    // ToxicTree
    &[
        Monster::new(92, Class::Warrior, [1680, 550, 525, 1400, 450], 651000, 478738),
        Monster::new(98, Class::Scout, [460, 1800, 450, 1520, 660], 601920, 605700),
        Monster::new(104, Class::Mage, [470, 490, 2000, 1560, 700], 327600, 758451),
        Monster::new(106, Class::Scout, [500, 1960, 490, 1680, 720], 719040, 815853),
        Monster::new(118, Class::Scout, [560, 2200, 520, 1920, 810], 913920, 1237696),
        Monster::new(124, Class::Warrior, [2320, 790, 765, 2040, 610], 1275000, 1506706),
        Monster::new(128, Class::Scout, [610, 2400, 600, 2120, 885], 1093920, 1710914),
        Monster::new(136, Class::Mage, [630, 650, 2640, 2200, 940], 602800, 2187846),
        Monster::new(144, Class::Scout, [690, 2720, 680, 2440, 1005], 1415200, 2767832),
        Monster::new(150, Class::Scout, [720, 2840, 710, 2560, 1050], 1546240, 3280697),
    ] as &[Monster],
    // MagmaStream
    &[
        Monster::new(112, Class::Scout, [530, 2080, 520, 1800, 765], 813600, 1009041),
        Monster::new(116, Class::Scout, [550, 2160, 540, 1880, 795], 879840, 1157092),
        Monster::new(120, Class::Mage, [550, 570, 2320, 1880, 820], 454960, 1322625),
        Monster::new(126, Class::Warrior, [2360, 805, 780, 2080, 620], 1320800, 1606255),
        Monster::new(134, Class::Warrior, [2520, 865, 840, 2240, 660], 1512000, 2059369),
        Monster::new(138, Class::Warrior, [2600, 895, 870, 2320, 680], 1612400, 2322552),
        Monster::new(142, Class::Mage, [660, 680, 2760, 2320, 985], 663520, 2612278),
        Monster::new(146, Class::Warrior, [2760, 955, 930, 2480, 720], 1822800, 2930646),
        Monster::new(148, Class::Warrior, [2800, 970, 945, 2520, 730], 1877400, 3101774),
        Monster::new(170, Class::Warrior, [3240, 1135, 1110, 2960, 840], 2530800, 5583708),
    ] as &[Monster],
    // FrostBloodTemple
    &[
        Monster::new(132, Class::Warrior, [2480, 850, 825, 2200, 650], 1463000, 1937541),
        Monster::new(140, Class::Scout, [670, 2640, 660, 2360, 975], 1331040, 2463717),
        Monster::new(154, Class::Warrior, [2920, 1015, 990, 2640, 760], 2046000, 3663979),
        Monster::new(158, Class::Scout, [760, 3000, 750, 2720, 1110], 1729920, 4082943),
        Monster::new(164, Class::Scout, [790, 3120, 780, 2840, 1155], 1874400, 4785109),
        Monster::new(168, Class::Mage, [790, 810, 3280, 2840, 1180], 959920, 5306545),
        Monster::new(172, Class::Warrior, [3280, 1150, 1125, 3000, 850], 2595000, 5873522),
        Monster::new(180, Class::Scout, [870, 3440, 860, 3160, 1275], 2287840, 7157815),
        Monster::new(185, Class::Mage, [875, 895, 3620, 3180, 1305], 1182960, 8070081),
        Monster::new(200, Class::Mage, [950, 970, 3920, 3480, 1410], 1398960, 11412835),
    ] as &[Monster],
    // PyramidsofMadness
    &[
        Monster::new(152, Class::Warrior, [2880, 1000, 975, 2600, 750], 1989000, 3467701),
        Monster::new(156, Class::Mage, [730, 750, 3040, 2600, 1090], 816400, 3868959),
        Monster::new(160, Class::Scout, [770, 3040, 760, 2760, 1125], 1777440, 4307201),
        Monster::new(162, Class::Warrior, [3080, 1075, 1050, 2800, 800], 2282000, 4541147),
        Monster::new(166, Class::Mage, [780, 800, 3240, 2800, 1165], 935200, 5040468),
        Monster::new(174, Class::Warrior, [3320, 1165, 1140, 3040, 860], 2660000, 6175189),
        Monster::new(176, Class::Scout, [850, 3360, 840, 3080, 1245], 2180640, 6489101),
        Monster::new(178, Class::Scout, [860, 3400, 850, 3120, 1260], 2233920, 6816906),
        Monster::new(190, Class::Mage, [900, 920, 3720, 3280, 1340], 1252960, 9081081),
        Monster::new(0, Class::Warrior, [0, 0, 0, 0, 0], 0, 0),
    ] as &[Monster],
    // BlackSkullFortress
    &[
        Monster::new(205, Class::Scout, [995, 3940, 985, 3660, 1450], 3015840, 12751538),
        Monster::new(210, Class::Warrior, [4040, 1420, 1395, 3760, 1010], 3966800, 14222021),
        Monster::new(215, Class::Warrior, [4140, 1455, 1430, 3860, 1030], 4168800, 15824529),
        Monster::new(220, Class::Warrior, [4240, 1490, 1465, 3960, 1050], 4375800, 17581974),
        Monster::new(225, Class::Scout, [1095, 4340, 1085, 4060, 1590], 3670240, 19491852),
        Monster::new(230, Class::Scout, [1120, 4440, 1110, 4160, 1645], 3843840, 21576743),
        Monster::new(235, Class::Mage, [1125, 1145, 4620, 4180, 1655], 1972960, 23839326),
        Monster::new(240, Class::Warrior, [4640, 1630, 1605, 4360, 1130], 5253800, 26302843),
        Monster::new(245, Class::Warrior, [4740, 1665, 1640, 4460, 1150], 5485800, 28966329),
        Monster::new(250, Class::Warrior, [4840, 1700, 1675, 4560, 1170], 5722800, 31862139),
    ] as &[Monster],
    // CircusOfHorror
    &[
        Monster::new(255, Class::Warrior, [4940, 1735, 1710, 4660, 1190], 5964800, 34985806),
        Monster::new(260, Class::Scout, [1270, 5040, 1260, 4760, 1835], 4969440, 38369989),
        Monster::new(265, Class::Warrior, [5140, 1805, 1780, 4860, 1230], 6463800, 42016588),
        Monster::new(270, Class::Warrior, [5240, 1840, 1815, 4960, 1250], 6720800, 45958126),
        Monster::new(275, Class::Mage, [1325, 1345, 5420, 4980, 1935], 2748960, 50191950),
        Monster::new(280, Class::Scout, [1370, 5440, 1360, 5160, 1975], 5799840, 54764113),
        Monster::new(285, Class::Warrior, [5540, 1945, 1920, 5260, 1310], 7521800, 59666036),
        Monster::new(290, Class::Scout, [1420, 5640, 1410, 5360, 2045], 6239040, 64942539),
        Monster::new(295, Class::Mage, [1425, 1445, 5820, 5380, 2075], 3184960, 70595045),
        Monster::new(300, Class::Warrior, [5840, 2050, 2025, 5560, 1370], 8787200, 76669139),
    ] as &[Monster],
    // Hell
    &[
        Monster::new(305, Class::Mage, [1475, 1495, 6020, 5580, 2145], 3414960, 83158305),
        Monster::new(310, Class::Mage, [1500, 1520, 6120, 5680, 2180], 3532960, 90125436),
        Monster::new(315, Class::Warrior, [6140, 2155, 2130, 5860, 2255], 9258800, 97556858),
        Monster::new(320, Class::Scout, [1570, 6240, 1560, 5960, 2255], 7652640, 105514978),
        Monster::new(325, Class::Mage, [1575, 1595, 6420, 5980, 2285], 3898960, 113997992),
        Monster::new(330, Class::Warrior, [6440, 2260, 2235, 6160, 1490], 10194800, 123067419),
        Monster::new(335, Class::Scout, [1645, 6540, 1635, 6260, 2360], 8413440, 132712488),
        Monster::new(340, Class::Warrior, [6640, 2330, 2305, 6360, 1530], 10843800, 143018630),
        Monster::new(345, Class::Warrior, [6740, 2365, 2340, 6460, 1550], 11175800, 153964246),
        Monster::new(350, Class::Warrior, [6840, 2400, 2375, 6560, 1570], 11512800, 165631756),
    ] as &[Monster],
    // The13thFloor
    &[
        Monster::new(355, Class::Warrior, [7570, 2655, 2630, 7290, 1716], 12976200, 178017293),
        Monster::new(360, Class::Mage, [1970, 1990, 8000, 7560, 2838], 5458320, 191202824),
        Monster::new(365, Class::Warrior, [8290, 2908, 2882, 8010, 1860], 14658300, 205171015),
        Monster::new(370, Class::Mage, [2160, 2180, 8760, 8320, 3104], 6173440, 220033230),
        Monster::new(375, Class::Warrior, [9340, 3275, 3250, 9060, 2070], 17032800, 235758967),
        Monster::new(380, Class::Scout, [2682, 10690, 2672, 10410, 3812], 15864840, 252458197),
        Monster::new(385, Class::Mage, [2888, 2908, 11670, 11230, 4122], 8669560, 270120546),
        Monster::new(390, Class::Warrior, [12540, 4395, 4370, 12260, 2710], 23968300, 288853442),
        Monster::new(395, Class::Warrior, [13540, 4725, 4720, 13260, 2910], 26540800, 308630400),
        Monster::new(400, Class::Warrior, [16840, 5900, 5875, 16540, 3570], 33202800, 329599075),
        Monster::new(410, Class::Scout, [7080, 20200, 7050, 18210, 4280], 29937240, 300000000),
        Monster::new(420, Class::Warrior, [24240, 8490, 8460, 20030, 5130], 42163152, 300000000),
        Monster::new(430, Class::Mage, [10150, 10180, 29080, 22030, 6150], 18989860, 300000000),
        Monster::new(440, Class::Warrior, [34890, 12210, 12180, 24230, 7380], 53427152, 300000000),
        Monster::new(450, Class::Mage, [14610, 14650, 41860, 26650, 8850], 24038300, 300000000),
        Monster::new(460, Class::Warrior, [50230, 17580, 17530, 29310, 10620], 67559552, 300000000),
        Monster::new(470, Class::Warrior, [60270, 21090, 21030, 32240, 12740], 75925200, 300000000),
        Monster::new(480, Class::Warrior, [72320, 25300, 25230, 35460, 15280], 85281296, 300000000),
        Monster::new(490, Class::Scout, [30360, 86780, 30270, 39000, 18330], 76596000, 300000000),
        Monster::new(500, Class::Warrior, [0, 0, 0, 0, 0], 0, 300000000),
    ] as &[Monster],
    // Easteros
    &[
        Monster::new(310, Class::Warrior, [6040, 2120, 2095, 5760, 1410], 8956800, 90125436),
        Monster::new(320, Class::Warrior, [6240, 2190, 2165, 5960, 1450], 9565800, 105514978),
        Monster::new(330, Class::Mage, [3200, 3240, 13040, 12160, 4640], 8049920, 123067419),
        Monster::new(340, Class::Warrior, [13280, 4660, 4610, 12720, 3060], 21687600, 143018630),
        Monster::new(350, Class::Warrior, [13680, 4800, 4750, 13120, 3140], 23025600, 165631756),
        Monster::new(360, Class::Mage, [5910, 5970, 24000, 22680, 8514], 16374960, 191202824),
        Monster::new(370, Class::Scout, [6540, 26040, 6510, 25200, 9327], 37396800, 220033230),
        Monster::new(380, Class::Warrior, [32070, 11244, 11166, 31230, 7020], 59493152, 252458197),
        Monster::new(390, Class::Warrior, [35000, 11800, 12000, 40040, 8000], 95873200, 288853442),
        Monster::new(400, Class::Mage, [8500, 8500, 32000, 30000, 12000], 52867840, 300000000),
        Monster::new(410, Class::Scout, [8720, 34720, 8680, 33600, 12436], 55238400, 300000000),
        Monster::new(420, Class::Mage, [11100, 11200, 45000, 42800, 15940], 36037600, 300000000),
        Monster::new(430, Class::Warrior, [45800, 16060, 15935, 44400, 10170], 95682000, 300000000),
        Monster::new(440, Class::Scout, [11800, 47000, 11750, 45600, 16805], 80438400, 300000000),
        Monster::new(450, Class::Warrior, [57840, 20280, 20130, 56160, 12780], 126640800, 300000000),
        Monster::new(460, Class::Warrior, [59280, 20784, 20634, 57600, 13068], 132768000, 300000000),
        Monster::new(470, Class::Mage, [15120, 15240, 61200, 58560, 21648], 55163520, 300000000),
        Monster::new(480, Class::Mage, [18060, 18200, 73080, 70000, 25844], 67340000, 300000000),
        Monster::new(490, Class::Warrior, [74200, 26012, 25837, 72240, 16254], 177349200, 300000000),
        Monster::new(500, Class::Scout, [19040, 75880, 18970, 73920, 27055], 148135680, 300000000),
    ] as &[Monster],
    // Tower
    &[
        Monster::new(200, Class::Warrior, [4194, 1697, 1665, 15940, 2589], 16019700, 267461),
        Monster::new(202, Class::Mage, [1714, 1678, 4242, 16140, 2622], 6552840, 279582),
        Monster::new(204, Class::Scout, [1730, 4292, 1695, 16328, 2654], 13388960, 292142),
        Monster::new(206, Class::Warrior, [4340, 1746, 1715, 16512, 2690], 17089920, 305197),
        Monster::new(208, Class::Warrior, [4385, 1763, 1733, 16712, 2726], 17464040, 318717),
        Monster::new(210, Class::Mage, [1782, 1747, 4434, 16896, 2757], 7130112, 332714),
        Monster::new(212, Class::Warrior, [4482, 1794, 1766, 17100, 2790], 18211500, 347250),
        Monster::new(214, Class::Mage, [1813, 1787, 4529, 17284, 2822], 7432120, 362294),
        Monster::new(216, Class::Mage, [1828, 1800, 4578, 17484, 2858], 7588056, 377859),
        Monster::new(218, Class::Warrior, [4627, 1847, 1818, 17680, 2891], 19359600, 394011),
        Monster::new(220, Class::Mage, [1861, 1835, 4674, 17860, 2927], 7894120, 410715),
        Monster::new(222, Class::Mage, [1878, 1855, 4723, 18064, 2957], 8056544, 427985),
        Monster::new(224, Class::Warrior, [4771, 1898, 1869, 18244, 2991], 20524500, 445894),
        Monster::new(226, Class::Warrior, [4820, 1909, 1887, 18440, 3027], 20929400, 464404),
        Monster::new(228, Class::Warrior, [4870, 1928, 1907, 18620, 3060], 21319900, 483529),
        Monster::new(230, Class::Mage, [1943, 1921, 4914, 18824, 3094], 8696688, 503347),
        Monster::new(232, Class::Warrior, [4964, 1962, 1940, 19020, 3126], 22158300, 523816),
        Monster::new(234, Class::Scout, [1977, 5012, 1957, 19204, 3160], 18051760, 544954),
        Monster::new(236, Class::Warrior, [5059, 1993, 1975, 19392, 3198], 22979520, 566842),
        Monster::new(238, Class::Scout, [2009, 5109, 1991, 19584, 3230], 18722304, 589435),
        Monster::new(240, Class::Warrior, [5157, 2024, 2009, 19780, 3262], 23834900, 612753),
        Monster::new(242, Class::Warrior, [5206, 2043, 2028, 19960, 3295], 24251400, 636884),
        Monster::new(244, Class::Warrior, [5252, 2058, 2042, 20160, 3330], 24696000, 661779),
        Monster::new(246, Class::Warrior, [5302, 2077, 2061, 20360, 3362], 25144600, 687457),
        Monster::new(248, Class::Mage, [2090, 2078, 5348, 20544, 3398], 10230912, 714012),
        Monster::new(250, Class::Scout, [2109, 5398, 2098, 20728, 3430], 20810912, 741394),
        Monster::new(252, Class::Scout, [2139, 5448, 2128, 20944, 3477], 21195328, 769623),
        Monster::new(254, Class::Warrior, [5498, 2173, 2162, 21160, 3522], 26979000, 798800),
        Monster::new(256, Class::Warrior, [5549, 2207, 2198, 21356, 3567], 27442460, 828869),
        Monster::new(258, Class::Warrior, [5596, 2241, 2228, 21572, 3613], 27935740, 859851),
        Monster::new(260, Class::Warrior, [5646, 2275, 2263, 21792, 3657], 28438560, 891854),
        Monster::new(262, Class::Mage, [2306, 2296, 5695, 21992, 3705], 11567792, 924818),
        Monster::new(264, Class::Warrior, [5744, 2340, 2331, 22204, 3751], 29420300, 958767),
        Monster::new(266, Class::Warrior, [5796, 2377, 2362, 22412, 3794], 29920020, 993815),
        Monster::new(268, Class::Warrior, [5846, 2405, 2396, 22628, 3840], 30434660, 1029897),
        Monster::new(270, Class::Scout, [2442, 5894, 2430, 22828, 3885], 24745552, 1067041),
        Monster::new(272, Class::Scout, [2472, 5945, 2465, 23044, 3934], 25164048, 1105366),
        Monster::new(274, Class::Warrior, [5995, 2507, 2498, 23264, 3975], 31988000, 1144805),
        Monster::new(276, Class::Warrior, [6046, 2538, 2531, 23452, 4022], 32481020, 1185383),
        Monster::new(278, Class::Warrior, [6092, 2572, 2566, 23668, 4069], 33016860, 1227233),
        Monster::new(280, Class::Mage, [2609, 2597, 6144, 23872, 4113], 13416064, 1270279),
        Monster::new(282, Class::Mage, [2638, 2632, 6195, 24088, 4161], 13633808, 1314549),
        Monster::new(284, Class::Warrior, [6245, 2671, 2668, 24292, 4203], 34616100, 1360180),
        Monster::new(286, Class::Scout, [2704, 6293, 2700, 24508, 4252], 28135184, 1407095),
        Monster::new(288, Class::Scout, [2738, 6346, 2731, 24716, 4294], 28571696, 1455324),
        Monster::new(290, Class::Warrior, [6396, 2771, 2765, 6231, 4340], 36264420, 1505015),
        Monster::new(292, Class::Scout, [2806, 6442, 2802, 25132, 4386], 29454704, 1556080),
        Monster::new(294, Class::Mage, [2841, 2832, 6492, 25344, 4431], 14952960, 1608553),
        Monster::new(296, Class::Mage, [2870, 2867, 6543, 25556, 4479], 15180264, 1662591),
        Monster::new(298, Class::Warrior, [6593, 2905, 2902, 25764, 4523], 38517180, 1718104),
        Monster::new(300, Class::Warrior, [6640, 2937, 2932, 25976, 4569], 39093880, 1775123),
        Monster::new(302, Class::Scout, [2974, 6711, 2971, 26224, 4611], 31783488, 1833815),
        Monster::new(304, Class::Warrior, [6776, 3010, 3013, 26464, 4654], 40357600, 1894085),
        Monster::new(306, Class::Mage, [3048, 3053, 6840, 26728, 4697], 16410992, 1955969),
        Monster::new(308, Class::Warrior, [6906, 3089, 3091, 26964, 4741], 41659380, 2019640),
        Monster::new(310, Class::Warrior, [6973, 3121, 3132, 27220, 4784], 42327100, 2084998),
        Monster::new(312, Class::Warrior, [7040, 3160, 3173, 27456, 4828], 42968640, 2152079),
        Monster::new(314, Class::Scout, [3196, 7105, 3212, 27708, 4875], 34912080, 2221072),
        Monster::new(316, Class::Mage, [3236, 3250, 7173, 27948, 4915], 17719032, 2291866),
        Monster::new(318, Class::Warrior, [7240, 3270, 3291, 28196, 4958], 44972620, 2364500),
        Monster::new(320, Class::Warrior, [7303, 3309, 3331, 28448, 5005], 45659040, 2439169),
        Monster::new(322, Class::Warrior, [7370, 3348, 3368, 28692, 5043], 46337580, 2515761),
        Monster::new(324, Class::Warrior, [7436, 3382, 3409, 7236, 5088], 47034000, 2594317),
        Monster::new(326, Class::Scout, [3422, 7501, 3448, 29188, 5132], 38177904, 2675047),
        Monster::new(328, Class::Warrior, [7567, 3458, 3487, 29436, 5177], 48422220, 2757826),
        Monster::new(330, Class::Warrior, [7634, 3495, 3528, 29696, 5217], 49146880, 2842697),
        Monster::new(332, Class::Scout, [3532, 7700, 3567, 29936, 5262], 39874752, 2929882),
        Monster::new(334, Class::Mage, [3568, 3609, 7768, 30188, 5305], 20225960, 3019251),
        Monster::new(336, Class::Warrior, [7833, 3609, 3645, 30424, 5347], 51264440, 3110849),
        Monster::new(338, Class::Warrior, [7900, 3641, 3687, 30676, 5392], 51995820, 3204909),
        Monster::new(340, Class::Warrior, [7967, 3680, 3729, 30912, 5436], 52704960, 3301294),
        Monster::new(342, Class::Warrior, [8031, 3717, 3764, 31168, 5480], 53453120, 3400051),
        Monster::new(344, Class::Mage, [3756, 3805, 8101, 31408, 5523], 21671520, 3501429),
        Monster::new(346, Class::Warrior, [8167, 3790, 3845, 31656, 5566], 54923160, 3605278),
        Monster::new(348, Class::Warrior, [8229, 3829, 3886, 31908, 5611], 55679460, 3711649),
        Monster::new(350, Class::Warrior, [8297, 3868, 3923, 32152, 5651], 56426760, 3820806),
        Monster::new(352, Class::Warrior, [8541, 3976, 4007, 33072, 5767], 58372080, 3932590),
        Monster::new(354, Class::Warrior, [8787, 4088, 4093, 33976, 5881], 60307400, 4047055),
        Monster::new(356, Class::Scout, [4199, 9029, 4175, 34896, 5997], 49831488, 4164472),
        Monster::new(358, Class::Warrior, [9274, 4313, 4256, 35824, 6107], 64304080, 4284681),
        Monster::new(360, Class::Warrior, [9996, 4642, 4556, 38556, 6534], 69593584, 4407738),
        Monster::new(362, Class::Warrior, [10761, 4999, 4877, 41494, 6989], 75311608, 4533933),
        Monster::new(364, Class::Warrior, [11583, 5380, 5215, 44629, 7466], 81447928, 4663087),
        Monster::new(366, Class::Warrior, [12460, 5781, 5578, 47979, 7980], 88041464, 4795261),
        Monster::new(368, Class::Warrior, [13397, 6214, 5957, 51532, 8525], 95076544, 4930763),
        Monster::new(370, Class::Mage, [6418, 6128, 13843, 53238, 8758], 39502596, 5069406),
        Monster::new(372, Class::Warrior, [14300, 6632, 6299, 54963, 8991], 102505992, 5211250),
        Monster::new(374, Class::Warrior, [14765, 6840, 6470, 56701, 9233], 106314376, 5356618),
        Monster::new(376, Class::Warrior, [15233, 7059, 6649, 58485, 9478], 110244224, 5505316),
        Monster::new(378, Class::Warrior, [15716, 7280, 6822, 60298, 9716], 114264712, 5657407),
        Monster::new(380, Class::Warrior, [16203, 7500, 7506, 62140, 9973], 118376704, 5813229),
        Monster::new(382, Class::Warrior, [16700, 7729, 7192, 16000, 10226], 122561912, 5972576),
        Monster::new(384, Class::Warrior, [17199, 7961, 7373, 65914, 10493], 126884448, 6135515),
        Monster::new(386, Class::Warrior, [17717, 8198, 7566, 67861, 10748], 131311032, 6302407),
        Monster::new(388, Class::Warrior, [18240, 8432, 7758, 69809, 11021], 135778512, 6473030),
        Monster::new(390, Class::Warrior, [18767, 8678, 7954, 71816, 11296], 140400288, 6647454),
        Monster::new(392, Class::Warrior, [19306, 8927, 8151, 73848, 11569], 145111328, 6826051),
        Monster::new(394, Class::Warrior, [19856, 9175, 8354, 75919, 11850], 149940032, 7008597),
        Monster::new(396, Class::Warrior, [20413, 9432, 8564, 78007, 12138], 154843888, 7195163),
        Monster::new(398, Class::Warrior, [20977, 9686, 8769, 80151, 12429], 159901248, 7386146),
    ] as &[Monster],
    // TimeHonoredSchoolofMagic
    &[
        Monster::new(250, Class::Scout, [1400, 11000, 1400, 35000, 4500], 62500000, 31862139),
        Monster::new(257, Class::Warrior, [9722, 2404, 2426, 43730, 4764], 79681528, 36305620),
        Monster::new(265, Class::Scout, [1550, 11290, 1560, 44000, 4800], 68425000, 42016588),
        Monster::new(272, Class::Scout, [1965, 12320, 1980, 46000, 5940], 73970000, 47615630),
        Monster::new(280, Class::Mage, [3869, 3865, 12249, 49255, 7662], 39995056, 54764113),
        Monster::new(287, Class::Scout, [4465, 12440, 4430, 54100, 7960], 94220000, 61727510),
        Monster::new(295, Class::Scout, [3910, 12980, 3850, 54500, 7970], 96325000, 70595045),
        Monster::new(302, Class::Warrior, [14874, 3132, 1986, 58295, 8022], 138640080, 79213665),
        Monster::new(310, Class::Warrior, [14470, 5540, 5569, 70050, 10197], 174284400, 90125436),
        Monster::new(317, Class::Mage, [5911, 4338, 12174, 70050, 8540], 71282872, 100672054),
        Monster::new(325, Class::Mage, [3221, 3221, 16577, 61790, 8964], 64459324, 113997992),
        Monster::new(332, Class::Mage, [6128, 6057, 17028, 76695, 10905], 83003160, 126854805),
        Monster::new(340, Class::Mage, [5151, 5237, 18553, 79880, 12130], 90152560, 143018630),
        Monster::new(347, Class::Scout, [5875, 20170, 5870, 72300, 9320], 168640000, 158537364),
        Monster::new(355, Class::Mage, [5650, 5744, 19876, 86520, 12519], 104723808, 178017293),
        Monster::new(362, Class::Warrior, [19163, 9228, 9077, 96410, 13359], 306222272, 196694080),
        Monster::new(370, Class::Mage, [7773, 7772, 19358, 91315, 13191], 116878632, 220033230),
        Monster::new(377, Class::Mage, [5774, 5775, 24520, 96430, 13751], 125754360, 242313317),
        Monster::new(385, Class::Scout, [12400, 29300, 12600, 130350, 18640], 379430016, 270120546),
        Monster::new(400, Class::Mage, [8447, 8378, 30585, 126470, 18375], 190324704, 300000000),
    ] as &[Monster],
    // Hemorridor
    &[
        Monster::new(200, Class::Warrior, [8800, 1120, 1120, 28000, 3600], 28140000, 11412835),
        Monster::new(213, Class::Warrior, [8069, 1995, 2014, 36296, 3954], 38836720, 15166093),
        Monster::new(228, Class::Mage, [1333, 1333, 9709, 37840, 4128], 17330720, 20723258),
        Monster::new(242, Class::Scout, [1749, 10965, 1762, 40940, 5287], 39793680, 27343418),
        Monster::new(258, Class::Scout, [3559, 11269, 3559, 45315, 7049], 46946340, 36987473),
        Monster::new(273, Class::Warrior, [11818, 4209, 4209, 51395, 7562], 70411152, 48459828),
        Monster::new(289, Class::Scout, [3832, 12720, 3773, 53410, 7811], 61955600, 63853128),
        Monster::new(305, Class::Mage, [3163, 3163, 15023, 58878, 8102], 36033336, 83158305),
        Monster::new(319, Class::Warrior, [14904, 5706, 5736, 72152, 10503], 115443200, 103875282),
        Monster::new(333, Class::Scout, [6207, 12783, 6207, 73553, 8967], 98266808, 128778486),
        Monster::new(348, Class::Warrior, [17737, 3446, 3446, 66115, 9591], 115370672, 160885033),
        Monster::new(362, Class::Warrior, [18561, 6602, 6602, 83598, 11886], 151730368, 196694080),
        Monster::new(377, Class::Mage, [5718, 5813, 20594, 88667, 13464], 67032252, 242313317),
        Monster::new(392, Class::Scout, [6639, 22792, 663, 81699, 10532], 128430832, 296636662),
        Monster::new(408, Class::Mage, [6497, 6606, 22857, 99498, 14397], 81389360, 300000000),
        Monster::new(424, Class::Warrior, [22421, 10797, 10620, 112800, 15630], 239700000, 300000000),
        Monster::new(440, Class::Scout, [9250, 23036, 9250, 108665, 15697], 191685056, 300000000),
        Monster::new(456, Class::Warrior, [29669, 6998, 6998, 116680, 16639], 266613792, 300000000),
        Monster::new(474, Class::Warrior, [36039, 15498, 15498, 160331, 22927], 380786112, 300000000),
        Monster::new(500, Class::Warrior, [38231, 10473, 10474, 158088, 22969], 396010432, 300000000),
    ] as &[Monster],
    // NordicGods
    &[
        Monster::new(210, Class::Warrior, [8000, 2000, 2000, 36000, 4000], 43560000, 14222021),
        Monster::new(240, Class::Mage, [10965, 1762, 12000, 40500, 5000], 55687500, 26302843),
        Monster::new(270, Class::Mage, [4000, 4000, 11500, 51000, 7500], 31416000, 45958126),
        Monster::new(305, Class::Warrior, [15000, 3500, 3500, 58500, 8000], 101351256, 83158305),
        Monster::new(330, Class::Warrior, [12500, 6000, 6000, 73500, 9000], 137445008, 123067419),
        Monster::new(360, Class::Scout, [6500, 18500, 6500, 83500, 11500], 135938000, 191202824),
        Monster::new(390, Class::Warrior, [22500, 6500, 6500, 81500, 10500], 143440000, 288853442),
        Monster::new(420, Class::Scout, [10500, 22500, 10500, 112500, 15500], 212850000, 300000000),
        Monster::new(455, Class::Warrior, [29500, 7000, 7000, 115000, 16000], 235290000, 300000000),
        Monster::new(500, Class::Warrior, [38500, 10500, 10500, 158000, 23000], 354552000, 300000000),
    ] as &[Monster],
    // MountOlympus
    &[
        Monster::new(210, Class::Mage, [2000, 2000, 8000, 80000, 4000], 52800000, 14222021),
        Monster::new(240, Class::Warrior, [12000, 4000, 4000, 100000, 5000], 187500000, 26302843),
        Monster::new(270, Class::Scout, [6000, 16000, 6000, 120000, 6000], 201600000, 45958126),
        Monster::new(305, Class::Mage, [8000, 8000, 20000, 140000, 7000], 132300000, 83158305),
        Monster::new(330, Class::Mage, [10000, 10000, 24000, 160000, 8000], 163200000, 123067419),
        Monster::new(360, Class::Warrior, [28000, 12000, 12000, 180000, 10000], 499500000, 191202824),
        Monster::new(390, Class::Warrior, [32000, 14000, 14000, 200000, 11000], 600000000, 288853442),
        Monster::new(420, Class::Scout, [16000, 36000, 16000, 220000, 12000], 567600000, 300000000),
        Monster::new(455, Class::Scout, [28000, 40000, 28000, 250000, 13000], 697500032, 300000000),
        Monster::new(500, Class::Mage, [33333, 33333, 44444, 300000, 15000], 459000000, 300000000),
    ] as &[Monster],
    // TavernoftheDarkDoppelgangers
    &[
        Monster::new(410, Class::Mage, [7000, 7000, 20000, 18000, 4000], 10000000, 300000000),
        Monster::new(420, Class::Warrior, [24000, 8500, 8500, 20000, 5000], 30000000, 300000000),
        Monster::new(430, Class::Warrior, [29000, 10000, 10000, 22000, 6000], 25000000, 300000000),
        Monster::new(440, Class::Scout, [12000, 35000, 12000, 24000, 7500], 35000000, 300000000),
        Monster::new(450, Class::Warrior, [42000, 15000, 15000, 26000, 9000], 10000000, 300000000),
        Monster::new(460, Class::Mage, [18000, 18000, 50000, 29000, 11000], 25000000, 300000000),
        Monster::new(470, Class::Scout, [21000, 60000, 21000, 32000, 13000], 35000000, 300000000),
        Monster::new(480, Class::Mage, [25000, 25000, 72000, 35000, 15000], 50000000, 300000000),
        Monster::new(490, Class::Warrior, [86000, 30000, 30000, 39000, 19000], 30000000, 300000000),
        Monster::new(500, Class::Mage, [35000, 35000, 90000, 43000, 21000], 50000000, 300000000),
    ] as &[Monster],
    // DragonsHoard
    &[
        Monster::new(210, Class::Scout, [3100, 6200, 3100, 10500, 3100], 16000000, 14222021),
        Monster::new(213, Class::Warrior, [6560, 3280, 3280, 12000, 3280], 20500000, 15166093),
        Monster::new(216, Class::Mage, [3460, 3460, 6920, 13500, 3460], 0, 0),
        Monster::new(219, Class::Scout, [3640, 7280, 3640, 15000, 3640], 29500000, 17216101),
        Monster::new(222, Class::Scout, [3820, 7640, 3820, 16500, 3820], 34000000, 18327374),
        Monster::new(225, Class::Scout, [4000, 8000, 4000, 18000, 4000], 38500000, 19491852),
        Monster::new(228, Class::Warrior, [8360, 4180, 4180, 19500, 4180], 43000000, 20723258),
        Monster::new(231, Class::Warrior, [8720, 4360, 4360, 21000, 4360], 47500000, 22012436),
        Monster::new(234, Class::Scout, [4540, 9080, 4540, 22500, 4540], 52000000, 23374540),
        Monster::new(240, Class::Mage, [4900, 4900, 9800, 25500, 4900], 61000000, 26302843),
    ] as &[Monster],
    // HouseOfHorrors
    &[
        Monster::new(240, Class::Mage, [4900, 4900, 9800, 25500, 4900], 61000000, 26302843),
        Monster::new(243, Class::Warrior, [10160, 5080, 5080, 27000, 5080], 65500000, 27874231),
        Monster::new(246, Class::Warrior, [10520, 5260, 5260, 28500, 5260], 70000000, 29531221),
        Monster::new(249, Class::Warrior, [10880, 5440, 5440, 30000, 5440], 74500000, 31261572),
        Monster::new(252, Class::Scout, [5620, 11240, 5620, 31500, 5620], 79000000, 33084028),
        Monster::new(255, Class::Mage, [5800, 5800, 11600, 33000, 5800], 83500000, 34985806),
        Monster::new(258, Class::Scout, [5980, 11960, 5980, 34500, 5980], 88000000, 36987473),
        Monster::new(261, Class::Mage, [6160, 6160, 12320, 36000, 6160], 92500000, 39074626),
        Monster::new(264, Class::Mage, [6340, 6340, 12680, 37500, 6340], 97000000, 41269055),
        Monster::new(270, Class::Scout, [6700, 13400, 6700, 40500, 6700], 106000000, 45958126),
    ] as &[Monster],
    // ThirdLeagueOfSuperheroes
    &[
        Monster::new(280, Class::Mage, [7300, 7300, 14600, 45500, 7300], 121000000, 54764113),
        Monster::new(283, Class::Warrior, [14960, 7480, 7480, 47000, 7480], 125500000, 57660160),
        Monster::new(286, Class::Scout, [7660, 15320, 7660, 48500, 7660], 130000000, 60696773),
        Monster::new(289, Class::Warrior, [15680, 7840, 7840, 50000, 7840], 134500000, 63853128),
        Monster::new(292, Class::Scout, [8020, 16040, 8020, 51500, 8020], 139000000, 67159972),
        Monster::new(295, Class::Warrior, [16400, 8200, 8200, 53000, 8200], 143500000, 70595045),
        Monster::new(298, Class::Scout, [8380, 16760, 8380, 54500, 8380], 148000000, 74191870),
        Monster::new(301, Class::Mage, [8560, 8560, 17120, 56000, 8560], 152500000, 77926126),
        Monster::new(304, Class::Mage, [8740, 8740, 17480, 57500, 8740], 157000000, 81832831),
        Monster::new(310, Class::Scout, [9100, 18200, 9100, 60500, 9100], 166000000, 90125436),
    ] as &[Monster],
    // DojoOfChildhoodHeroes
    &[
        Monster::new(313, Class::Warrior, [18560, 9280, 9280, 62000, 9280], 170500000, 94521325),
        Monster::new(316, Class::Warrior, [18920, 9460, 9460, 63500, 9460], 175000000, 99114456),
        Monster::new(319, Class::Mage, [9640, 9640, 19280, 65000, 9640], 179500000, 103875282),
        Monster::new(322, Class::Warrior, [19640, 9820, 9820, 66500, 9820], 184000000, 108847145),
        Monster::new(325, Class::Mage, [10000, 10000, 20000, 68000, 10000], 188500000, 113997992),
        Monster::new(328, Class::Scout, [10180, 20360, 10180, 69500, 10180], 193000000, 119373368),
        Monster::new(331, Class::Mage, [10360, 10360, 20720, 71000, 10360], 197500000, 124939452),
        Monster::new(334, Class::Mage, [10540, 10540, 21080, 72500, 10540], 202000000, 130745487),
        Monster::new(337, Class::Warrior, [21440, 10720, 10720, 74000, 10720], 206500000, 136754704),
        Monster::new(340, Class::Mage, [10900, 10900, 21800, 75500, 10900], 211000000, 143018630),
    ] as &[Monster],
    // MonsterGrotto
    &[
        Monster::new(480, Class::Warrior, [66000, 35000, 35000, 174000, 35000], 540000000, 300000000),
        Monster::new(483, Class::Mage, [35750, 35750, 67350, 176400, 35750], 549000000, 300000000),
        Monster::new(486, Class::Scout, [36500, 68700, 36500, 178800, 36500], 558000000, 300000000),
        Monster::new(489, Class::Warrior, [70050, 37250, 37250, 181200, 37250], 567000000, 300000000),
        Monster::new(492, Class::Mage, [38000, 38000, 71400, 183600, 38000], 576000000, 300000000),
        Monster::new(495, Class::Warrior, [72750, 38750, 38750, 186000, 38750], 585000000, 300000000),
        Monster::new(498, Class::Scout, [39500, 74100, 39500, 188400, 39500], 594000000, 300000000),
        Monster::new(501, Class::Scout, [40250, 80500, 40250, 190800, 40250], 603000000, 300000000),
        Monster::new(504, Class::Warrior, [82000, 41000, 41000, 193200, 41000], 612000000, 300000000),
        Monster::new(510, Class::Warrior, [85000, 42500, 42500, 198000, 42500], 630000000, 300000000),
    ] as &[Monster],
    // CityOfIntrigues
    &[],
    // SchoolOfMagicExpress
    &[],
    // AshMountain
    &[],
    // PlayaGamesHQ
    &[],
    // TrainingCamp
    &[],
    // Sandstorm
    &[]
]);
