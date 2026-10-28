# Changelog

## 0.1.0 (2026-10-28)


### Features

* **bevy:** Updated bevy to 0.20 ([ce6011c](https://github.com/EndurnyrProject/lifthrasir/commit/ce6011c1a195b3c2721aa0ce5547eb6f4ef48cfd))
* Dropped the transform-gizmo-bevy, using transform-gizmo directly ([c23254c](https://github.com/EndurnyrProject/lifthrasir/commit/c23254cd899c6acb0d48f8486cd5a717ccad074c))
* **editor:** add and edit point, spot and sun lights ([b3968cc](https://github.com/EndurnyrProject/lifthrasir/commit/b3968ccc2a61763bfa3a256b0e67f08b370462f0))
* **editor:** Adds support to sessions ([e27a342](https://github.com/EndurnyrProject/lifthrasir/commit/e27a342dd242f72459a28dd0428ccf79a836a74f))
* **editor:** asset index and browser panel ([8cbbe09](https://github.com/EndurnyrProject/lifthrasir/commit/8cbbe0978cd8c7b17f42f595f3cc79a2f49f7c09))
* **editor:** auto gat ([b58a4e6](https://github.com/EndurnyrProject/lifthrasir/commit/b58a4e6e200d212d9facb3f76f35dadfcc50babe))
* **editor:** autosave snapshots + settings window - [#94](https://github.com/EndurnyrProject/lifthrasir/issues/94) ([48f2003](https://github.com/EndurnyrProject/lifthrasir/commit/48f20039e6d6116e50adfa52edf381f1cfde80d0))
* **editor:** collect map documents from the live scene with byte-exact saves ([ee00bb4](https://github.com/EndurnyrProject/lifthrasir/commit/ee00bb4e4b5e46c57601b3d3eb3f6000b129b32f))
* **editor:** edit the sun's elevation and azimuth in the inspector ([1574642](https://github.com/EndurnyrProject/lifthrasir/commit/15746428cfe4234ddde8ee73141546b9de515845))
* **editor:** editor-side picking with hover outlines and an outliner window ([ce898dd](https://github.com/EndurnyrProject/lifthrasir/commit/ce898dd7f9e753d38e9b398aa1700f5d5630503f))
* **editor:** GAT cell painting with a walkability overlay ([fd5e273](https://github.com/EndurnyrProject/lifthrasir/commit/fd5e273234e3c4ced53abbad71b48cfbb57afe97))
* **editor:** limit fps to 60 ([296b59b](https://github.com/EndurnyrProject/lifthrasir/commit/296b59b88dcc76e7c9e985caa24978704095140a))
* **editor:** map settings window for ambient, exposure, indoor and tint ([5389c2e](https://github.com/EndurnyrProject/lifthrasir/commit/5389c2e270807b4e2fbeb63e82691c4ec080f706))
* **editor:** models grid view ([af7d9bb](https://github.com/EndurnyrProject/lifthrasir/commit/af7d9bb6dc835d0f6da0296480ca6804df219039))
* **editor:** new map wizard ([bf72490](https://github.com/EndurnyrProject/lifthrasir/commit/bf72490e264d83ad56d1e5faed76da91ce3f281e))
* **editor:** open, save and save-as map glbs with an unsaved-changes guard ([c98c9c3](https://github.com/EndurnyrProject/lifthrasir/commit/c98c9c330d74f1389a18d4a9458ea6a25c24d3c2))
* **editor:** place props, sounds and effects on the terrain ([e2155a5](https://github.com/EndurnyrProject/lifthrasir/commit/e2155a571677398e0849567f6efda86c96ae1f20))
* **editor:** render-to-texture asset preview ([2c1a356](https://github.com/EndurnyrProject/lifthrasir/commit/2c1a356f1a7b3b5f975313c077967db5c2765054))
* **editor:** return to select with the move gizmo after placing a light ([a7039b0](https://github.com/EndurnyrProject/lifthrasir/commit/a7039b0047d3da8a9ca9d538fa34931fd9163061))
* **editor:** return to select with the move gizmo after placing an asset ([02d757f](https://github.com/EndurnyrProject/lifthrasir/commit/02d757f8d1946ab45621a764058b2c5dbe637528))
* **editor:** scaffold the map editor app with egui shell and camera ([b0ab654](https://github.com/EndurnyrProject/lifthrasir/commit/b0ab654cbc5e2a60f2d98fa0ce21a9290b09a785))
* **editor:** selection, editor handles and node inspector ([112d87f](https://github.com/EndurnyrProject/lifthrasir/commit/112d87f428b62dd5e604b04995b646976e1dfb15))
* **editor:** terrain chunk index, mesh snapshots and quad helpers ([6320e5e](https://github.com/EndurnyrProject/lifthrasir/commit/6320e5edf760edd1841fe14d93628d3e11cf9d53))
* **editor:** terrain sculpt tool with GAT height update ([7fc95fc](https://github.com/EndurnyrProject/lifthrasir/commit/7fc95fcaa257c53463cf1642c6357e1e25630c76))
* **editor:** tile paint tool ([c3534f0](https://github.com/EndurnyrProject/lifthrasir/commit/c3534f09bfcfa7f9c6b81af2c7c0e1db47e01fea))
* **editor:** transform gizmo with one undo step per drag ([f7b1f2b](https://github.com/EndurnyrProject/lifthrasir/commit/f7b1f2bd94d50d0cac779299f4acb3e65abd3f84))
* **editor:** undo stack, stable node ids and node snapshots ([6f6ce28](https://github.com/EndurnyrProject/lifthrasir/commit/6f6ce280cc097ba5aa281c1afaea400a7c18e5b0))
* **editor:** vertex color brush ([52717d6](https://github.com/EndurnyrProject/lifthrasir/commit/52717d674751811c895a28fa7a1325df3ce187e7))
* **editor:** wall tool sharing the converter's wall geometry ([71efed6](https://github.com/EndurnyrProject/lifthrasir/commit/71efed67dc2eeaf5cacde353dcf37b8d15955ae8))
* **editor:** WASD pans the orbit camera; gizmo modes move to 1/2/3 ([b5949ef](https://github.com/EndurnyrProject/lifthrasir/commit/b5949ef07ecc11af178b376e47760055aab40b41))
* **editor:** water zones, tile recompute and tile painting ([0cc7ba6](https://github.com/EndurnyrProject/lifthrasir/commit/0cc7ba6adc08845a2615cf592c9b112500c8ca78))
* **textures:** Changing textures compression to BC7 ([cc0dc72](https://github.com/EndurnyrProject/lifthrasir/commit/cc0dc72049959b1f0d801f469c63715928a5bf2b))


### Bug Fixes

* **editor:** render Korean text with a system Hangul fallback font ([2f18380](https://github.com/EndurnyrProject/lifthrasir/commit/2f1838055c1e6ee59b2c3edcfa9eb51689f943a5))


### Performance Improvements

* **editor:** chunk the GAT overlay and build it in parallel ([cbbe082](https://github.com/EndurnyrProject/lifthrasir/commit/cbbe082f4bbcc45ee265a64d780fb20ca09c839f))
