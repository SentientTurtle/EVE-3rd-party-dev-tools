# Glossary

### Data Objects

Notation: *[NAME]* (`[ID name]`)  
These data-types usually have a unique ID number, which is widely used to refer to them.

* *Type* (`typeID`)  
  Kind of game item; Akin to data-types & classes and instances thereof.  
  *Types* describe most "things" in the game; 'Inventory'/cargo items, ships, objects in space.  
* *Item* (`itemID`)
  Individual instances of a type; e.g. *Type* 648 ("Badger") describes all Badger ships, any individual assembled ship has a unique itemID.  
  "An object" as opposed to "a class" in programming terms.
* *Group* (`groupID`)  
  Collection of related *Types*
* *Category* (`categoryID`)  
  Collection of related *Groups*  
  Usually differentiates the "kind" of object; E.g. "Ship" and "Module" are different *Categories*.

* *Icon* (`iconID`)  
  Icon images, such as inventory icons, UI icons, overview icons, etc.
* *Graphic* (`graphicID`)  
  Info about 3D models; Model geometry, textures, icons/renders of those models.

### APIs & Data sources

* *ESI* "EVE Swagger Interface"  
  Current 3rd party development API
* *CREST* "Carbon RESTful API"  
  Previous generation 3rd party dev API (Defunct since 2018)
* *XML API*  
  Previous generation 3rd party dev API (Defunct since 2018)
* *IGB* "In-Game Browser"
  In-game browser, defunct since 2016. Used to provide some JavaScript APIs to interact with the game client.  
  Some of the API functionality has been added to *ESI*.

* *SDE* "Static Data Export"  
  Export of static game data (i.e. only changing with game updates), currently provided by CCP in YAML format.  
  Many community conversions are available.
* *Static Data Dump*  
  Prior generation of *SDE*; A direct database dump file rather than the current YAML files. (Defunct since ???)

### Data Formats
* *Multibuy*  
  The in-game "multi-buy" menu for buying multiple items at once supports a variety of input formats.  
  Known formats are:
  * `{QUANTITY}[tab or space]{ITEM_NAME}`
  * `{ITEM_NAME}[tab or space]{QUANTITY}`
  * `{ITEM_NAME} x{QUANTITY}`
  * TODO: Check EFT format

* *Ship DNA*
  Compact data format for ship fittings
> Full Description  
> The Ship DNA format is a shorthand notation to describe a ship and its fitting purely through the use of the type ids of the modules, items and the ship itself. It follows this basic format.  
> First in the format is the ship id. This is followed by a colon (:). If T3 then this is followed by a list of 5 subsystems seperated with colons. This is followed by a list of module ids and quantity with colons (:) separating them. Each module id/quantity is in the format of <moduleID>;<quantity> Charges and Drones can also be included by listing them in the same fashion as modules.  
> All modules are assumed to be fit, therefore only include modules that will fit in the ship itself.
>
> DNA -> SHIP ':' HIGHS ':' MEDS ':' LOWS ':' RIGS ':' CHARGES  
> SHIP -> SHIP_TYPE_ID ( ':' SUBSYSTEM_ID ':' SUBSYSTEM_ID ':' SUBSYSTEM_ID ':' SUBSYSTEM_ID ':' SUBSYSTEM_ID )  
> HIGHS -> EMPTY | MODULE ( ':' MODULE )  
> MEDS -> EMPTY | MODULE ( ':' MODULE )  
> LOWS -> EMPTY | MODULE ( ':' MODULE )  
> RIGS -> EMPTY | MODULE ( ':' MODULE )  
> CHARGES -> EMPTY | CHARGE ( ':' CHARGE )  
> MODULE -> QUANTITY ';' MODULE_ID  
> CHARGE -> QUANTITY ';' CHARGE_ID  
> SHIP_TYPE_ID -> the typeID of a ship  
> SUBSYSTEM_ID -> the typeID of the fitted subsystems  
> MODULE_ID -> the typeID of the fitted module  
> CHARGE_ID -> the typeID of a charge or a drone  
> QUANTITY -> an integer quantity of the type.
>
> (Source: https://web.archive.org/web/20100315140744/http://wiki.eveonline.com/en/wiki/Ship_DNA)

* *EFT*  
  "EVE Fitting Tool" Human-Readable format for ship fittings; Derived from the now-defunct third party program of the same name, format has since been adopted by CCP  
  Notes:
  * The name of the fitting is mandatory
  * Module names should always be in english when copied-to-clipboard from in-game. Other tools may not respect this.
> It's made up of a few sections that are separated with empty linebreaks:  
> First line lists the ship and fitting name, separated by a comma (i.e., [Raven, karkur's little raven fit])  
> Low slot modules  
> Mid slot modules and charge (if available)  
> High slot modules and charge (if available) (i.e., 125mm Railgun I, Antimatter Charge S)  
> Rigs  
> Subsystems  
> Drones in drone bay with amount (i.e., Warrior II x2)  
> Items in cargo bay with amount(i.e., Antimatter Charge M x1)
>
> (Source: CCP Dev blog)

```
[Tengu, my random Tengu fit]  
Internal Force Field Array I  
Magnetic Field Stabilizer II  
  
Caldari Navy Stasis Webifier  
Medium Capacitor Booster II,Navy Cap Booster 400  
  
Heavy Neutron Blaster II,Void M  
Heavy Neutron Blaster II,Void M  
  
Medium Core Defense Capacitor Safeguard II  
  
Tengu Defensive - Amplification Node  
Tengu Electronics - Emergent Locus Analyzer  
```

* *XML Fitting*  
  Older XML-based fitting format, supports multiple fittings in one file/document  Notes:
  Notes:
  * CAUTION: Module names are localized
  * Slots are similar to ESI fitting format, albeit with different notation
```XML
<?xml version="1.0" ?>
  <fittings>
    <fitting name="Tristan fit">
      <description value=""/>
      <shipType value="Tristan"/>
      <hardware qty="500" slot="cargo" type="Carbonized Lead S"/>
      <hardware qty="500" slot="cargo" type="Phased Plasma S"/>
      <hardware slot="hi slot 0" type="125mm Gatling AutoCannon I"/>
      <hardware slot="hi slot 1" type="125mm Gatling AutoCannon I"/>
      <hardware slot="low slot 0" type="Damage Control I"/>
      <hardware qty="5" slot="drone bay" type="Hobgoblin I"/>
      <hardware slot="med slot 1" type="Fleeting Compact Stasis Webifier"/>
      <hardware slot="med slot 2" type="Initiated Compact Warp Disruptor"/>
      <hardware slot="low slot 1" type="Micro Auxiliary Power Core I"/>
      <hardware slot="rig slot 0" type="Small Transverse Bulkhead I"/>
      <hardware slot="rig slot 1" type="Small Transverse Bulkhead I"/>
      <hardware slot="rig slot 2" type="Small Transverse Bulkhead I"/>
      <hardware slot="med slot 0" type="10MN Y-S8 Compact Afterburner"/>
      <hardware slot="low slot 2" type="AE-K Compact Drone Damage Amplifier"/>
    </fitting>
  </fittings>
```
```XML Japanese UI language
<?xml version="1.0" ?>
  <fittings>
    <fitting name="Tristan fit">
      <description value=""/>
      <shipType value="トリスタン"/>
      <hardware qty="500" slot="cargo" type="炭化鉛弾S"/>
      <hardware qty="500" slot="cargo" type="フェーズプラズマ弾S"/>
      <hardware slot="hi slot 0" type="125mmガトリングオートキャノンI"/>
      <hardware slot="hi slot 1" type="125mmガトリングオートキャノンI"/>
      <hardware slot="low slot 0" type="ダメージ制御I"/>
      <hardware qty="5" slot="drone bay" type="ホブゴブリンI"/>
      <hardware slot="med slot 1" type="一時的コンパクトステイシスウェビファイヤー"/>
      <hardware slot="med slot 2" type="イニシエート式コンパクトワープ妨害器"/>
      <hardware slot="low slot 1" type="超小型補助パワーコアI"/>
      <hardware slot="rig slot 0" type="小型横隔壁I"/>
      <hardware slot="rig slot 1" type="小型横隔壁I"/>
      <hardware slot="rig slot 2" type="小型横隔壁I"/>
      <hardware slot="med slot 0" type="10MN Y-S8 コンパクトアフターバーナー"/>
      <hardware slot="low slot 2" type="AE-Kコンパクトドローンダメージ強化装置"/>
    </fitting>
  </fittings>
```

### Technical terms

* *BSD* "Branched Static Data"  
  Old authoring format for Game Data, not all data has been ported over to the new *FSD* "File Static Data"  
  No meaningful difference for *SDE* users
* *FSD* "File Static Data"
  New authoring format for Game Data, not all data has been ported over  
  No meaningful difference for *SDE* users