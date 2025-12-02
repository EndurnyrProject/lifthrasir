type spritePngRequest = {
  spritePath: string,
  actPath: option<string>,
  actionIndex: int,
  frameIndex: int,
  palettePath: option<string>,
  scale: float,
}

type spritePngResponse = {
  dataUrl: string,
  width: int,
  height: int,
  offsetX: int,
  offsetY: int,
  fromCache: bool,
}

let getSpritePng = async (request: spritePngRequest): spritePngResponse => {
  let response = await TauriCommands.Assets.getSpritePng({
    spritePath: request.spritePath,
    actionIndex: request.actionIndex,
    frameIndex: request.frameIndex,
    actPath: request.actPath,
    palettePath: request.palettePath,
    scale: request.scale,
  })

  {
    dataUrl: response.dataUrl,
    width: response.width,
    height: response.height,
    offsetX: response.offsetX,
    offsetY: response.offsetY,
    fromCache: response.fromCache,
  }
}

type preloadBatchResponse = {
  successfulKeys: array<string>,
  failedKeys: array<string>,
  total: int,
}

let preloadSpriteBatch = async (requests: array<spritePngRequest>): preloadBatchResponse => {
  let batchRequests =
    requests->Array.map(req => {
      let result: TauriCommands.Assets.batchRequest = {
        spritePath: req.spritePath,
        actPath: req.actPath,
        actionIndex: req.actionIndex,
        frameIndex: req.frameIndex,
        palettePath: req.palettePath,
        scale: req.scale,
      }
      result
    })

  let response = await TauriCommands.Assets.preloadSpriteBatch(batchRequests)

  {
    successfulKeys: response.successfulKeys,
    failedKeys: response.failedKeys,
    total: response.total,
  }
}

let clearSpriteCache = async (): unit => {
  await TauriCommands.Assets.clearSpriteCache()
}
