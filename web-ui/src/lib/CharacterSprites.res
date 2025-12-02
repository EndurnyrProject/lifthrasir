type gender = Female | Male

let genderToInt = gender =>
  switch gender {
  | Female => 0
  | Male => 1
  }

let genderFromInt = int =>
  switch int {
  | 0 => Female
  | _ => Male
  }

let jobSpriteNames: Map.t<int, string> = Map.fromArray([
  (0, `초보자`),
  (1, `검사`),
  (2, `마법사`),
  (3, `궁수`),
  (4, `성직자`),
  (5, `상인`),
  (6, `도둑`),
  (7, `기사`),
  (8, `프리스트`),
  (9, `위저드`),
  (10, `제철공`),
  (11, `헌터`),
  (12, `어세신`),
  (14, `크루세이더`),
  (15, `몽크`),
  (16, `세이지`),
  (17, `로그`),
  (18, `알케미스트`),
  (19, `바드댄서`),
])

let getGenderSuffix = gender =>
  switch gender {
  | Female => `여`
  | Male => `남`
  }

let getBodySpritePath = (jobClass: int, gender: gender): string => {
  let sexSuffix = getGenderSuffix(gender)
  let jobSpriteName = jobSpriteNames->Map.get(jobClass)->Option.getOr(`초보자`)
  `data\\sprite\\인간족\\몸통\\${sexSuffix}\\${jobSpriteName}_${sexSuffix}.spr`
}

let getHairSpritePath = (hairStyle: int, gender: gender): string => {
  let sexSuffix = getGenderSuffix(gender)
  let hairStyleStr = Int.toString(hairStyle)
  `data\\sprite\\인간족\\머리통\\${sexSuffix}\\${hairStyleStr}_${sexSuffix}.spr`
}

let getHairPalettePath = (hairStyle: int, gender: gender, hairColor: int): option<string> => {
  switch hairColor {
  | 0 => None
  | color => {
      let sexSuffix = getGenderSuffix(gender)
      let hairStyleStr = Int.toString(hairStyle)
      let colorStr = Int.toString(color)
      Some(`data\\palette\\머리\\${hairStyleStr}_${sexSuffix}_${colorStr}.pal`)
    }
  }
}
