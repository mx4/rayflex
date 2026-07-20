# Third-party model / texture credits

Downloaded assets used by the texture-showcase scenes. The other meshes in
this directory (teapot, buddha, cow, trolley, teddy, cornell-box) predate
these and are not covered here.

## Keenan Crane model repository — `spot`, `bob`, `blub`

`spot.obj`, `bob.obj`, `blub.obj` and their `*_texture.png` maps (used by
`scenes/toybox.json`) are from Keenan Crane's 3D model repository:
https://www.cs.cmu.edu/~kmcrane/Projects/ModelRepository/

Released by the author **into the public domain** (CC0). The single-material
`*.mtl` files here were authored locally (the originals ship without one) to
point at each `map_Kd` texture.

## Survival Guitar Backpack — `backpack`

`backpack.obj` / `backpack.mtl` / `diffuse.jpg` (used by
`scenes/backpack.json`) — model by **Berk Gedik**:
https://sketchfab.com/3d-models/survival-guitar-backpack-low-poly-799f8c4511f84fab8c3f12887f7e6b36

Licensed **CC BY 4.0** (https://creativecommons.org/licenses/by/4.0/).
Obtained via the LearnOpenGL resources (Joey de Vries), which modified the
material assignment and renamed the albedo map to `diffuse.jpg` for a
non-PBR setup. Only the diffuse map is used here (normal/specular/roughness
maps are not, as the renderer supports `map_Kd` only).

## Atrium Sponza — `sponza/`

`sponza/sponza.obj`, `sponza/sponza.mtl`, and `sponza/textures/*.jpg` (used
by `scenes/sponza.json`) — the Crytek Sponza atrium by **Frank Meinl**
(Crytek, 2010), based on the original Sponza by Marko Dabrovic (2002).

Licensed **CC BY 3.0** (https://creativecommons.org/licenses/by/3.0/).
Obtained via the OBJ mirror at https://github.com/jimmiebergmann/Sponza .

Modified locally for this repo:
- geometry for the three alpha-masked materials (`vase_plant`, `chain`,
  `leaf`) was stripped, since the renderer has no alpha-cutout support and
  would draw them as opaque slabs;
- only the `map_Kd` diffuse textures are kept, converted from the original
  uncompressed TGA to JPG (q90) to cut ~91 MB to ~11 MB. The `.mtl` still
  lists the original `map_d`/`map_bump`/etc. maps, but the loader reads only
  `map_Kd`.
