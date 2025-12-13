open Tauri

type gender = Male | Female

type serverInfo = {
  ip: int,
  port: int,
  name: string,
  users: int,
  @as("server_type") serverType: JSON.t,
  @as("new_server") newServer: int,
}

type sessionData = {
  username: string,
  @as("login_id1") loginId1: int,
  @as("account_id") accountId: int,
  @as("login_id2") loginId2: int,
  sex: int,
  servers: array<serverInfo>,
}

type loginResponse = {
  success: bool,
  error: option<string>,
  @as("session_data") sessionData: option<sessionData>,
}

type characterData = {
  @as("char_id") charId: int,
  name: string,
  @as("class") class_: int,
  @as("job_name") jobName: string,
  @as("body_sprite_path") bodySpritePathh: string,
  @as("hair_sprite_path") hairSpritePath: string,
  @as("hair_palette_path") hairPalettePath: option<string>,
  @as("base_level") baseLevel: int,
  @as("job_level") jobLevel: int,
  @as("base_exp") baseExp: int,
  @as("job_exp") jobExp: int,
  hp: int,
  @as("max_hp") maxHp: int,
  sp: int,
  @as("max_sp") maxSp: int,
  zeny: int,
  str: int,
  agi: int,
  vit: int,
  int_: int,
  dex: int,
  luk: int,
  hair: int,
  @as("hair_color") hairColor: int,
  @as("clothes_color") clothesColor: int,
  weapon: int,
  shield: int,
  @as("head_top") headTop: int,
  @as("head_mid") headMid: int,
  @as("head_bottom") headBottom: int,
  robe: int,
  @as("last_map") lastMap: string,
  @as("delete_date") deleteDate: Nullable.t<int>,
  sex: gender,
}

type characterListResponse = {
  success: bool,
  error: option<string>,
  characters: option<array<characterData>>,
}

type selectCharacterResponse = {
  success: bool,
  error: option<string>,
}

type characterStatus = {
  hp: int,
  @as("max_hp") maxHp: int,
  sp: int,
  @as("max_sp") maxSp: int,
  @as("base_level") baseLevel: int,
  @as("job_level") jobLevel: int,
  @as("base_exp") baseExp: int,
  @as("max_base_exp") maxBaseExp: int,
  @as("job_exp") jobExp: int,
  @as("max_job_exp") maxJobExp: int,
  zeny: int,
  weight: int,
  @as("max_weight") maxWeight: int,
}

type spritePngResponse = {
  @as("data_url") dataUrl: string,
  width: int,
  height: int,
  @as("offset_x") offsetX: int,
  @as("offset_y") offsetY: int,
  @as("from_cache") fromCache: bool,
}

type preloadBatchResponse = {
  @as("successful_keys") successfulKeys: array<string>,
  @as("failed_keys") failedKeys: array<string>,
  total: int,
}

module Auth = {
  type loginRequest = {
    username: string,
    password: string,
  }

  let login = (request: loginRequest): promise<loginResponse> => {
    Core.invoke("login", {"request": request})
  }

  let selectServer = (serverIndex: int): promise<selectCharacterResponse> => {
    Core.invoke("select_server", {"serverIndex": serverIndex})
  }
}

module Character = {
  let getCharacterList = (): promise<characterListResponse> => {
    Core.invokeNoArgs("get_character_list")
  }

  let selectCharacter = (slot: int): promise<selectCharacterResponse> => {
    Core.invoke("select_character", {"slot": slot})
  }

  let getCharacterStatus = (): promise<characterStatus> => {
    Core.invokeNoArgs("get_character_status")
  }
}

module Input = {
  let forwardKeyboardInput = (code: string, pressed: bool): promise<unit> => {
    Core.invoke("forward_keyboard_input", {"code": code, "pressed": pressed})
  }

  let forwardMousePosition = (x: float, y: float): promise<unit> => {
    Core.invoke("forward_mouse_position", {"x": x, "y": y})
  }

  let forwardMouseClick = (x: float, y: float): promise<unit> => {
    Core.invoke("forward_mouse_click", {"x": x, "y": y})
  }

  let forwardCameraRotation = (deltaX: float, deltaY: float): promise<unit> => {
    Core.invoke("forward_camera_rotation", {"deltaX": deltaX, "deltaY": deltaY})
  }
}

module Assets = {
  let getAsset = (path: string): promise<array<int>> => {
    Core.invoke("get_asset", {"path": path})
  }

  type spritePngRequest = {
    spritePath: string,
    actionIndex: int,
    frameIndex: int,
    actPath: option<string>,
    palettePath: option<string>,
    scale: float,
  }

  let getSpritePng = (request: spritePngRequest): promise<spritePngResponse> => {
    Core.invoke(
      "get_sprite_png",
      {
        "spritePath": request.spritePath,
        "actionIndex": request.actionIndex,
        "frameIndex": request.frameIndex,
        "actPath": request.actPath,
        "palettePath": request.palettePath,
        "scale": request.scale,
      },
    )
  }

  type batchRequest = {
    @as("sprite_path") spritePath: string,
    @as("act_path") actPath: option<string>,
    @as("action_index") actionIndex: int,
    @as("frame_index") frameIndex: int,
    @as("palette_path") palettePath: option<string>,
    scale: float,
  }

  let preloadSpriteBatch = (requests: array<batchRequest>): promise<preloadBatchResponse> => {
    Core.invoke("preload_sprite_batch", {"requests": requests})
  }

  let clearSpriteCache = (): promise<unit> => {
    Core.invokeNoArgs("clear_sprite_cache")
  }
}

module Utility = {
  let openDevtools = (): promise<unit> => {
    Core.invokeNoArgs("open_devtools")
  }

  let sendChatMessage = (message: string): promise<unit> => {
    Core.invoke("send_chat_message", {"message": message})
  }
}
