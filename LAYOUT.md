# Data layout

For each octree node:
```
01100101 01100101 01100101 01100101
^------Pointer to children--------^
       or Voxel index
```

For each octree voxel:
```
01100101 01100101 01100101 01100101
^-Ray hit count-^ ^-Palette index-^
       or Voxel index
```
