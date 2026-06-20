# Turtle's Tools for EVE Online third party dev

A mixed bag of tools & libraries for EVE Online third party development.

Current services:
* 'Turtle's Alternate Icons': Alternative set of item icons for EVE Online, correcting some problems in the officially provided icons.  
  Provided as GitHub Releases in normal, "Old style" (only glossy tech-tier icons) and "Bonus Style" (modern tech-tier icons, clone requirement icons, and module slot requirements)  
  To mix-and-match options, the 'eveicongenerator' rust crate can be built and ran.
  !['Turtle's Alternate Icons'.
  Features: Fixed tech-tier icons. Proper Transparency. Better Blueprints. SKIN support.
  Bonus options: Old Style Overlays, Clone Restriction icons, and Module Slot icons.](./turtlealticons.png)

Current software:
* "evestaticdata" A Rust library to interface with the EVE Online 'Static Data Export'
* "evesharedcache" A Rust library to interface with the EVE Online SharedCache game files, and CDN
* "eveicongenerator" A Rust program to generate EVE Online item-type icons
* "sdeslash" A proof of concept web service to provide arbitrary 'subsets' of the EVE Online Static Data Export.