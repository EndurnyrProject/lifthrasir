type uint8Array
type blob

@val @scope("URL")
external createObjectURL: blob => string = "createObjectURL"

@val @scope("URL")
external revokeObjectURL: string => unit = "revokeObjectURL"

@new external makeUint8Array: array<int> => uint8Array = "Uint8Array"

@new external makeBlob: array<uint8Array> => blob = "Blob"

@val external btoa: string => string = "btoa"

@val @scope("String") @variadic
external fromCharCodeMany: array<int> => string = "fromCharCode"

let loadAsset = async (path: string): string => {
  let bytes = await TauriCommands.Assets.getAsset(path)
  let uint8Array = makeUint8Array(bytes)
  let blob = makeBlob([uint8Array])
  createObjectURL(blob)
}

let loadAssetAsDataUrl = async (path: string, ~mimeType: string="application/octet-stream"): string => {
  let bytes = await TauriCommands.Assets.getAsset(path)
  let base64 = btoa(fromCharCodeMany(bytes))
  `data:${mimeType};base64,${base64}`
}

let preloadAssets = async (paths: array<string>): Map.t<string, string> => {
  let loadOne = async path => {
    let url = await loadAsset(path)
    (path, url)
  }

  let results = await paths->Array.map(loadOne)->Promise.all
  Map.fromArray(results)
}

let revokeAssetUrl = revokeObjectURL
