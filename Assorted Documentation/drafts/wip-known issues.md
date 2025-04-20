# Known Issues

## Static Data Export
* Various data bugs; The SDE is not regenerated after small bugfixes.
#### Workaround:
* Use ESI or community alternatives

### SDE - Map data
* The z-axis "min" and "max" fields are incorrect
#### Workaround
* Negate and swap z-axis Min and Max values `(zMin, zMax) = (-zMax, -zMin)`  
* OR: Negate z coordinate in positions; `zMin < -z < zMax` does work. (Caution: This changes the coordinate system's "handedness")

### Image Service
* General Icon problems
  * Some icons are outdated
  * No icons for SKIN licenses
  * Tech Tier corner icons are wrong on some types
  * Reaction "blueprint items" use the wrong background
  * Transparency/Alpha is premultiplied when it shouldn't be

#### Workaround [DO NOT INCLUDE THIS ONE IN OFFICIAL DOCS]
* Use community alternatives:
  * https://newedenencyclopedia.net/thirdpartydev.html