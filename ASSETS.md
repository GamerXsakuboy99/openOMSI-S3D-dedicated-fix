# Assets: objects for the Customize screen

An **asset** is a small 3D object (a can, a poster, a ticket holder, a number plate...) that a
player puts anywhere on a bus in *Career > Fleet > Customize*: outside, or inside from the
"Interior view". It is a folder of its own, everything side by side:

```
Assets/
  ColaCan/
    cola.asset        <- the description (this file makes the folder an asset)
    cola.o3d          <- the model (.o3d or .x)
    cola_label.png    <- its textures (png, tga, dds, bmp, jpg)
```

## Where assets go

The `Assets` folder lies next to the launcher's program (the folder with the `.exe`).
`OPENOMSI_ASSETS=<folder>` names another one.

* **Drag a folder or a `.zip` onto the launcher window.** If it holds a `.asset` file it is
  put into `Assets`, not installed as a mod.
* Or copy the folder into `Assets` yourself. In the Customize screen, *Assets folder* opens
  it and *Read again* looks at it again.

A zip with the `.asset` file at its top is unpacked into a folder named like the zip; a zip
whose files are already in folders is unpacked as it is. Several `.asset` files can share a
folder (one object each). Folders inside folders are fine (up to four deep).

## The `.asset` file

A `.asset` file is written like a `model.cfg`: `[tag]` on a line, its values on the lines
after it. `[mesh]` and the `[matl...]` commands are the model.cfg's, with the same meaning.
Three tags only the assets have say what the object is.

```
[name]
Cola can

[author]
Jan Kowalski

[description]
A 0.33 l can for the driver's cabin.

[mesh]
cola.o3d

[matl]
cola_label.png
0
[matl_alpha]
1

[mesh]
cola_glow.o3d

[matl]
glow.png
0
[matl_alpha]
2
[matl_noZwrite]
```

| tag | meaning |
|---|---|
| `[name]`, `[author]`, `[description]` | Shown in the list. The name falls back to the file's. |
| `[mesh]` | A model of this folder (or a folder below it). `.o3d` or `.x`. **At least one.** Several `[mesh]` make one object. |
| `[matl]` + texture name + index | Selects a material of the mesh, as in a model.cfg. The commands below belong to it. |

Material commands that work (as in model.cfg): `[matl_alpha]` (0 solid, 1 cut out, 2
blended), `[matl_noZwrite]`, `[matl_noZcheck]`, `[matl_zbias]`, `[matl_envmap]`,
`[matl_envmap_mask]`, `[matl_bumpmap]`, `[matl_transmap]`, `[matl_nightmap]`,
`[matl_allcolor]`, `[matl_texadress_...]`. Their textures (and the mesh's own) are looked for
in the asset's folder.

Not for assets, because they need the bus's script variables or animations (they are read
and ignored): `[matl_change]`/`[matl_item]`, `[matl_lightmap]`, `[matl_alphascale]`,
`[matl_freetex]`, `[useTextTexture]`/`[useScriptTexture]`, `[texcoordtransX/Y]`,
`[newanim]`, lights, `[visible]`. An asset is a still object.

The asset's **id** is `<folder>/<name of the .asset file without extension>`, lower case
(`colacan/cola`). Saved sets remember an object by its id, so renaming a folder or a `.asset`
file makes sets that used it lose it (it is left out; nothing breaks).

There is no size, turn or category in the file: the object is put down as modelled (scale 1)
and the player scales and turns it in the editor.

## The model

* Axes as the engine has them: x right, **y forward, z up**.
* **An object is put down exactly as modelled**: the model's axes are the bus's axes, its
  origin (0, 0, 0) is the point under the cursor. A model that is right in OMSI is right here
  - a can stands upright on a shelf, whatever surface it is put on - and the player turns it
  with the gizmo. *On the surface* stands it on the surface instead (its +z away from the
  surface, +y the top of the picture), *As modelled* puts it back.
  So put the origin where the object should touch: a can's base, a poster's middle.
* Textures: the names in the model are looked for in the asset's folder - any case, `\` or
  `/`, and if there is no such file, the same name with another picture extension (a model
  that says `label.bmp` finds `label.png`). A missing texture is logged and shows as OMSI's
  "missing" picture.
* Keep it light: an asset is drawn as part of the bus, for every copy of it in the game.

## What the player gets

In the Customize screen, "Customize objects" > "Choose an object…" shows a tile for every asset: a small picture the launcher draws itself from the model and its textures, the `[name]`, the `[author]` and, beside a larger picture, the `[description]`. Fill those three tags in.


* *ADD AN OBJECT*: choose an object, press *Add the object*; it follows the cursor over the
  bus - on any surface, outside or (Interior view) inside - and is put down with a click.
* A selected object has a **gizmo** in the picture: *Move* shows an arrow along each axis
  (drag one to move along it), *Rotate* a ring about each axis (drag one to turn about it).
  The axes are either the object's own (they turn with it) or the bus's. The same by buttons:
  *Move by* and *Turn by* with a step (1 cm to 1 m, 1° to 90°) for each axis, and *Scale*.
* It belongs to the mesh of the bus it was put on, so an object put on a door opens and
  closes with the door, even after it was moved off the surface. (Not on tyres and small
  switches.)
* Saved like stickers: for one bus, for every bus of its model, or as a named preset.
* The preview in the editor shows the pictures and the alpha modes; reflections, bump and
  night maps show in the game.

## Trying it

1. Make `Assets/Test/test.asset` with a `[mesh]` naming a small `.o3d`, put the `.o3d` and its
   textures next to it.
2. Start the launcher, open *Career > Fleet > Customize* on any bus. Under *ADD AN OBJECT*
   the object appears (use *Read again* if you added it while the screen was open).
3. Add it, click on the bus, move and turn it with the gizmo, *Save and close*, start a drive.
