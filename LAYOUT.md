# Data layout

For each octree node:
```
01100101 01100101 01100101 01100101
^-----------Pointer-----------^^-Hit counter
```
The pointer is either a pointer to a group of 8 children or if the pointer is greater than VOXEL_OFFSET then the node is a voxel and the pointer is VOXEL_OFFSET + palette id. A palette id of 0 (just VOXEL_OFFSET) is the empty node.
