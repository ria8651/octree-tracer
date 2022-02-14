# Data layout

For each octree node:
```
01100101 01100101 01100101 01100101
^------Pointer to children-----^^-^
       or Palette index          |
                                 |
        How many times voxel-----+
        has been hit by ray

```
