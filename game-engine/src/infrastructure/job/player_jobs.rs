use std::collections::HashMap;

pub fn get_player_job_sprite_mapping() -> HashMap<u32, &'static str> {
    let mut map = HashMap::new();

    map.insert(0, "초보자");
    map.insert(1, "검사");
    map.insert(2, "마법사");
    map.insert(3, "궁수");
    map.insert(4, "성직자");
    map.insert(5, "상인");
    map.insert(6, "도둑");
    map.insert(7, "기사");
    map.insert(8, "프리스트");
    map.insert(9, "위저드");
    map.insert(10, "제철공");
    map.insert(11, "헌터");
    map.insert(12, "어세신");
    map.insert(14, "크루세이더");
    map.insert(15, "몽크");
    map.insert(16, "세이지");
    map.insert(17, "로그");
    map.insert(18, "연금술사");
    map.insert(19, "바드");
    map.insert(20, "무희");
    map.insert(23, "슈퍼노비스");
    map.insert(24, "건너");
    map.insert(25, "닌자");

    map.insert(4001, "초보자");
    map.insert(4002, "검사");
    map.insert(4003, "마법사");
    map.insert(4004, "궁수");
    map.insert(4005, "성직자");
    map.insert(4006, "상인");
    map.insert(4007, "도둑");
    map.insert(4008, "기사");
    map.insert(4009, "프리스트");
    map.insert(4010, "위저드");
    map.insert(4011, "제철공");
    map.insert(4012, "헌터");
    map.insert(4013, "어세신");
    map.insert(4015, "크루세이더");
    map.insert(4016, "몽크");
    map.insert(4017, "세이지");
    map.insert(4018, "로그");
    map.insert(4019, "연금술사");
    map.insert(4020, "바드");
    map.insert(4021, "무희");
    map.insert(4023, "슈퍼노비스");
    map.insert(4024, "건너");
    map.insert(4025, "닌자");
    map.insert(4046, "태권소년");
    map.insert(4047, "권성");
    map.insert(4049, "소울링커");

    map.insert(4054, "로드나이트");
    map.insert(4055, "하이프리");
    map.insert(4056, "하이위저드");
    map.insert(4057, "화이트스미스");
    map.insert(4058, "스나이퍼");
    map.insert(4059, "어쌔신크로스");
    map.insert(4060, "로드페코");
    map.insert(4061, "팔라딘");
    map.insert(4062, "챔피온");
    map.insert(4063, "프로페서");
    map.insert(4064, "스토커");
    map.insert(4065, "크리에이터");
    map.insert(4066, "클라운");
    map.insert(4067, "클라운");

    map.insert(4096, "드래곤나이트");
    map.insert(4097, "성투사2");
    map.insert(4098, "소서러");
    map.insert(4099, "미케닉");
    map.insert(4100, "레인져");
    map.insert(4101, "길로틴크로스");
    map.insert(4103, "가드");
    map.insert(4104, "슈라");
    map.insert(4105, "제네릭");
    map.insert(4106, "쉐도우체이서");
    map.insert(4107, "아크비숍");
    map.insert(4108, "워록");
    map.insert(4109, "민스트럴");

    map.insert(4211, "dragon_knight");
    map.insert(4212, "imperial_guard");
    map.insert(4213, "arch_mage");
    map.insert(4214, "cardinal");
    map.insert(4215, "meister");
    map.insert(4216, "shadow_cross");
    map.insert(4217, "arch_mage");
    map.insert(4218, "cardinal");
    map.insert(4219, "windhawk");
    map.insert(4220, "imperial_guard");
    map.insert(4221, "biolo");
    map.insert(4222, "abyss_chaser");
    map.insert(4223, "elemetal_master");
    map.insert(4224, "inquisitor");
    map.insert(4225, "troubadour");
    map.insert(4226, "trouvere");

    map.insert(4239, "rebellion");
    map.insert(4240, "kagerou");
    map.insert(4241, "oboro");

    map.insert(4252, "소울리퍼");
    map.insert(4253, "성제");

    map
}

pub fn is_player_job(job_id: u32) -> bool {
    (job_id <= 150)
        || (4001..=4300).contains(&job_id)
        || (21..=25).contains(&job_id)
        || (4046..=4049).contains(&job_id)
}
