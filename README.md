### RAYMAX

Rust implementation of a ray-tracer.

## Scenes
Scenes are described by a json file that contains:
 - the position, direction and field-of-view of the camera
 - the definition of the light sources
 - the definition of each material kd/ke/ks used throughout the scene
 - the position of each infinite-plane, sphere or triangle if any,
 - a pointer to a 3D mesh object stored in OBJ format
 - the resolution of the resulting picture

## Screenshots

![teapot](./assets/teapot.png)
![cornell-box](./assets/cornell-box.png)
![buddha](./assets/buddha.png)
